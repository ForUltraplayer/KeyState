use crate::config::InputMode;
use crate::input::keycode::is_mouse_vk;
use crate::state::{CaptureMode, SharedState};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    MAPVK_VSC_TO_VK_EX, MOD_CONTROL, MOD_SHIFT,
};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RAWKEYBOARD, RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostMessageW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow,
    SetWindowsHookExW, TrackPopupMenu, TranslateMessage, UnhookWindowsHookEx, HHOOK,
    IDI_APPLICATION, KBDLLHOOKSTRUCT, LLKHF_UP, MF_SEPARATOR, MF_STRING, MSG, MSLLHOOKSTRUCT,
    RI_KEY_BREAK, RI_KEY_E0, SW_SHOWNORMAL, TPM_NONOTIFY, TPM_RETURNCMD, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_APP, WM_HOTKEY, WM_INPUT, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSEXW, WS_OVERLAPPED,
};

const WM_TRAY_ICON: u32 = WM_APP + 1;
pub const WM_SHOW_TRAY_MENU: u32 = WM_APP + 2;
pub const WM_RELOAD_HOTKEYS: u32 = WM_APP + 5;
pub const WM_REREGISTER_RAWINPUT: u32 = WM_APP + 6;
pub const WM_UPDATE_INPUT_MODE: u32 = WM_APP + 7;
const TRAY_ID: u32 = 1;

static GLOBAL_STATE: OnceLock<Arc<SharedState>> = OnceLock::new();
static IS_ADMIN_CACHE: OnceLock<bool> = OnceLock::new();
static TASKBAR_CREATED_MSG: AtomicU32 = AtomicU32::new(0);
static RECORD_KEY_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_CAPTURE_DEDUP: OnceLock<Mutex<Option<(u32, Instant)>>> = OnceLock::new();

thread_local! {
    static ACTIVE_HOOKS: RefCell<(HHOOK, HHOOK)> = RefCell::new((HHOOK::default(), HHOOK::default()));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputSource {
    RawInput,
    LowLevelHook,
    AsyncPolling,
}

#[repr(C, align(8))]
struct AlignedRawInputBuf([u8; 256]);

/// 현재 프로세스가 관리자 권한(High Integrity)으로 실행 중인지 확인
pub fn is_running_as_admin() -> bool {
    *IS_ADMIN_CACHE.get_or_init(|| unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elev as *mut TOKEN_ELEVATION) as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elev.TokenIsElevated != 0
    })
}

/// UAC 프롬프트를 통해 현재 실행 파일을 관리자 권한으로 재실행
pub fn relaunch_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe_w = HSTRING::from(exe.as_os_str());
    let ret = unsafe { ShellExecuteW(None, w!("runas"), &exe_w, None, None, SW_SHOWNORMAL) };
    (ret.0 as isize) > 32
}

