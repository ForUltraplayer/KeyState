pub mod canvas;
pub mod general_tab;
pub mod style_tab;
pub mod track_tab;

use crate::input::hook::{is_running_as_admin, post_reregister_raw_input, relaunch_as_admin};
use crate::input::keycode::vk_to_label;
use crate::state::{CaptureMode, SharedState};
use crate::ui::canvas::CanvasEditor;
use crate::ui::general_tab::{set_hotkey_slot, GeneralTabState};
use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, RichText};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Layout,
    Style,
    Track,
    General,
}

pub struct SettingsApp {
    state: Arc<SharedState>,
    tab: Tab,
    canvas: CanvasEditor,
    general: GeneralTabState,
    hwnd_captured: bool,
    raw_input_reregister_frames: u8,
}

impl SettingsApp {
    pub fn new(cc: &eframe::CreationContext<'_>, state: Arc<SharedState>) -> Self {
        install_korean_fonts(&cc.egui_ctx);
        configure_modern_visuals(&cc.egui_ctx);

        *state.egui_ctx.lock() = Some(cc.egui_ctx.clone());

        // [C-1] eframe/winit 윈도우 초기화 직후 키보드 Raw Input 타겟을 호스트 창으로 재등록
        post_reregister_raw_input(&state);

        let mut canvas = CanvasEditor::default();
        if let Some(first_id) = state.config.read().keys.first().map(|k| k.id) {
            canvas.select_only(first_id);
        }

        Self {
            state,
            tab: Tab::Layout,
            canvas,
            general: GeneralTabState::default(),
            hwnd_captured: false,
            raw_input_reregister_frames: 2,
        }
    }

    fn process_captured_inputs(&mut self) {
        let events: Vec<(CaptureMode, u32)> = {
            let mut q = self.state.captured_events.lock();
            q.drain(..).collect()
        };
        if events.is_empty() {
            return;
        }
        let mut changed = false;
        for (mode, vk) in events {
            match mode {
                CaptureMode::ContinuousKeyboard | CaptureMode::AddSingleAny => {
                    let mut cfg = self.state.config.write();
                    self.canvas.add_key_to_config(&mut cfg, vk);
                    changed = true;
                }
                CaptureMode::RebindKey(id) => {
                    let mut cfg = self.state.config.write();
                    if let Some(k) = cfg.keys.iter_mut().find(|k| k.id == id) {
                        k.vk = vk;
                        k.label = vk_to_label(vk);
                        changed = true;
                    }
                }
                CaptureMode::HotkeySlot(slot) => {
                    set_hotkey_slot(&self.state, slot, vk);
                }
            }
        }
        if changed {
            self.state.mark_config_changed();
        }
    }
}

