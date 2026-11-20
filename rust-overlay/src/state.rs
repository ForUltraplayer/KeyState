use crate::config::{storage, AppConfig, InputMode};
use crate::input::state::KeyStateManager;
use chrono::Local;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CreateEventW, SetEvent};
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// 연속 키보드 키 추가 (마우스 클릭은 UI 조작을 위해 무시)
    ContinuousKeyboard,
    /// 단일 키/마우스 버튼 캡처 (특정 키 ID의 VK 변경)
    RebindKey(u64),
    /// 단일 키/마우스 버튼 1개 추가
    AddSingleAny,
    /// 글로벌 단축키 지정 (Ctrl+Shift+[VK])
    HotkeySlot(HotkeySlot),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeySlot {
    Settings,
    ToggleDisplay,
    NextTheme,
    PrevTheme,
    ToggleTrack,
    ToggleChart,
    Record,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingKeySample {
    pub total: u64,
    pub kps: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingSnapshot {
    pub time_ms: u64,
    pub total_presses: u64,
    pub total_kps: u32,
    pub keys: Vec<RecordingKeySample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingFile {
    pub version: u32,
    pub started_at: String,
    pub duration_ms: u64,
    pub keys: Vec<String>,
    pub data: Vec<RecordingSnapshot>,
}

#[derive(Default)]
pub struct RecordingState {
    pub active: bool,
    pub start_instant: Option<Instant>,
    pub started_at_str: String,
    pub last_sample_instant: Option<Instant>,
    pub key_labels: Vec<String>,
    pub key_vks: Vec<u32>,
    pub snapshots: Vec<RecordingSnapshot>,
    pub last_saved_path: Option<String>,
}

pub struct WakeEvent {
    handle: isize,
}

impl WakeEvent {
    pub fn new() -> Self {
        let h = unsafe { CreateEventW(None, false, false, None).expect("CreateEventW failed") };
        Self { handle: h.0 as isize }
    }

    pub fn raw(&self) -> HANDLE {
        HANDLE(self.handle as *mut core::ffi::c_void)
    }

    pub fn signal(&self) {
        unsafe {
            let _ = SetEvent(self.raw());
        }
    }
}

impl Drop for WakeEvent {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                let _ = CloseHandle(self.raw());
            }
        }
    }
}

pub struct SharedState {
    pub config: RwLock<AppConfig>,
    pub keys: KeyStateManager,
    pub config_version: AtomicU64,
    pub config_dirty: AtomicBool,

    // [H-3] 입력 훅/폴링 핫패스용 Lock-free 미러 Atomics
    pub input_mode_atomic: AtomicU8,
    pub record_vk_atomic: AtomicU32,
    pub tracked_vks: [AtomicU64; 4], // VK 0..255 비트셋
    pub any_mouse_tracked: AtomicBool,

    pub capture_mode: Mutex<Option<CaptureMode>>,
    pub captured_events: Mutex<VecDeque<(CaptureMode, u32)>>,

    pub recording: Mutex<RecordingState>,
    pub overlay_wake: WakeEvent,
    pub hook_thread_hwnd: AtomicIsize,

    pub egui_ctx: Mutex<Option<egui::Context>>,
    pub settings_hwnd: AtomicIsize,
    pub show_settings_req: AtomicBool,
    pub should_exit: AtomicBool,
    pub running: AtomicBool,
    pub last_overlay_fps: AtomicU64,
}

impl SharedState {
    pub fn new(config: AppConfig) -> Arc<Self> {
        let keys = KeyStateManager::new();
        // [M-4] 저장된 키별 누적 카운트 복원
        for k in &config.keys {
            if k.total_presses > 0 {
                keys.set_total(k.vk, k.total_presses);
            }
        }

        let state = Arc::new(Self {
            input_mode_atomic: AtomicU8::new(config.input_mode.to_u8()),
            record_vk_atomic: AtomicU32::new(config.hotkeys.record_vk),
            tracked_vks: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            any_mouse_tracked: AtomicBool::new(false),
            config: RwLock::new(config),
            keys,
            config_version: AtomicU64::new(1),
            config_dirty: AtomicBool::new(false),
            capture_mode: Mutex::new(None),
            captured_events: Mutex::new(VecDeque::new()),
            recording: Mutex::new(RecordingState::default()),
            overlay_wake: WakeEvent::new(),
            hook_thread_hwnd: AtomicIsize::new(0),
            egui_ctx: Mutex::new(None),
            settings_hwnd: AtomicIsize::new(0),
            show_settings_req: AtomicBool::new(false),
            should_exit: AtomicBool::new(false),
            running: AtomicBool::new(true),
            last_overlay_fps: AtomicU64::new(0),
        });
        state.sync_input_atomics();
        state
    }

    /// [H-3] 설정 변경 시 입력 핫패스용 Atomic 캐시 동기화 (config.read 락을 짧게 1회만 잡음)
    pub fn sync_input_atomics(&self) {
        let cfg = self.config.read();
        self.input_mode_atomic
            .store(cfg.input_mode.to_u8(), Ordering::Release);
        self.record_vk_atomic
            .store(cfg.hotkeys.record_vk, Ordering::Release);

        let mut masks = [0u64; 4];
        let mut mouse_any = false;
        for k in &cfg.keys {
            if k.vk < 256 {
                let idx = (k.vk / 64) as usize;
                let bit = k.vk % 64;
                masks[idx] |= 1u64 << bit;
                if matches!(k.vk, 0x01 | 0x02 | 0x04 | 0x05 | 0x06) {
                    mouse_any = true;
                }
            }
        }
        for i in 0..4 {
            self.tracked_vks[i].store(masks[i], Ordering::Release);
        }
        self.any_mouse_tracked.store(mouse_any, Ordering::Release);
    }