/// [C-1] eframe/winit 초기화 완료 후 Raw Input 키보드 타겟을 우리 호스트 창으로 재등록
pub fn post_reregister_raw_input(state: &SharedState) {
    let host = state.hook_thread_hwnd.load(Ordering::Acquire);
    if host != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(host as *mut core::ffi::c_void),
                WM_REREGISTER_RAWINPUT,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// [H-2] InputMode 전환 시 동적으로 LL 훅 설치/해제 및 Raw Input 갱신 요청
pub fn post_update_input_mode(state: &SharedState) {
    state.keys.clear_pressed_states();
    let host = state.hook_thread_hwnd.load(Ordering::Acquire);
    if host != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(host as *mut core::ffi::c_void),
                WM_UPDATE_INPUT_MODE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// [M-4] 프로필 불러오기/초기화 시 글로벌 단축키 재등록 요청
pub fn post_reload_hotkeys(state: &SharedState) {
    let host = state.hook_thread_hwnd.load(Ordering::Acquire);
    if host != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(host as *mut core::ffi::c_void),
                WM_RELOAD_HOTKEYS,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// [M-2] 훅 스레드와 폴러 스레드 핸들을 모두 반환 + 호스트 HWND 생성 완료까지 동기화
pub fn spawn_hook_and_tray_thread(
    state: Arc<SharedState>,
) -> (std::thread::JoinHandle<()>, std::thread::JoinHandle<()>) {
    let _ = GLOBAL_STATE.set(state.clone());
    let _ = LAST_CAPTURE_DEDUP.set(Mutex::new(None));

    let poll_state = state.clone();
    let poller_handle = std::thread::Builder::new()
        .name("async-key-poller".into())
        .spawn(move || run_async_poll_loop(poll_state))
        .expect("Failed to spawn poller thread");

    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let hook_handle = std::thread::Builder::new()
        .name("input-hook-tray".into())
        .spawn(move || run_hook_loop(state, ready_tx))
        .expect("Failed to spawn hook thread");

    let _ = ready_rx.recv();
    (hook_handle, poller_handle)
}

const MOUSE_VKS: [u32; 5] = [0x01, 0x02, 0x04, 0x05, 0x06];

fn run_async_poll_loop(state: Arc<SharedState>) {
    let mut prev_down: HashMap<u32, bool> = HashMap::new();
    let mut prev_mouse: [bool; 5] = [false; 5];
    let mut was_capturing = false;
    let mut timer_boosted = false;

    while state.running.load(Ordering::Acquire) {
        let mode = state.input_mode_fast();
        let capturing = state.capture_mode.lock().is_some();

        // 1) RawInput 모드에서 마우스 버튼(5개)만 초경량 폴링 (WM_MOUSEMOVE 폭주 완전 회피!)
        if mode == InputMode::RawInput {
            let need_mouse = state.any_mouse_tracked.load(Ordering::Acquire) || capturing;
            if need_mouse {
                if !timer_boosted {
                    unsafe {
                        timeBeginPeriod(1);
                    }
                    timer_boosted = true;
                }
                if capturing && !was_capturing {
                    // 첫 캡처 틱에서는 현재 눌린 마우스 버튼 시드만 기록해 클릭 오캡처 방지
                    for (i, &vk) in MOUSE_VKS.iter().enumerate() {
                        prev_mouse[i] = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
                    }
                } else {
                    for (i, &vk) in MOUSE_VKS.iter().enumerate() {
                        let down = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
                        if down != prev_mouse[i] {
                            prev_mouse[i] = down;
                            handle_raw_input(vk, down, InputSource::RawInput);
                        }
                    }
                }
                was_capturing = capturing;
                std::thread::sleep(Duration::from_millis(2));
            } else {
                if timer_boosted {
                    unsafe {
                        timeEndPeriod(1);
                    }
                    timer_boosted = false;
                }
                prev_mouse = [false; 5];
                was_capturing = false;
                std::thread::sleep(Duration::from_millis(50));
            }
            continue;
        }

        // 2) AsyncPolling 이외 모드에서는 절전
        if mode != InputMode::AsyncPolling {
            if timer_boosted {
                unsafe {
                    timeEndPeriod(1);
                }
                timer_boosted = false;
            }
            if !prev_down.is_empty() {
                prev_down.clear();
            }
            was_capturing = false;
            std::thread::sleep(Duration::from_millis(60));
            continue;
        }

        // 3) AsyncPolling (500Hz 고정밀 폴링)
        if !timer_boosted {
            unsafe {
                timeBeginPeriod(1);
            }
            timer_boosted = true;
        }

        if capturing {
            if !was_capturing {
                // [M-1] 캡처 진입 첫 사이클: 현재 눌려있는 모든 키(LMB 클릭 포함)를 시드만 하고 이벤트 미발생
                for vk in 1u32..=254 {
                    if vk == 229 || vk == 0x10 || vk == 0x11 || vk == 0x12 {
                        continue;
                    }
                    let down = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
                    prev_down.insert(vk, down);
                }
            } else {
                for vk in 1u32..=254 {
                    if vk == 229 || vk == 0x10 || vk == 0x11 || vk == 0x12 {
                        continue;
                    }
                    let down = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
                    let was = *prev_down.get(&vk).unwrap_or(&false);
                    if down != was {
                        prev_down.insert(vk, down);
                        handle_raw_input(vk, down, InputSource::AsyncPolling);
                    }
                }
            }
        } else {
            // [H-3] Lock-free 비트셋 + record_vk만 스캔
            let rec_vk = state.record_vk_atomic.load(Ordering::Acquire);
            for word_idx in 0..4usize {
                let mut mask = state.tracked_vks[word_idx].load(Ordering::Acquire);
                while mask != 0 {
                    let bit = mask.trailing_zeros();
                    mask &= mask - 1;
                    let vk = (word_idx as u32) * 64 + bit;
                    if vk == 0 || vk == 229 {
                        continue;
                    }
                    let down = unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };
                    let was = *prev_down.get(&vk).unwrap_or(&false);
                    if down != was {
                        prev_down.insert(vk, down);
                        handle_raw_input(vk, down, InputSource::AsyncPolling);
                    }
                }
            }
            if rec_vk != 0 && rec_vk < 256 && !state.is_vk_tracked(rec_vk) {
                let down = unsafe { (GetAsyncKeyState(rec_vk as i32) as u16 & 0x8000) != 0 };
                let was = *prev_down.get(&rec_vk).unwrap_or(&false);
                if down != was {
                    prev_down.insert(rec_vk, down);
                    handle_raw_input(rec_vk, down, InputSource::AsyncPolling);
                }
            }
        }
        was_capturing = capturing;
        std::thread::sleep(Duration::from_millis(2));
    }

    if timer_boosted {
        unsafe {
            timeEndPeriod(1);
        }
    }
}

unsafe fn add_tray_icon(hwnd: HWND) -> NOTIFYICONDATAW {
    let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
    let app_icon = LoadIconW(hinst, PCWSTR(1 as *const u16))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_ICON,
        hIcon: app_icon,
        ..Default::default()
    };
    let tip = if is_running_as_admin() {
        "KeyState-RS [관리자] (클릭: 설정 열기)"
    } else {
        "KeyState-RS (클릭: 설정 열기)"
    };
    for (i, ch) in tip.encode_utf16().take(nid.szTip.len() - 1).enumerate() {
        nid.szTip[i] = ch;
    }
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    nid
}

unsafe fn sync_input_backends(hwnd: HWND, hinst: HINSTANCE, mode: InputMode) {
    // 1. Raw Input (키보드 전용 0x01/0x06 — 마우스 이동 WM_INPUT 폭주 원천 차단!)
    let want_raw = matches!(mode, InputMode::RawInput | InputMode::RawInputAndHook);
    let kb_dev = [RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: if want_raw { RIDEV_INPUTSINK } else { RIDEV_REMOVE },
        hwndTarget: if want_raw { hwnd } else { HWND::default() },
    }];
    let _ = RegisterRawInputDevices(&kb_dev, std::mem::size_of::<RAWINPUTDEVICE>() as u32);

    // 2. [H-2] LL 훅은 LowLevelHook / RawInputAndHook 모드일 때만 동적으로 설치
    let want_hooks = matches!(mode, InputMode::LowLevelHook | InputMode::RawInputAndHook);
    ACTIVE_HOOKS.with(|cell| {
        let mut hooks = cell.borrow_mut();
        if want_hooks {
            if hooks.0.is_invalid() {
                hooks.0 = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook_proc), hinst, 0)
                    .unwrap_or_default();
            }
            if hooks.1.is_invalid() {
                hooks.1 = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), hinst, 0)
                    .unwrap_or_default();
            }
        } else {
            if !hooks.0.is_invalid() {
                let _ = UnhookWindowsHookEx(hooks.0);
                hooks.0 = HHOOK::default();
            }
            if !hooks.1.is_invalid() {
                let _ = UnhookWindowsHookEx(hooks.1);
                hooks.1 = HHOOK::default();
            }
        }
    });
}