/// [L-5] 현재 프로세스 PID + 제목이 모두 일치하는 윈도우만 찾아 설정 창 HWND 안전 획득
fn find_own_settings_hwnd() -> isize {
    struct SearchCtx {
        target_pid: u32,
        found: isize,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lp: LPARAM) -> BOOL {
        let ctx = &mut *(lp.0 as *mut SearchCtx);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == ctx.target_pid {
            let mut buf = [0u16; 128];
            let len = GetWindowTextW(hwnd, &mut buf) as usize;
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len]);
                if title == "KeyState-RS 설정" {
                    ctx.found = hwnd.0 as isize;
                    return BOOL(0);
                }
            }
        }
        BOOL(1)
    }

    let mut ctx = SearchCtx {
        target_pid: unsafe { GetCurrentProcessId() },
        found: 0,
    };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut SearchCtx as isize));
    }
    ctx.found
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // [C-1] winit 첫 프레임 이벤트 루프 정착 후 Raw Input 확실하게 재등록
        if self.raw_input_reregister_frames > 0 {
            self.raw_input_reregister_frames -= 1;
            if self.raw_input_reregister_frames == 0 {
                post_reregister_raw_input(&self.state);
            }
        }

        if !self.hwnd_captured {
            let hwnd = find_own_settings_hwnd();
            if hwnd != 0 {
                self.state.settings_hwnd.store(hwnd, Ordering::Release);
                self.hwnd_captured = true;
            }
        }

        // 창 닫기(X) 시 종료 대신 트레이로 숨김 + [L-1] 캡처 모드 자동 해제
        if ctx.input(|i| i.viewport().close_requested())
            && !self.state.should_exit.load(Ordering::Acquire)
        {
            self.state.reset_capture();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.state.save_if_dirty();
            return;
        }

        if self.state.show_settings_req.swap(false, Ordering::AcqRel) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        self.process_captured_inputs();

        // 상단 헤더 & 탭 & 빠른 제어 바 ([UX-7] 모노스페이스 고정폭 FPS로 좌우 흔들림 제거)
        egui::TopBottomPanel::top("top_bar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(Color32::from_rgb(22, 24, 31))
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("KeyState-RS")
                            .size(17.0)
                            .strong()
                            .color(Color32::from_rgb(115, 195, 255)),
                    );
                    ui.separator();

                    for (t, label) in [
                        (Tab::Layout, "키 배치"),
                        (Tab::Style, "스타일 & 테마"),
                        (Tab::Track, "트랙 & 통계"),
                        (Tab::General, "프로필 & 일반"),
                    ] {
                        if ui
                            .selectable_label(self.tab == t, RichText::new(label).size(13.5))
                            .clicked()
                        {
                            if self.tab != t {
                                // [L-1] 탭 전환 시 활성 캡처 모드 자동 해제
                                self.state.reset_capture();
                                self.tab = t;
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new("종료").fill(Color32::from_rgb(130, 42, 50)))
                            .clicked()
                        {
                            self.state.request_exit();
                        }

                        let mut changed = false;
                        {
                            let mut cfg = self.state.config.write();
                            if ui.selectable_label(cfg.click_through, "클릭 투과").clicked() {
                                cfg.click_through = !cfg.click_through;
                                changed = true;
                            }
                            if ui.selectable_label(cfg.show_display, "오버레이 ON").clicked() {
                                cfg.show_display = !cfg.show_display;
                                changed = true;
                            }
                        }
                        if changed {
                            self.state.mark_config_changed();
                        }

                        let fps = self.state.last_overlay_fps.load(Ordering::Relaxed);
                        ui.label(
                            RichText::new(format!("GPU {fps:>3}fps"))
                                .small()
                                .monospace()
                                .color(Color32::from_rgb(130, 220, 155)),
                        );

                        if is_running_as_admin() {
                            ui.label(
                                RichText::new("★ 관리자")
                                    .small()
                                    .color(Color32::from_rgb(110, 230, 150)),
                            );
                        } else if ui
                            .add(
                                egui::Button::new("▲ 관리자 실행")
                                    .fill(Color32::from_rgb(150, 95, 30)),
                            )
                            .on_hover_text(
                                "관리자 권한으로 실행 중인 게임 위에서 키 입력이 감지되지 않으면 클릭해 관리자 권한으로 재실행하세요.",
                            )
                            .clicked()
                        {
                            if relaunch_as_admin() {
                                self.state.request_exit();
                            }
                        }
                    });
                });
            });

        // [UX-5] 키 배치 탭은 egui 정식 SidePanel + TopBottomPanel + CentralPanel 도킹으로 완전 반응형 구성
        match self.tab {
            Tab::Layout => {
                egui::TopBottomPanel::top("layout_toolbar")
                    .frame(
                        egui::Frame::side_top_panel(&ctx.style())
                            .fill(Color32::from_rgb(20, 22, 29))
                            .inner_margin(egui::Margin::symmetric(12, 8)),
                    )
                    .show(ctx, |ui| {
                        self.canvas.ui_toolbar(ui, &self.state);
                    });

                egui::SidePanel::right("inspector_panel")
                    .resizable(true)
                    .default_width(275.0)
                    .width_range(235.0..=360.0)
                    .frame(
                        egui::Frame::side_top_panel(&ctx.style())
                            .fill(Color32::from_rgb(19, 21, 28))
                            .inner_margin(egui::Margin::same(10)),
                    )
                    .show(ctx, |ui| {
                        self.canvas.ui_inspector(ui, &self.state);
                    });

                egui::TopBottomPanel::bottom("registered_keys_panel")
                    .frame(
                        egui::Frame::side_top_panel(&ctx.style())
                            .fill(Color32::from_rgb(20, 22, 29))
                            .inner_margin(egui::Margin::symmetric(12, 7)),
                    )
                    .show(ctx, |ui| {
                        self.canvas.ui_registered_keys(ui, &self.state);
                    });

                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::central_panel(&ctx.style())
                            .fill(Color32::from_rgb(15, 17, 23))
                            .inner_margin(egui::Margin::same(10)),
                    )
                    .show(ctx, |ui| {
                        self.canvas.ui_canvas_center(ui, &self.state);
                    });
            }
            Tab::Style | Tab::Track | Tab::General => {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::central_panel(&ctx.style())
                            .fill(Color32::from_rgb(17, 19, 25))
                            .inner_margin(egui::Margin::same(12)),
                    )
                    .show(ctx, |ui| match self.tab {
                        Tab::Style => style_tab::ui(ui, &self.state),
                        Tab::Track => track_tab::ui(ui, &self.state),
                        Tab::General => self.general.ui(ui, &self.state),
                        Tab::Layout => {}
                    });
            }
        }

        // [L-1] 불필요한 50ms 상시 리페인트 제거: 일반 탭 녹화 중일 때만 1초마다 경과 시간 갱신
        if self.tab == Tab::General && self.state.recording.lock().active {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }
}

fn install_korean_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\malgunbd.ttf",
        r"C:\Windows\Fonts\gulim.ttc",
        r"C:\Windows\Fonts\NotoSansKR-Regular.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("korean".to_owned(), Arc::new(FontData::from_owned(bytes)));
            if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
                list.insert(0, "korean".to_owned());
            }
            if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
                list.push("korean".to_owned());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

fn configure_modern_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(20, 22, 29);
    visuals.panel_fill = Color32::from_rgb(17, 19, 25);
    visuals.extreme_bg_color = Color32::from_rgb(12, 14, 19);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.selection.bg_fill = Color32::from_rgb(55, 115, 200);
    ctx.set_visuals(visuals);
}
