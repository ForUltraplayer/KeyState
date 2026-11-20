#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod input;
mod overlay;
mod state;
mod ui;

use crate::config::storage;
use crate::input::hook::spawn_hook_and_tray_thread;
use crate::overlay::spawn_overlay_thread;
use crate::state::SharedState;
use crate::ui::SettingsApp;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_QUIT};

fn main() {
    // [M-5] 어떤 창도 생성되기 전에 Per-Monitor DPI V2 명시 설정
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let cfg = storage::load_config();
    let state = SharedState::new(cfg);

    // 1. 전용 스레드: Raw Input + 동적 LL 훅 + 폴링 + 시스템 트레이 + 글로벌 단축키
    let (hook_handle, poller_handle) = spawn_hook_and_tray_thread(state.clone());

    // 2. 전용 스레드: Direct2D + DirectComposition GPU 오버레이 (메인 + 차트)
    let overlay_handle = spawn_overlay_thread(state.clone());

    // 3. 메인 스레드: 모던 egui 설정 & 비주얼 키보드 에디터
    let app_icon = egui::IconData {
        rgba: include_bytes!("../assets/icon_64.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KeyState-RS 설정")
            .with_icon(app_icon)
            .with_inner_size([1020.0, 680.0])
            .with_min_inner_size([780.0, 520.0]),
        ..Default::default()
    };

    let app_state = state.clone();
    let _ = eframe::run_native(
        "KeyState-RS 설정",
        native_options,
        Box::new(move |cc| Ok(Box::new(SettingsApp::new(cc, app_state)))),
    );

    // [M-2] 종료 시 모든 백그라운드 스레드 안전하게 종료 및 조인
    state.running.store(false, Ordering::Release);
    state.overlay_wake.signal();
    let host_hwnd = state.hook_thread_hwnd.load(Ordering::Acquire);
    if host_hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(host_hwnd as *mut core::ffi::c_void),
                WM_QUIT,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    let _ = overlay_handle.join();
    let _ = hook_handle.join();
    let _ = poller_handle.join();
    state.save_now();
}