fn run_hook_loop(state: Arc<SharedState>, ready_tx: std::sync::mpsc::SyncSender<()>) {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();

        // [L-5] explorer.exe 재시작 시 트레이 아이콘 자동 복구 메시지 등록
        let tb_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
        TASKBAR_CREATED_MSG.store(tb_msg, Ordering::Release);

        let class_name = w!("KeyStateRsHiddenHost");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(host_wnd_proc),
            hInstance: hinst,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("KeyState-RS Host"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            hinst,
            None,
        )
        .unwrap_or_default();

        state.hook_thread_hwnd.store(hwnd.0 as isize, Ordering::Release);
        let _ = ready_tx.send(());

        let nid = add_tray_icon(hwnd);
        register_hotkeys(hwnd, &state);
        sync_input_backends(hwnd, hinst, state.input_mode_fast());

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if !state.running.load(Ordering::Acquire) {
                break;
            }
        }

        // 정리: 훅 해제 & RawInput 해제 & HWND 0 초기화 후 파괴 ([M-2])
        sync_input_backends(hwnd, hinst, InputMode::AsyncPolling);
        unregister_hotkeys(hwnd);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        state.hook_thread_hwnd.store(0, Ordering::Release);
        if !hwnd.is_invalid() {
            let _ = DestroyWindow(hwnd);
        }
    }
}

