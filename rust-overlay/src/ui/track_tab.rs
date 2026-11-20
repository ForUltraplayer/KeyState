use crate::config::ChartType;
use crate::state::SharedState;
use crate::ui::canvas::color_row;
use egui::Ui;

pub fn ui(ui: &mut Ui, state: &SharedState) {
    let mut changed = false;
    {
        let mut cfg = state.config.write();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. 키 내부 & 별도 통계 박스
                ui.heading("키 통계 & BPM / KPS 박스");
                ui.group(|ui| {
                    // [UX-6] 전체 가로 폭 일관 유지
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .checkbox(&mut cfg.show_total_in_key, "키 내부 Total 표시")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .checkbox(&mut cfg.show_kps_in_key, "키 내부 KPS 표시")
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .checkbox(&mut cfg.show_summary_box, "합계 (Total) 박스")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui.checkbox(&mut cfg.show_kps_box, "KPS 박스").changed() {
                            changed = true;
                        }
                        if ui.checkbox(&mut cfg.show_bpm_box, "BPM 박스").changed() {
                            changed = true;
                        }
                    });

                    if cfg.show_bpm_box {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("BPM 기준 분음표:");
                            for div in [8u32, 16, 32, 64] {
                                if ui
                                    .selectable_value(
                                        &mut cfg.bpm_note_div,
                                        div,
                                        format!("{div}분음표"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("동시 입력 병합 시간:");
                            if ui
                                .add(egui::Slider::new(&mut cfg.bpm_merge_ms, 0..=100).suffix("ms"))
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    }

                    ui.add_space(4.0);
                    egui::Grid::new("stat_box_grid")
                        .num_columns(4)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Total 박스");
                            if color_row(ui, "배경", &mut cfg.total_box.bg) {
                                changed = true;
                            }
                            if color_row(ui, "글자", &mut cfg.total_box.fg) {
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.total_box.width)
                                            .range(36.0..=200.0)
                                            .prefix("W:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.total_box.height)
                                            .range(36.0..=160.0)
                                            .prefix("H:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();

                            ui.label("KPS 박스");
                            if color_row(ui, "배경", &mut cfg.kps_box.bg) {
                                changed = true;
                            }
                            if color_row(ui, "글자", &mut cfg.kps_box.fg) {
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.kps_box.width)
                                            .range(36.0..=200.0)
                                            .prefix("W:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.kps_box.height)
                                            .range(36.0..=160.0)
                                            .prefix("H:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();

                            ui.label("BPM 박스");
                            if color_row(ui, "배경", &mut cfg.bpm_box.bg) {
                                changed = true;
                            }
                            if color_row(ui, "글자", &mut cfg.bpm_box.fg) {
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.bpm_box.width)
                                            .range(36.0..=200.0)
                                            .prefix("W:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.bpm_box.height)
                                            .range(36.0..=160.0)
                                            .prefix("H:"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();
                        });
                });

                ui.add_space(10.0);

                // 2. 히스토리 트랙
                ui.heading("히스토리 트랙 (노트 상승 효과)");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    if ui
                        .checkbox(&mut cfg.show_history, "히스토리 트랙 활성화")
                        .changed()
                    {
                        changed = true;
                    }
                    if cfg.show_history {
                        ui.add_space(4.0);
                        egui::Grid::new("track_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("트랙 높이");
                                if ui
                                    .add(
                                        egui::Slider::new(&mut cfg.history_track_h, 40.0..=600.0)
                                            .suffix("px"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("성장 속도 (누름)");
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut cfg.history_grow_speed,
                                            60.0..=900.0,
                                        )
                                        .suffix("px/s"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("상승 속도 (뗌)");
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut cfg.history_float_speed,
                                            60.0..=900.0,
                                        )
                                        .suffix("px/s"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("최대 블록 비율");
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut cfg.history_block_max_pct,
                                            20.0..=100.0,
                                        )
                                        .suffix("%"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("트랙-키 간격");
                                if ui
                                    .add(
                                        egui::Slider::new(&mut cfg.history_track_gap, 0.0..=40.0)
                                            .suffix("px"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("트랙 배경 불투명도");
                                if ui
                                    .add(
                                        egui::Slider::new(&mut cfg.history_track_alpha, 0..=100)
                                            .suffix("%"),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("블록 불투명도");
                                if ui
                                    .add(egui::Slider::new(&mut cfg.history_block_alpha, 40..=255))
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("경계선 표시");
                                if ui
                                    .checkbox(
                                        &mut cfg.history_show_lines,
                                        "트랙 양쪽 경계선 그리기",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();
                            });
                    }
                });

                ui.add_space(10.0);

                // 3. 실시간 KPS 차트 창
                ui.heading("실시간 KPS 차트 오버레이");
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    if ui
                        .checkbox(&mut cfg.show_chart, "KPS 차트 창 표시")
                        .changed()
                    {
                        changed = true;
                    }
                    if cfg.show_chart {
                        ui.add_space(4.0);
                        egui::Grid::new("chart_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("차트 스타일");
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_value(
                                            &mut cfg.chart_type,
                                            ChartType::Line,
                                            "꺾은선",
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .selectable_value(
                                            &mut cfg.chart_type,
                                            ChartType::Scatter,
                                            "산점도",
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .selectable_value(&mut cfg.chart_type, ChartType::Bar, "막대")
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                                ui.end_row();

                                ui.label("옵션");
                                ui.horizontal(|ui| {
                                    if ui
                                        .checkbox(&mut cfg.chart_gradient_fill, "그라데이션 채우기")
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .checkbox(&mut cfg.chart_show_grid, "그리드 표시")
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                                ui.end_row();

                                ui.label("시간 범위");
                                let mut secs = (cfg.chart_time_range_ms / 1000).clamp(1, 30) as u32;
                                if ui
                                    .add(egui::Slider::new(&mut secs, 1..=30).suffix("초"))
                                    .changed()
                                {
                                    cfg.chart_time_range_ms = secs as u64 * 1000;
                                    changed = true;
                                }
                                ui.end_row();

                                ui.label("크기 / 둥글기");
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut cfg.chart_w)
                                                .range(160..=1000)
                                                .prefix("W:"),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut cfg.chart_h)
                                                .range(90..=500)
                                                .prefix("H:"),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut cfg.chart_radius)
                                                .range(0.0..=28.0)
                                                .prefix("R:"),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                                ui.end_row();

                                ui.label("차트 색상");
                                ui.horizontal(|ui| {
                                    if color_row(ui, "배경", &mut cfg.chart_bg) {
                                        changed = true;
                                    }
                                    if color_row(ui, "선/그래프", &mut cfg.chart_line) {
                                        changed = true;
                                    }
                                });
                                ui.end_row();

                                ui.label("오버레이 하단 스냅");
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut cfg.chart_snap, "자동 부착").changed() {
                                        changed = true;
                                    }
                                    if cfg.chart_snap {
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut cfg.chart_snap_offset_x)
                                                    .range(-300..=300)
                                                    .prefix("X:"),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut cfg.chart_snap_offset_y)
                                                    .range(-300..=300)
                                                    .prefix("Y:"),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                    }
                                });
                                ui.end_row();
                            });
                    }
                });
            });
    } // [H-3] 락 해제 후 알림

    if changed {
        state.mark_config_changed();
    }
}
