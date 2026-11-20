use crate::config::{storage, AppConfig, InputMode};
use crate::input::hook::{
    is_running_as_admin, post_reload_hotkeys, post_update_input_mode, relaunch_as_admin,
};
use crate::input::keycode::vk_to_label;
use crate::state::{CaptureMode, HotkeySlot, SharedState};
use egui::{Color32, Ui};
use std::sync::Arc;

pub struct GeneralTabState {
    pub profile_name_input: String,
    pub cached_profiles: Vec<String>,
    pub status_msg: String,
}

impl Default for GeneralTabState {
    fn default() -> Self {
        Self {
            profile_name_input: String::new(),
            cached_profiles: storage::list_profiles(),
            status_msg: String::new(),
        }
    }
}

impl GeneralTabState {
    pub fn ui(&mut self, ui: &mut Ui, state: &Arc<SharedState>) {
        let mut changed = false;
        let mut input_mode_changed = false;
        let mut hotkeys_changed = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. 프로필 관리
                ui.heading("게임별 프로필 관리");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    let active_name = state.config.read().active_profile.clone();
                    ui.label(format!("현재 프로필: {active_name}"));
                    ui.horizontal(|ui| {
                        ui.label("새 프로필명:");
                        ui.text_edit_singleline(&mut self.profile_name_input);
                        if ui.button("현재 설정 저장").clicked() {
                            let name = if self.profile_name_input.trim().is_empty() {
                                active_name.clone()
                            } else {
                                self.profile_name_input.trim().to_string()
                            };
                            // [H-3] 쓰기 락을 디스크 I/O 전에 반드시 해제
                            let snapshot = {
                                let mut cfg = state.config.write();
                                cfg.active_profile = name.clone();
                                cfg.clone()
                            };
                            match storage::save_profile(&name, &snapshot) {
                                Ok(saved) => {
                                    self.status_msg = format!("프로필 '{saved}' 저장 완료");
                                    self.cached_profiles = storage::list_profiles();
                                    changed = true;
                                }
                                Err(e) => {
                                    self.status_msg = format!("저장 실패: {e}");
                                }
                            }
                        }
                        if ui.button("새로고침").clicked() {
                            self.cached_profiles = storage::list_profiles();
                        }
                    });

                    if !self.cached_profiles.is_empty() {
                        ui.separator();
                        let profiles = self.cached_profiles.clone();
                        for p in profiles {
                            ui.horizontal(|ui| {
                                ui.label(&p);
                                if ui.small_button("불러오기").clicked() {
                                    if let Some(loaded) = storage::load_profile(&p) {
                                        *state.config.write() = loaded;
                                        self.status_msg = format!("프로필 '{p}' 적용됨");
                                        changed = true;
                                        input_mode_changed = true;
                                        hotkeys_changed = true;
                                    }
                                }
                                if ui.small_button("삭제").clicked() {
                                    let _ = storage::delete_profile(&p);
                                    self.cached_profiles = storage::list_profiles();
                                    self.status_msg = format!("프로필 '{p}' 삭제됨");
                                }
                            });
                        }
                    }
                    if !self.status_msg.is_empty() {
                        ui.colored_label(Color32::from_rgb(110, 220, 150), &self.status_msg);
                    }
                });

                ui.add_space(10.0);

                // 2. 입력 감지 방식 & 게임 호환성
                ui.heading("입력 감지 방식 & 게임 호환성");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    {
                        let mut cfg = state.config.write();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("감지 백엔드:");
                            for (mode, label) in [
                                (InputMode::RawInput, "Raw Input (권장 · 게임 호환)"),
                                (InputMode::RawInputAndHook, "Raw Input + Hook (하이브리드)"),
                                (InputMode::AsyncPolling, "고속 폴링 (안티치트 대응)"),
                                (InputMode::LowLevelHook, "LL Hook 단독"),
                            ] {
                                if ui.selectable_value(&mut cfg.input_mode, mode, label).changed() {
                                    changed = true;
                                    input_mode_changed = true;
                                }
                            }
                        });
                    }

                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if is_running_as_admin() {
                            ui.colored_label(
                                Color32::from_rgb(110, 225, 150),
                                "★ 현재 관리자 권한으로 실행 중 (관리자 권한 게임 위에서도 입력 감지 가능)",
                            );
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(245, 185, 90),
                                "※ 일반 권한 실행 중 — 관리자 권한 게임 위에서는 Windows 보안(UIPI) 때문에 입력이 차단될 수 있습니다.",
                            );
                            if ui
                                .add(
                                    egui::Button::new("▲ 관리자 권한으로 재실행")
                                        .fill(Color32::from_rgb(160, 100, 32)),
                                )
                                .clicked()
                            {
                                if relaunch_as_admin() {
                                    state.request_exit();
                                }
                            }
                        }
                    });
                });

                ui.add_space(10.0);

                // 3. 오버레이 성능 & 동작
                ui.heading("오버레이 동작 & 성능");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    let mut cfg = state.config.write();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .checkbox(&mut cfg.always_on_top, "항상 위에 표시 (Topmost)")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .checkbox(&mut cfg.click_through, "마우스 클릭 투과 (Click-through)")
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("렌더링 목표 FPS:");
                        for fps in [30u32, 60, 90, 120, 144, 240] {
                            if ui
                                .selectable_value(&mut cfg.fps, fps, format!("{fps}"))
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
                    ui.label("Direct2D + DirectComposition GPU 렌더링 및 완전 유휴 절전(0fps)이 적용되어 있습니다.");
                });

                ui.add_space(10.0);

                // 4. 글로벌 단축키
                ui.heading("글로벌 단축키");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    let hk = state.config.read().hotkeys.clone();
                    let cur_cap = *state.capture_mode.lock();

                    let slots: [(&str, HotkeySlot, u32, bool); 7] = [
                        ("설정 창 열기", HotkeySlot::Settings, hk.settings_vk, true),
                        (
                            "오버레이 표시/숨김",
                            HotkeySlot::ToggleDisplay,
                            hk.toggle_display_vk,
                            true,
                        ),
                        ("다음 테마", HotkeySlot::NextTheme, hk.next_theme_vk, true),
                        ("이전 테마", HotkeySlot::PrevTheme, hk.prev_theme_vk, true),
                        (
                            "히스토리 트랙 토글",
                            HotkeySlot::ToggleTrack,
                            hk.toggle_track_vk,
                            true,
                        ),
                        ("KPS 차트 토글", HotkeySlot::ToggleChart, hk.toggle_chart_vk, true),
                        (
                            "KPS 녹화 시작/중지 (단일 키)",
                            HotkeySlot::Record,
                            hk.record_vk,
                            false,
                        ),
                    ];

                    egui::Grid::new("hotkey_grid")
                        .num_columns(3)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            for (label, slot, vk, with_mods) in slots {
                                ui.label(label);
                                let key_str = if vk == 0 {
                                    "미지정".to_string()
                                } else if with_mods {
                                    format!("Ctrl + Shift + {}", vk_to_label(vk))
                                } else {
                                    vk_to_label(vk)
                                };
                                ui.strong(key_str);

                                let capturing = cur_cap == Some(CaptureMode::HotkeySlot(slot));
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            [108.0, 22.0],
                                            egui::Button::new(if capturing {
                                                "키 입력 대기..."
                                            } else {
                                                "변경"
                                            }),
                                        )
                                        .clicked()
                                    {
                                        let mut cap = state.capture_mode.lock();
                                        *cap = if capturing {
                                            None
                                        } else {
                                            Some(CaptureMode::HotkeySlot(slot))
                                        };
                                    }
                                    if ui
                                        .add_enabled(vk != 0, egui::Button::new("해제").small())
                                        .clicked()
                                    {
                                        set_hotkey_slot(state, slot, 0);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });

                ui.add_space(10.0);

                // 5. KPS 녹화 & 초기화
                ui.heading("입력 데이터 녹화 & 관리");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    let (rec_active, rec_secs, last_saved) = {
                        let rec = state.recording.lock();
                        let secs = rec
                            .start_instant
                            .map(|s| s.elapsed().as_secs())
                            .unwrap_or(0);
                        (rec.active, secs, rec.last_saved_path.clone())
                    };

                    ui.horizontal_wrapped(|ui| {
                        let btn = egui::Button::new(if rec_active {
                            format!("■ 녹화 중지 ({rec_secs}초)")
                        } else {
                            "● KPS 녹화 시작".to_string()
                        })
                        .fill(if rec_active {
                            Color32::from_rgb(190, 50, 60)
                        } else {
                            Color32::from_rgb(45, 110, 80)
                        });
                        // [UX-7] 고정 버튼 폭으로 타이머 초 증가 시 옆 버튼 흔들림 방지
                        if ui.add_sized([162.0, 26.0], btn).clicked() {
                            state.toggle_recording();
                        }
                        if ui.button("모든 카운트 초기화").clicked() {
                            state.keys.reset_totals();
                            state.overlay_wake.signal();
                        }
                        if ui.button("전체 설정 기본값 복원").clicked() {
                            *state.config.write() = AppConfig::default();
                            changed = true;
                            input_mode_changed = true;
                            hotkeys_changed = true;
                        }
                    });
                    if let Some(path) = last_saved {
                        ui.label(format!("최근 저장된 녹화: {path}"));
                    }
                });
            });

        if changed {
            state.mark_config_changed();
        }
        if input_mode_changed {
            post_update_input_mode(state);
        }
        if hotkeys_changed {
            post_reload_hotkeys(state);
        }
    }
}

pub fn set_hotkey_slot(state: &SharedState, slot: HotkeySlot, vk: u32) {
    {
        let mut cfg = state.config.write();
        match slot {
            HotkeySlot::Settings => cfg.hotkeys.settings_vk = vk,
            HotkeySlot::ToggleDisplay => cfg.hotkeys.toggle_display_vk = vk,
            HotkeySlot::NextTheme => cfg.hotkeys.next_theme_vk = vk,
            HotkeySlot::PrevTheme => cfg.hotkeys.prev_theme_vk = vk,
            HotkeySlot::ToggleTrack => cfg.hotkeys.toggle_track_vk = vk,
            HotkeySlot::ToggleChart => cfg.hotkeys.toggle_chart_vk = vk,
            HotkeySlot::Record => cfg.hotkeys.record_vk = vk,
        }
    }
    state.mark_config_changed();
    post_reload_hotkeys(state);
}