unsafe fn register_hotkeys(hwnd: HWND, state: &SharedState) {
    let hk = state.config.read().hotkeys.clone();
    let mods: HOT_KEY_MODIFIERS = MOD_CONTROL | MOD_SHIFT;
    if hk.settings_vk != 0 {
        let _ = RegisterHotKey(hwnd, 1, mods, hk.settings_vk);
    }
    if hk.toggle_display_vk != 0 {
        let _ = RegisterHotKey(hwnd, 2, mods, hk.toggle_display_vk);
    }
    if hk.next_theme_vk != 0 {
        let _ = RegisterHotKey(hwnd, 3, mods, hk.next_theme_vk);
    }
    if hk.prev_theme_vk != 0 {
        let _ = RegisterHotKey(hwnd, 4, mods, hk.prev_theme_vk);
    }
    if hk.toggle_track_vk != 0 {
        let _ = RegisterHotKey(hwnd, 5, mods, hk.toggle_track_vk);
    }
    if hk.toggle_chart_vk != 0 {
        let _ = RegisterHotKey(hwnd, 6, mods, hk.toggle_chart_vk);
    }
}

unsafe fn unregister_hotkeys(hwnd: HWND) {
    for id in 1..=6 {
        let _ = UnregisterHotKey(hwnd, id);
    }
}

unsafe fn show_tray_popup(hwnd: HWND, state: &SharedState) {
    if let Ok(menu) = CreatePopupMenu() {
        let show_disp = state.config.read().show_display;
        let _ = AppendMenuW(menu, MF_STRING, 1, w!("설정 열기"));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            2,
            if show_disp {
                w!("오버레이 숨기기")
            } else {
                w!("오버레이 보이기")
            },
        );
        let _ = AppendMenuW(menu, MF_STRING, 3, w!("카운트 초기화"));
        if !is_running_as_admin() {
            let _ = AppendMenuW(menu, MF_STRING, 5, w!("관리자 권한으로 재실행 (게임용)"));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, 4, w!("종료"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, None);
        let _ = DestroyMenu(menu);

        match cmd.0 {
            1 => state.request_show_settings(),
            2 => {
                {
                    let mut cfg = state.config.write();
                    cfg.show_display = !cfg.show_display;
                }
                state.mark_config_changed();
                state.request_egui_repaint();
            }
            3 => {
                state.keys.reset_totals();
                state.overlay_wake.signal();
            }
            4 => {
                state.request_exit();
                PostQuitMessage(0);
            }
            5 => {
                if relaunch_as_admin() {
                    state.request_exit();
                    PostQuitMessage(0);
                }
            }
            _ => {}
        }
    }
}

/// RAWKEYBOARD 스캔코드 + E0 플래그를 정확한 Windows VK 코드(좌/우 구분, 넘패드/방향키 구분)로 정규화
unsafe fn normalize_raw_keyboard(kb: &RAWKEYBOARD) -> Option<(u32, bool)> {
    let mut vk = kb.VKey as u32;
    let make_code = kb.MakeCode as u32;
    let flags = kb.Flags as u32;

    if vk == 0 || vk == 255 || vk == 229 {
        return None;
    }

    let is_up = (flags & RI_KEY_BREAK) != 0;
    let is_e0 = (flags & RI_KEY_E0) != 0;

    vk = match vk {
        0x10 => {
            // [M-1] MapVirtualKeyW가 0x10을 반환할 경우 MakeCode로 좌/우 Shift 확정
            let mapped = MapVirtualKeyW(make_code, MAPVK_VSC_TO_VK_EX);
            if mapped == 0xA0 || mapped == 0xA1 {
                mapped
            } else if make_code == 0x36 {
                0xA1
            } else {
                0xA0
            }
        }
        0x11 => {
            if is_e0 { 0xA3 } else { 0xA2 }
        }
        0x12 => {
            if is_e0 { 0xA5 } else { 0xA4 }
        }
        0x2D if !is_e0 => 0x60,
        0x23 if !is_e0 => 0x61,
        0x28 if !is_e0 => 0x62,
        0x22 if !is_e0 => 0x63,
        0x25 if !is_e0 => 0x64,
        0x0C if !is_e0 => 0x65,
        0x27 if !is_e0 => 0x66,
        0x24 if !is_e0 => 0x67,
        0x26 if !is_e0 => 0x68,
        0x21 if !is_e0 => 0x69,
        0x2E if !is_e0 => 0x6E,
        _ => vk,
    };

    Some((vk, !is_up))
}

