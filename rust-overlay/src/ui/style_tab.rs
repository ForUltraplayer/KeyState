use crate::config::ThemePreset;
use crate::state::SharedState;
use crate::ui::canvas::color_row;
use egui::{Color32, CornerRadius, Sense, Stroke, StrokeKind, Ui, Vec2};

const COMMON_FONTS: &[&str] = &[
    "Segoe UI",
    "Malgun Gothic",
    "Consolas",
    "Arial",
    "Bahnschrift",
    "Cascadia Mono",
    "Impact",
    "Verdana",
];

pub fn ui(ui: &mut Ui, state: &SharedState) {
    let mut changed = false;
    {
        let mut cfg = state.config.write();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("외형 & 애니메이션");
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    egui::Grid::new("style_grid")
                        .num_columns(2)
                        .spacing([14.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("오버레이 전체 투명도");
                            if ui
                                .add(
                                    egui::Slider::new(&mut cfg.overlay_opacity, 0.1..=1.0)
                                        .fixed_decimals(2),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("모서리 둥글기 (Radius)");
                            if ui
                                .add(egui::Slider::new(&mut cfg.key_radius, 0.0..=32.0).suffix("px"))
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("테두리 굵기");
                            if ui
                                .add(egui::Slider::new(&mut cfg.key_border_w, 0.0..=8.0).suffix("px"))
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("키 글꼴 & 크기");
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("font_combo")
                                    .selected_text(&cfg.key_font_name)
                                    .width(150.0)
                                    .show_ui(ui, |ui| {
                                        for &f in COMMON_FONTS {
                                            if ui
                                                .selectable_value(
                                                    &mut cfg.key_font_name,
                                                    f.to_string(),
                                                    f,
                                                )
                                                .changed()
                                            {
                                                changed = true;
                                            }
                                        }
                                    });
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.key_font_size)
                                            .range(8.0..=64.0)
                                            .suffix("px"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();

                            ui.label("직접 폰트명 입력");
                            if ui.text_edit_singleline(&mut cfg.key_font_name).changed() {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("눌림 이징 시간 (Smoothstep)");
                            if ui
                                .add(
                                    egui::Slider::new(&mut cfg.press_ease_ms, 10.0..=250.0)
                                        .suffix("ms"),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("뗌 이징 시간 (Smoothstep)");
                            if ui
                                .add(
                                    egui::Slider::new(&mut cfg.release_ease_ms, 10.0..=350.0)
                                        .suffix("ms"),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();

                            ui.label("글로우 발광 강도");
                            if ui
                                .add(
                                    egui::Slider::new(&mut cfg.glow_intensity, 0.0..=1.5)
                                        .fixed_decimals(2),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();
                        });
                });

                ui.add_space(12.0);

                // 2. 반응형 카드 그리드 (창 폭에 맞춰 2~5열 자동 정렬, 가로 퍼짐 완전 해결)
                ui.horizontal(|ui| {
                    ui.heading("테마 프리셋");
                    ui.add_space(8.0);
                    if ui.button("+ 현재 색상으로 새 테마 추가").clicked() {
                        let first_key = cfg.keys.first().cloned().unwrap_or_default();
                        let idx = cfg.themes.len() + 1;
                        let chart_bg = cfg.chart_bg;
                        let chart_line = cfg.chart_line;
                        cfg.themes.push(ThemePreset {
                            name: format!("커스텀 테마 {idx}"),
                            font: first_key.color_font,
                            normal: first_key.color_normal,
                            press: first_key.color_press,
                            chart_bg,
                            chart_line,
                        });
                        cfg.active_theme = cfg.themes.len() - 1;
                        changed = true;
                    }
                });
                ui.add_space(6.0);

                let avail_w = ui.available_width().max(320.0);
                let spacing = 8.0_f32;
                let cols = ((avail_w / 176.0).floor() as usize).clamp(2, 5);
                let card_w = ((avail_w - spacing * (cols as f32 - 1.0)) / cols as f32)
                    .floor()
                    .max(130.0);
                let card_h = 54.0_f32;

                let mut apply_idx: Option<usize> = None;
                egui::Grid::new("theme_preset_grid")
                    .num_columns(cols)
                    .spacing([spacing, spacing])
                    .show(ui, |ui| {
                        for (i, theme) in cfg.themes.iter().enumerate() {
                            let is_active = i == cfg.active_theme;
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::click());

                            if resp.clicked() {
                                apply_idx = Some(i);
                            }

                            let bg = if is_active {
                                Color32::from_rgb(44, 62, 96)
                            } else if resp.hovered() {
                                Color32::from_rgb(38, 42, 54)
                            } else {
                                Color32::from_rgb(28, 31, 40)
                            };
                            let border = if is_active {
                                Stroke::new(2.0_f32, Color32::from_rgb(95, 180, 255))
                            } else if resp.hovered() {
                                Stroke::new(1.0_f32, Color32::from_rgb(90, 100, 125))
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(52, 57, 72))
                            };

                            let painter = ui.painter();
                            painter.rect_filled(rect, CornerRadius::same(8), bg);
                            painter.rect_stroke(rect, CornerRadius::same(8), border, StrokeKind::Inside);

                            // 상단 3색 미리보기 스와치 (기본 / 눌림 / 글자)
                            let sw_y = rect.min.y + 9.0;
                            let sw_size = Vec2::new(20.0, 14.0);
                            for (si, col) in [theme.normal, theme.press, theme.font]
                                .into_iter()
                                .enumerate()
                            {
                                let sw_rect = egui::Rect::from_min_size(
                                    egui::pos2(rect.min.x + 10.0 + si as f32 * 25.0, sw_y),
                                    sw_size,
                                );
                                painter.rect_filled(sw_rect, CornerRadius::same(4), col.to_egui());
                                painter.rect_stroke(
                                    sw_rect,
                                    CornerRadius::same(4),
                                    Stroke::new(1.0_f32, Color32::from_white_alpha(65)),
                                    StrokeKind::Inside,
                                );
                            }

                            // 하단 테마 이름
                            painter.text(
                                egui::pos2(rect.min.x + 10.0, rect.max.y - 14.0),
                                egui::Align2::LEFT_CENTER,
                                &theme.name,
                                egui::FontId::proportional(13.0),
                                if is_active {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(215, 220, 232)
                                },
                            );

                            if (i + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });

                if let Some(i) = apply_idx {
                    cfg.apply_theme(i);
                    changed = true;
                }

                ui.add_space(12.0);

                let cur = cfg.active_theme;
                if cur < cfg.themes.len() {
                    ui.heading("선택된 테마 편집");
                    ui.group(|ui| {
                        ui.set_min_width(ui.available_width());
                        let mut edited = false;
                        ui.horizontal(|ui| {
                            ui.label("테마 이름:");
                            if ui.text_edit_singleline(&mut cfg.themes[cur].name).changed() {
                                edited = true;
                            }
                            if cfg.themes.len() > 1 && cur >= 16 {
                                if ui.button("테마 삭제").clicked() {
                                    cfg.themes.remove(cur);
                                    cfg.active_theme = 0;
                                    cfg.apply_theme(0);
                                    changed = true;
                                }
                            }
                        });
                        if cur < cfg.themes.len() {
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                if color_row(ui, "글자", &mut cfg.themes[cur].font) {
                                    edited = true;
                                }
                                if color_row(ui, "기본", &mut cfg.themes[cur].normal) {
                                    edited = true;
                                }
                                if color_row(ui, "눌림", &mut cfg.themes[cur].press) {
                                    edited = true;
                                }
                                if color_row(ui, "차트 배경", &mut cfg.themes[cur].chart_bg) {
                                    edited = true;
                                }
                                if color_row(ui, "차트 선", &mut cfg.themes[cur].chart_line) {
                                    edited = true;
                                }
                            });
                            if edited {
                                cfg.apply_theme(cur);
                                changed = true;
                            }
                        }
                    });
                }
            });
    }

    if changed {
        state.mark_config_changed();
    }
}