    #[inline]
    pub fn input_mode_fast(&self) -> InputMode {
        InputMode::from_u8(self.input_mode_atomic.load(Ordering::Acquire))
    }

    #[inline]
    pub fn is_vk_tracked(&self, vk: u32) -> bool {
        if vk >= 256 {
            return false;
        }
        let idx = (vk / 64) as usize;
        let bit = vk % 64;
        (self.tracked_vks[idx].load(Ordering::Acquire) & (1u64 << bit)) != 0
    }

    /// [M-3] config와 config_version을 락 안에서 원자적으로 함께 스냅샷
    pub fn read_config_with_version(&self) -> (AppConfig, u64) {
        let cfg = self.config.read();
        let ver = self.config_version.load(Ordering::Acquire);
        (cfg.clone(), ver)
    }

    pub fn mark_config_changed(&self) {
        self.sync_input_atomics();
        self.config_version.fetch_add(1, Ordering::AcqRel);
        self.config_dirty.store(true, Ordering::Release);
        self.overlay_wake.signal();
    }

    pub fn reset_capture(&self) {
        *self.capture_mode.lock() = None;
    }

    pub fn request_egui_repaint(&self) {
        if let Some(ctx) = self.egui_ctx.lock().as_ref() {
            ctx.request_repaint();
        }
    }

    pub fn request_show_settings(&self) {
        self.show_settings_req.store(true, Ordering::Release);
        let hwnd_raw = self.settings_hwnd.load(Ordering::Acquire);
        if hwnd_raw != 0 {
            let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut core::ffi::c_void);
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
        }
        if let Some(ctx) = self.egui_ctx.lock().as_ref() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }

    pub fn request_exit(&self) {
        self.should_exit.store(true, Ordering::Release);
        self.running.store(false, Ordering::Release);
        self.overlay_wake.signal();
        if let Some(ctx) = self.egui_ctx.lock().as_ref() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx.request_repaint();
        }
    }

    /// [M-4] 누적 카운트까지 동기화하여 설정 파일 원자적 저장
    pub fn save_if_dirty(&self) {
        if self.config_dirty.swap(false, Ordering::AcqRel) {
            self.save_now();
        }
    }

    pub fn save_now(&self) {
        let mut cfg = self.config.read().clone();
        for k in &mut cfg.keys {
            k.total_presses = self.keys.get_total(k.vk);
        }
        let _ = storage::save_config(&cfg);
    }

    /// [H-4] 녹화 종료 시 파일 직렬화/디스크 쓰기를 백그라운드 스레드로 위임 (입력 스레드 블로킹 제거)
    pub fn toggle_recording(self: &Arc<Self>) {
        let save_payload = {
            let mut rec = self.recording.lock();
            if !rec.active {
                let cfg = self.config.read();
                rec.active = true;
                rec.start_instant = Some(Instant::now());
                rec.started_at_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                rec.last_sample_instant = None;
                rec.key_labels = cfg.keys.iter().map(|k| k.label.clone()).collect();
                rec.key_vks = cfg.keys.iter().map(|k| k.vk).collect();
                rec.snapshots.clear();
                None
            } else {
                rec.active = false;
                let snapshots = std::mem::take(&mut rec.snapshots);
                if snapshots.is_empty() {
                    None
                } else {
                    Some(RecordingFile {
                        version: 1,
                        started_at: rec.started_at_str.clone(),
                        duration_ms: snapshots.last().map(|s| s.time_ms).unwrap_or(0),
                        keys: rec.key_labels.clone(),
                        data: snapshots,
                    })
                }
            }
        };

        if let Some(file_data) = save_payload {
            let state_clone = self.clone();
            std::thread::spawn(move || {
                let dir = storage::records_dir();
                let _ = fs::create_dir_all(&dir);
                let ts = Local::now().format("%Y%m%d_%H%M%S");
                let path = dir.join(format!("KeyStateRecording_{ts}.json"));
                if let Ok(json) = serde_json::to_string_pretty(&file_data) {
                    if fs::write(&path, json).is_ok() {
                        state_clone.recording.lock().last_saved_path =
                            Some(path.display().to_string());
                        state_clone.request_egui_repaint();
                    }
                }
            });
        }

        self.request_egui_repaint();
    }

    pub fn tick_recording(&self) {
        let mut rec = self.recording.lock();
        if !rec.active {
            return;
        }
        let now = Instant::now();
        let should_sample = match rec.last_sample_instant {
            None => true,
            Some(prev) => now.duration_since(prev).as_millis() >= 1000,
        };
        if !should_sample {
            return;
        }
        rec.last_sample_instant = Some(now);
        let time_ms = rec
            .start_instant
            .map(|s| now.duration_since(s).as_millis() as u64)
            .unwrap_or(0);

        // [L-2] 중복 VK 이중 합산 방지
        let mut unique_vks = rec.key_vks.clone();
        unique_vks.sort_unstable();
        unique_vks.dedup();
        let sum_total: u64 = unique_vks.iter().map(|&vk| self.keys.get_total(vk)).sum();
        let sum_kps: u32 = unique_vks.iter().map(|&vk| self.keys.get_kps(vk) as u32).sum();

        let mut key_samples = Vec::with_capacity(rec.key_vks.len());
        for &vk in &rec.key_vks {
            key_samples.push(RecordingKeySample {
                total: self.keys.get_total(vk),
                kps: self.keys.get_kps(vk) as u32,
            });
        }
        rec.snapshots.push(RecordingSnapshot {
            time_ms,
            total_presses: sum_total,
            total_kps: sum_kps,
            keys: key_samples,
        });
    }
}
