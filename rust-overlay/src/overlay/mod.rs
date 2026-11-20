pub mod anim;
pub mod chart;
pub mod d2d;
pub mod display;

use crate::overlay::chart::ChartOverlay;
use crate::overlay::d2d::{is_device_lost, D2dShared};
use crate::overlay::display::DisplayOverlay;
use crate::state::SharedState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW, TranslateMessage, MSG,
    MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT,
};

pub fn spawn_overlay_thread(state: Arc<SharedState>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("gpu-overlay".into())
        .spawn(move || run_overlay_loop(state))
        .expect("Failed to spawn overlay thread")
}

/// 필드 선언 순서에 따라 chart -> display -> shared 순으로 안전하게 Drop ([H-6])
struct OverlayPipeline {
    chart: ChartOverlay,
    display: DisplayOverlay,
    shared: D2dShared,
}

impl OverlayPipeline {
    fn new(state: &Arc<SharedState>) -> Option<Self> {
        let shared = D2dShared::new().ok()?;
        let display = DisplayOverlay::new(&shared, state.clone()).ok()?;
        let chart = ChartOverlay::new(&shared, state.clone()).ok()?;
        Some(Self {
            chart,
            display,
            shared,
        })
    }
}

fn run_overlay_loop(state: Arc<SharedState>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        timeBeginPeriod(1);
    }

    let mut pipeline = match OverlayPipeline::new(&state) {
        Some(p) => Some(p),
        None => {
            eprintln!("Direct2D/DirectComposition overlay init failed");
            unsafe {
                timeEndPeriod(1);
                CoUninitialize();
            }
            return;
        }
    };

    let mut last_cfg_ver = 0u64;
    let mut last_save_check = Instant::now();
    let mut fps_counter = 0u64;
    let mut fps_window_start = Instant::now();
    let mut prev_active = true;
    let mut force_render = true;

    while state.running.load(Ordering::Acquire) {
        // [H-1] 루프 시작(대기 해제 직후) 시점을 프레임 시작으로 기록해 정확한 FPS 페이싱 보장
        let frame_start = Instant::now();

        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if !state.running.load(Ordering::Acquire) {
            break;
        }

        let now = Instant::now();
        // [M-3] config와 config_version을 락 안에서 원자적으로 함께 스냅샷
        let (cfg, cur_ver) = state.read_config_with_version();
        let cfg_changed = cur_ver != last_cfg_ver;
        if cfg_changed {
            last_cfg_ver = cur_ver;
        }

        state.tick_recording();

        if now.duration_since(last_save_check) >= Duration::from_millis(400) {
            last_save_check = now;
            state.save_if_dirty();
        }

        let Some(p) = pipeline.as_mut() else {
            break;
        };

        let dragging = p.display.is_dragging() || p.chart.is_dragging();
        let need_render = prev_active || cfg_changed || dragging || force_render;
        let mut device_lost = false;
        let mut active = false;

        // [H-5] 완전 유휴 시 GPU 렌더링 및 Present 완전 스킵 (진정한 GPU 0fps 절전!)
        if need_render {
            force_render = false;
            match p
                .display
                .update_and_render(&p.shared, &cfg, &state.keys, cfg_changed, now)
            {
                Ok(a) => {
                    if a {
                        active = true;
                    }
                }
                Err(e) if is_device_lost(&e) => {
                    device_lost = true;
                }
                Err(_) => {}
            }

            if !device_lost {
                let disp_rect = p.display.last_window_rect();
                match p.chart.update_and_render(
                    &p.shared,
                    &cfg,
                    &state.keys,
                    disp_rect,
                    cfg_changed,
                    now,
                ) {
                    Ok(a) => {
                        if a {
                            active = true;
                        }
                    }
                    Err(e) if is_device_lost(&e) => {
                        device_lost = true;
                    }
                    Err(_) => {}
                }
            }

            if device_lost {
                drop(pipeline.take());
                std::thread::sleep(Duration::from_millis(120));
                pipeline = OverlayPipeline::new(&state);
                if pipeline.is_some() {
                    last_cfg_ver = 0;
                    force_render = true;
                    prev_active = true;
                    continue;
                } else {
                    break;
                }
            }

            fps_counter += 1;
        }

        let elapsed_fps = now.duration_since(fps_window_start);
        if elapsed_fps >= Duration::from_secs(1) {
            state.last_overlay_fps.store(fps_counter, Ordering::Relaxed);
            fps_counter = 0;
            fps_window_start = now;
        }

        prev_active = active || dragging;

        let target_fps = cfg.fps.clamp(24, 240);
        let frame_dur = Duration::from_micros(1_000_000 / target_fps as u64);

        let wait_ms = if prev_active || cfg_changed {
            let spent = Instant::now().saturating_duration_since(frame_start);
            if spent < frame_dur {
                ((frame_dur - spent).as_millis() as u32).max(1)
            } else {
                0
            }
        } else {
            250
        };

        if wait_ms > 0 {
            let handles: [HANDLE; 1] = [state.overlay_wake.raw()];
            let wait_res = unsafe {
                MsgWaitForMultipleObjectsEx(
                    Some(&handles),
                    wait_ms,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                )
            };
            if wait_res == WAIT_OBJECT_0 {
                force_render = true;
            }
        }
    }

    state.save_now();
    drop(pipeline);

    unsafe {
        timeEndPeriod(1);
        CoUninitialize();
    }
}