/// [C-2] 8바이트 정렬 스택 버퍼 + 페이로드 크기 검증으로 UB 및 OOB Read 완전 제거
unsafe fn process_wm_input(lp: LPARAM) {
    let hraw = HRAWINPUT(lp.0 as *mut core::ffi::c_void);
    let hdr_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
    let mut size = std::mem::size_of::<AlignedRawInputBuf>() as u32;
    let mut stack_buf = AlignedRawInputBuf([0u8; 256]);

    let read = GetRawInputData(
        hraw,
        RID_INPUT,
        Some(stack_buf.0.as_mut_ptr() as *mut core::ffi::c_void),
        &mut size,
        hdr_size,
    );
    if read == u32::MAX {
        return;
    }
    let min_kb_size =
        std::mem::size_of::<RAWINPUTHEADER>() + std::mem::size_of::<RAWKEYBOARD>();
    if (read as usize) < min_kb_size {
        return;
    }

    let raw = &*(stack_buf.0.as_ptr() as *const RAWINPUT);
    if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
        let kb = &raw.data.keyboard;
        if let Some((vk, pressed)) = normalize_raw_keyboard(kb) {
            handle_raw_input(vk, pressed, InputSource::RawInput);
        }
    }
}

unsafe extern "system" fn host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    let tb_msg = TASKBAR_CREATED_MSG.load(Ordering::Acquire);
    if tb_msg != 0 && msg == tb_msg {
        let _ = add_tray_icon(hwnd);
        return LRESULT(0);
    }

    if let Some(state) = GLOBAL_STATE.get() {
        match msg {
            WM_INPUT => {
                process_wm_input(lp);
                return LRESULT(0);
            }
            WM_REREGISTER_RAWINPUT | WM_UPDATE_INPUT_MODE => {
                let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
                sync_input_backends(hwnd, hinst, state.input_mode_fast());
                return LRESULT(0);
            }
            WM_TRAY_ICON => {
                let ev = (lp.0 & 0xFFFF) as u32;
                if ev == WM_LBUTTONUP {
                    state.request_show_settings();
                } else if ev == WM_RBUTTONUP {
                    show_tray_popup(hwnd, state);
                }
                return LRESULT(0);
            }
            WM_SHOW_TRAY_MENU => {
                show_tray_popup(hwnd, state);
                return LRESULT(0);
            }
            WM_RELOAD_HOTKEYS => {
                unregister_hotkeys(hwnd);
                register_hotkeys(hwnd, state);
                return LRESULT(0);
            }
            WM_HOTKEY => {
                match wp.0 {
                    1 => state.request_show_settings(),
                    2 => {
                        {
                            let mut cfg = state.config.write();
                            cfg.show_display = !cfg.show_display;
                        }
                        state.mark_config_changed();
                        state.request_egui_repaint();
                    }
                    3 => {
                        state.config.write().cycle_theme(1);
                        state.mark_config_changed();
                        state.request_egui_repaint();
                    }
                    4 => {
                        state.config.write().cycle_theme(-1);
                        state.mark_config_changed();
                        state.request_egui_repaint();
                    }
                    5 => {
                        {
                            let mut cfg = state.config.write();
                            cfg.show_history = !cfg.show_history;
                        }
                        state.mark_config_changed();
                        state.request_egui_repaint();
                    }
                    6 => {
                        {
                            let mut cfg = state.config.write();
                            cfg.show_chart = !cfg.show_chart;
                        }
                        state.mark_config_changed();
                        state.request_egui_repaint();
                    }
                    _ => {}
                }
                return LRESULT(0);
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

#[inline]
fn source_allowed(mode: InputMode, src: InputSource) -> bool {
    match mode {
        InputMode::RawInput => src == InputSource::RawInput,
        InputMode::RawInputAndHook => {
            src == InputSource::RawInput || src == InputSource::LowLevelHook
        }
        InputMode::AsyncPolling => src == InputSource::AsyncPolling,
        InputMode::LowLevelHook => src == InputSource::LowLevelHook,
    }
}

#[inline]
fn is_standalone_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5)
}

fn should_dedup_capture(vk: u32) -> bool {
    let Some(lock) = LAST_CAPTURE_DEDUP.get() else {
        return false;
    };
    let now = Instant::now();
    let mut guard = lock.lock();
    if let Some((prev_vk, prev_t)) = *guard {
        if prev_vk == vk && now.duration_since(prev_t) < Duration::from_millis(40) {
            return true;
        }
    }
    *guard = Some((vk, now));
    false
}

/// [H-3] 완전 Lock-free(config 락 미사용) 입력 핫패스
fn handle_raw_input(vk: u32, pressed: bool, src: InputSource) {
    let Some(state) = GLOBAL_STATE.get() else {
        return;
    };

    let mode = state.input_mode_fast();
    if !source_allowed(mode, src) {
        return;
    }

    // 1. UI 캡처 모드 체크 ([C-1] 하이브리드 모드에서도 타임스탬프 디바운싱으로 양쪽 소스 모두 지원)
    if pressed {
        let mut cap = state.capture_mode.lock();
        if let Some(cmode) = *cap {
            if mode == InputMode::RawInputAndHook && should_dedup_capture(vk) {
                return;
            }
            match cmode {
                CaptureMode::ContinuousKeyboard => {
                    if !is_mouse_vk(vk) {
                        state.captured_events.lock().push_back((cmode, vk));
                        state.request_egui_repaint();
                        return;
                    }
                }
                CaptureMode::HotkeySlot(_) => {
                    // [M-1] 글로벌 단축키 슬롯에는 마우스 버튼 및 단독 수식키 차단
                    if is_mouse_vk(vk) || is_standalone_modifier(vk) {
                        return;
                    }
                    *cap = None;
                    state.captured_events.lock().push_back((cmode, vk));
                    state.request_egui_repaint();
                    return;
                }
                CaptureMode::RebindKey(_) | CaptureMode::AddSingleAny => {
                    *cap = None;
                    state.captured_events.lock().push_back((cmode, vk));
                    state.request_egui_repaint();
                    return;
                }
            }
        }
    }

    // 2. [H-4] 녹화 단축키 체크 (Down 엣지에서만 1회 실행, Auto-repeat 무한 토글 차단)
    let rec_vk = state.record_vk_atomic.load(Ordering::Acquire);
    if rec_vk != 0 && vk == rec_vk {
        if pressed {
            if !RECORD_KEY_DOWN.swap(true, Ordering::AcqRel) {
                state.toggle_recording();
            }
        } else {
            RECORD_KEY_DOWN.store(false, Ordering::Release);
        }
        return;
    }

    // 3. 등록된 키인지 Lock-free 256비트 비트셋으로 O(1) 검사
    if state.is_vk_tracked(vk) {
        if state.keys.on_key_event(vk, pressed) {
            state.overlay_wake.signal();
            // [L-1] 설정 창이 열려 있을 때 실시간 키 눌림 미리보기만 이벤트 기반으로 갱신
            state.request_egui_repaint();
        }
    }
}

unsafe extern "system" fn kb_hook_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let ks = &*(lp.0 as *const KBDLLHOOKSTRUCT);
        if ks.vkCode != 229 {
            let msg = wp.0 as u32;
            let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if !(msg == WM_KEYDOWN && (ks.flags.0 & LLKHF_UP.0) != 0) {
                handle_raw_input(ks.vkCode, pressed, InputSource::LowLevelHook);
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // [H-2] WM_MOUSEMOVE를 최우선 즉시 통과시켜 마우스 이동 지연 제거
    let msg = wp.0 as u32;
    if code >= 0 && msg != WM_MOUSEMOVE {
        let ms = &*(lp.0 as *const MSLLHOOKSTRUCT);
        let mapped = match msg {
            WM_LBUTTONDOWN => Some((0x01u32, true)),
            WM_LBUTTONUP => Some((0x01u32, false)),
            WM_RBUTTONDOWN => Some((0x02u32, true)),
            WM_RBUTTONUP => Some((0x02u32, false)),
            WM_MBUTTONDOWN => Some((0x04u32, true)),
            WM_MBUTTONUP => Some((0x04u32, false)),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let xbtn = (ms.mouseData >> 16) & 0xFFFF;
                let vk = if xbtn == 1 {
                    0x05
                } else if xbtn == 2 {
                    0x06
                } else {
                    0
                };
                if vk != 0 {
                    Some((vk, msg == WM_XBUTTONDOWN))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some((vk, pressed)) = mapped {
            handle_raw_input(vk, pressed, InputSource::LowLevelHook);
        }
    }
    CallNextHookEx(None, code, wp, lp)
}
