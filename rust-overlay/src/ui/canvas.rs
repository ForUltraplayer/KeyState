use crate::config::presets::{apply_layout_preset, LayoutPresetKind};
use crate::config::{AppConfig, KeyConfig, LayoutMode, RgbaColor};
use crate::input::keycode::vk_to_label;
use crate::state::{CaptureMode, SharedState};
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragAction {
    None,
    MoveKeys {
        start_pointer: Pos2,
    },
    ResizeKey {
        id: u64,
        start_pointer: Pos2,
        orig_w: f32,
        orig_h: f32,
    },
    MoveStatBox {
        kind: StatBoxKind,
        start_pointer: Pos2,
        orig_x: f32,
        orig_y: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatBoxKind {
    Total,
    Kps,
    Bpm,
}

pub struct CanvasEditor {
    pub selected_ids: HashSet<u64>,
    pub primary_id: Option<u64>,
    drag_action: DragAction,
    drag_origins: Vec<(u64, f32, f32)>,
    zoom: f32,
}

impl Default for CanvasEditor {
    fn default() -> Self {
        Self {
            selected_ids: HashSet::new(),
            primary_id: None,
            drag_action: DragAction::None,
            drag_origins: Vec::new(),
            zoom: 1.0,
        }
    }
}

impl CanvasEditor {
    pub fn select_only(&mut self, id: u64) {
        self.selected_ids.clear();
        self.selected_ids.insert(id);
        self.primary_id = Some(id);
    }

    /// [UX-1] 기존 캔버스 폭 안에서 우선 줄바꿈 배치하고, 부득이할 때만 세로 확장
    pub fn add_key_to_config(&mut self, cfg: &mut AppConfig, vk: u32) {
        let theme = cfg
            .themes
            .get(cfg.active_theme)
            .cloned()
            .unwrap_or_default();

        let label = vk_to_label(vk);
        let default_w = if label.len() >= 5 { 96.0 } else { 64.0 };
        let default_h = 64.0;
        let bound_w = cfg.canvas_w.max(240.0);

        let (nx, ny) = if let Some(last) = cfg.keys.last() {
            let next_x = last.x + last.width + 8.0;
            if next_x + default_w + 8.0 <= bound_w {
                (next_x, last.y)
            } else {
                (8.0, last.y + last.height + 8.0)
            }
        } else {
            (8.0, 8.0)
        };

        let id = cfg.alloc_id();
        cfg.keys.push(KeyConfig {
            id,
            vk,
            label,
            color_font: theme.font,
            color_normal: theme.normal,
            color_press: theme.press,
            x: nx,
            y: ny,
            width: default_w,
            height: default_h,
            total_presses: 0,
        });
        if ny + default_h + 8.0 > cfg.canvas_h {
            cfg.canvas_h = (ny + default_h + 16.0).min(900.0);
        }
        self.select_only(id);
    }

    /// 상단 도구 모음 ([UX-4] 고정 버튼 폭 & 조건부 행 숨김 제거로 레이아웃 흔들림 0)
    pub fn ui_toolbar(&mut self, ui: &mut Ui, state: &SharedState) {
        let mut changed = false;

        // 1행: 키 추가 & 마우스 빠른 추가 & 프리셋
        ui.horizontal_wrapped(|ui| {
            let cur_cap = *state.capture_mode.lock();
            let cont_on = cur_cap == Some(CaptureMode::ContinuousKeyboard);
            let cont_btn = egui::Button::new(if cont_on {
                "■ 연속 추가 중 (클릭 종료)"
            } else {
                "+ 연속 키보드 추가"
            })
            .fill(if cont_on {
                Color32::from_rgb(190, 55, 65)
            } else {
                Color32::from_rgb(45, 95, 175)
            });
            if ui.add_sized([168.0, 24.0], cont_btn).clicked() {
                let mut cap = state.capture_mode.lock();
                *cap = if cont_on {
                    None
                } else {
                    Some(CaptureMode::ContinuousKeyboard)
                };
            }

            let single_on = cur_cap == Some(CaptureMode::AddSingleAny);
            let single_btn = egui::Button::new(if single_on {
                "● 키/마우스 입력 대기..."
            } else {
                "+ 키/마우스 1개 추가"
            })
            .fill(if single_on {
                Color32::from_rgb(165, 105, 35)
            } else {
                Color32::from_rgb(40, 44, 56)
            });
            if ui.add_sized([156.0, 24.0], single_btn).clicked() {
                let mut cap = state.capture_mode.lock();
                *cap = if single_on {
                    None
                } else {
                    Some(CaptureMode::AddSingleAny)
                };
            }

            ui.separator();
            ui.label("마우스:");
            for (vk, name) in [
                (0x01u32, "+LMB"),
                (0x02, "+RMB"),
                (0x04, "+MMB"),
                (0x05, "+MB4"),
                (0x06, "+MB5"),
            ] {
                if ui.small_button(name).clicked() {
                    let mut cfg = state.config.write();
                    self.add_key_to_config(&mut cfg, vk);
                    changed = true;
                }
            }

            ui.separator();
            egui::ComboBox::from_id_salt("preset_combo")
                .selected_text("레이아웃 프리셋")
                .width(170.0)
                .show_ui(ui, |ui| {
                    for &preset in LayoutPresetKind::ALL {
                        if ui.selectable_label(false, preset.display_name()).clicked() {
                            state.reset_capture();
                            let mut cfg = state.config.write();
                            apply_layout_preset(&mut cfg, preset);
                            self.selected_ids.clear();
                            self.primary_id = cfg.keys.first().map(|k| k.id);
                            if let Some(id) = self.primary_id {
                                self.selected_ids.insert(id);
                            }
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        // 2행: 배치 모드 & 그리드 & 캔버스 크기 & 줌 ([UX-4] 위젯 숨김 대신 disabled 유지)
        {
            let mut cfg = state.config.write();
            ui.horizontal_wrapped(|ui| {
                ui.label("배치:");
                if ui
                    .selectable_value(&mut cfg.layout_mode, LayoutMode::FreeCanvas, "자유 캔버스")
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .selectable_value(&mut cfg.layout_mode, LayoutMode::AutoRow, "자동 가로 1줄")
                    .changed()
                {
                    changed = true;
                }

                ui.separator();
                let is_free = cfg.layout_mode == LayoutMode::FreeCanvas;

                ui.add_enabled_ui(is_free, |ui| {
                    if ui.checkbox(&mut cfg.grid_snap, "그리드").changed() {
                        changed = true;
                    }
                    if ui
                        .add_enabled(
                            cfg.grid_snap,
                            egui::DragValue::new(&mut cfg.grid_size)
                                .range(4.0..=32.0)
                                .speed(1.0)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(&mut cfg.show_canvas_boundary, "경계선")
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut cfg.canvas_w)
                                .range(140.0..=1200.0)
                                .speed(4.0)
                                .prefix("W:"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut cfg.canvas_h)
                                .range(80.0..=900.0)
                                .speed(4.0)
                                .prefix("H:"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });

                ui.separator();
                ui.label("간격:");
                if ui
                    .add(
                        egui::DragValue::new(&mut cfg.key_spacing)
                            .range(0.0..=32.0)
                            .speed(1.0)
                            .suffix("px"),
                    )
                    .changed()
                {
                    changed = true;
                }

                ui.separator();
                ui.label("줌:");
                ui.add(
                    egui::DragValue::new(&mut self.zoom)
                        .range(0.5..=2.0)
                        .speed(0.05)
                        .fixed_decimals(2),
                );
            });

            ui.add_space(3.0);

            // 3행: 다중 선택 정렬 툴바 — 항상 고정 높이로 유지([UX-4] 수직 점프 완전 제거)
            let multi_active = self.selected_ids.len() >= 2 && cfg.layout_mode == LayoutMode::FreeCanvas;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(multi_active, |ui| {
                    ui.label(if self.selected_ids.len() >= 2 {
                        format!("정렬 ({}개 선택):", self.selected_ids.len())
                    } else {
                        "정렬 (Shift+클릭 다중선택):".to_string()
                    });
                    if ui.small_button("좌측 맞춤").clicked() {
                        let min_x = cfg
                            .keys
                            .iter()
                            .filter(|k| self.selected_ids.contains(&k.id))
                            .map(|k| k.x)
                            .fold(f32::INFINITY, f32::min);
                        for k in &mut cfg.keys {
                            if self.selected_ids.contains(&k.id) {
                                k.x = min_x;
                            }
                        }
                        changed = true;
                    }
                    if ui.small_button("상단 맞춤").clicked() {
                        let min_y = cfg
                            .keys
                            .iter()
                            .filter(|k| self.selected_ids.contains(&k.id))
                            .map(|k| k.y)
                            .fold(f32::INFINITY, f32::min);
                        for k in &mut cfg.keys {
                            if self.selected_ids.contains(&k.id) {
                                k.y = min_y;
                            }
                        }
                        changed = true;
                    }
                    if ui.small_button("가로 간격 균등").clicked() {
                        let mut idxs: Vec<usize> = cfg
                            .keys
                            .iter()
                            .enumerate()
                            .filter(|(_, k)| self.selected_ids.contains(&k.id))
                            .map(|(i, _)| i)
                            .collect();
                        idxs.sort_by(|&a, &b| {
                            cfg.keys[a]
                                .x
                                .partial_cmp(&cfg.keys[b].x)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        if let Some(&first) = idxs.first() {
                            let mut cx = cfg.keys[first].x;
                            let gap = cfg.key_spacing.max(6.0);
                            for i in idxs {
                                cfg.keys[i].x = cx;
                                cx += cfg.keys[i].width + gap;
                            }
                            changed = true;
                        }
                    }
                    if ui.small_button("크기 통일").clicked() {
                        if let Some(pid) = self.primary_id {
                            if let Some((w, h)) = cfg
                                .keys
                                .iter()
                                .find(|k| k.id == pid)
                                .map(|k| (k.width, k.height))
                            {
                                for k in &mut cfg.keys {
                                    if self.selected_ids.contains(&k.id) {
                                        k.width = w;
                                        k.height = h;
                                    }
                                }
                                changed = true;
                            }
                        }
                    }
                });
            });
        }

        if changed {
            state.mark_config_changed();
        }
    }

    /// 하단 등록된 키 칩 바 ([UX-4] 텍스트 변화 대신 색상 하이라이트, AutoRow 순서 변경 버튼 고정 배치)
    pub fn ui_registered_keys(&mut self, ui: &mut Ui, state: &SharedState) {
        let mut changed = false;
        {
            let mut cfg = state.config.write();
            let is_auto = cfg.layout_mode == LayoutMode::AutoRow;
            let primary_idx = self
                .primary_id
                .and_then(|id| cfg.keys.iter().position(|k| k.id == id));

            ui.horizontal(|ui| {
                ui.strong(format!("등록된 키 ({}개)", cfg.keys.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // AutoRow 순서 이동 버튼을 고정 위치에 배치 ([UX-4])
                    let can_right = is_auto
                        && primary_idx.is_some_and(|i| i + 1 < cfg.keys.len());
                    let can_left = is_auto && primary_idx.is_some_and(|i| i > 0);
                    if ui
                        .add_enabled(can_right, egui::Button::new("순서 ▶").small())
                        .clicked()
                    {
                        if let Some(i) = primary_idx {
                            cfg.keys.swap(i, i + 1);
                            changed = true;
                        }
                    }
                    if ui
                        .add_enabled(can_left, egui::Button::new("◀ 순서").small())
                        .clicked()
                    {
                        if let Some(i) = primary_idx {
                            cfg.keys.swap(i, i - 1);
                            changed = true;
                        }
                    }
                });
            });

            ui.add_space(3.0);
            egui::ScrollArea::horizontal()
                .id_salt("registered_key_chips")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for k in &cfg.keys {
                            let is_sel = self.selected_ids.contains(&k.id);
                            let down = state.keys.is_down(k.vk);
                            // [UX-4] 키 눌림 시 텍스트에 "● "을 붙이지 않고 배경/테두리 색으로만 표시 (가로 폭 불변!)
                            let fill = if down {
                                k.color_press.with_alpha(200).to_egui()
                            } else if is_sel {
                                Color32::from_rgb(50, 95, 165)
                            } else {
                                Color32::from_rgb(34, 38, 48)
                            };
                            let stroke = if is_sel {
                                Stroke::new(1.5_f32, Color32::from_rgb(120, 195, 255))
                            } else if down {
                                Stroke::new(1.5_f32, Color32::WHITE)
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(65, 70, 86))
                            };
                            let btn = egui::Button::new(&k.label)
                                .fill(fill)
                                .stroke(stroke)
                                .corner_radius(CornerRadius::same(5));
                            if ui.add(btn).clicked() {
                                if ui.input(|i| i.modifiers.shift) {
                                    if is_sel {
                                        self.selected_ids.remove(&k.id);
                                    } else {
                                        self.selected_ids.insert(k.id);
                                        self.primary_id = Some(k.id);
                                    }
                                } else {
                                    self.select_only(k.id);
                                }
                            }
                        }
                    });
                });
        }
        if changed {
            state.mark_config_changed();
        }
    }

    /// 중앙 비주얼 인터랙티브 캔버스 ([UX-3], [UX-4], [UX-5])
    pub fn ui_canvas_center(&mut self, ui: &mut Ui, state: &SharedState) {
        let mut changed = false;
        let zoom = self.zoom;
        let avail = ui.available_size();

        egui::Frame::canvas(ui.style())
            .fill(Color32::from_rgb(20, 22, 28))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 60, 75)))
            .corner_radius(CornerRadius::same(8))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("canvas_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut cfg = state.config.write();
                        // [UX-3] 히스토리 트랙 높이를 제외한 순수 키/박스 영역 크기만 사용
                        let (base_w, base_h) = cfg.compute_editor_size();

                        let desired = Vec2::new(
                            (base_w * zoom + 48.0).max(avail.x - 16.0),
                            (base_h * zoom + 52.0).max(avail.y - 16.0),
                        );
                        let (rect, response) =
                            ui.allocate_exact_size(desired, Sense::click_and_drag());
                        let painter = ui.painter_at(rect);
                        let origin = rect.min + Vec2::new(16.0, 16.0);

                        let area_rect = Rect::from_min_size(
                            origin,
                            Vec2::new(base_w * zoom, base_h * zoom),
                        );
                        painter.rect_filled(area_rect, 4.0, Color32::from_rgb(26, 29, 36));
                        painter.rect_stroke(
                            area_rect,
                            4.0,
                            Stroke::new(1.0_f32, Color32::from_rgb(70, 78, 96)),
                            StrokeKind::Inside,
                        );

                        let step = cfg.grid_size.max(8.0) * zoom;
                        if step >= 6.0 {
                            let grid_col = Color32::from_rgba_unmultiplied(255, 255, 255, 14);
                            let mut gx = area_rect.min.x + step;
                            while gx < area_rect.max.x {
                                painter.line_segment(
                                    [
                                        Pos2::new(gx, area_rect.min.y),
                                        Pos2::new(gx, area_rect.max.y),
                                    ],
                                    Stroke::new(1.0_f32, grid_col),
                                );
                                gx += step;
                            }
                            let mut gy = area_rect.min.y + step;
                            while gy < area_rect.max.y {
                                painter.line_segment(
                                    [
                                        Pos2::new(area_rect.min.x, gy),
                                        Pos2::new(area_rect.max.x, gy),
                                    ],
                                    Stroke::new(1.0_f32, grid_col),
                                );
                                gy += step;
                            }
                        }

                        let mut key_rects: Vec<(usize, u64, Rect, Rect)> =
                            Vec::with_capacity(cfg.keys.len());
                        let mut auto_x = cfg.key_spacing;
                        for (idx, k) in cfg.keys.iter().enumerate() {
                            let (kx, ky) = match cfg.layout_mode {
                                LayoutMode::FreeCanvas => (k.x, k.y),
                                LayoutMode::AutoRow => {
                                    let x = auto_x;
                                    auto_x += k.width + cfg.key_spacing;
                                    (x, cfg.key_spacing)
                                }
                            };
                            let r = Rect::from_min_size(
                                origin + Vec2::new(kx * zoom, ky * zoom),
                                Vec2::new(k.width * zoom, k.height * zoom),
                            );
                            let handle = Rect::from_min_size(
                                Pos2::new(r.max.x - 10.0, r.max.y - 10.0),
                                Vec2::new(10.0, 10.0),
                            );
                            key_rects.push((idx, k.id, r, handle));
                        }

                        let mut stat_rects: Vec<(StatBoxKind, &str, Rect, RgbaColor, RgbaColor)> =
                            Vec::new();
                        if cfg.show_summary_box {
                            let (sx, sy) = match cfg.layout_mode {
                                LayoutMode::FreeCanvas => (cfg.total_box.x, cfg.total_box.y),
                                LayoutMode::AutoRow => {
                                    let x = auto_x;
                                    auto_x += cfg.total_box.width + cfg.key_spacing;
                                    (x, cfg.key_spacing)
                                }
                            };
                            stat_rects.push((
                                StatBoxKind::Total,
                                "합계",
                                Rect::from_min_size(
                                    origin + Vec2::new(sx * zoom, sy * zoom),
                                    Vec2::new(
                                        cfg.total_box.width * zoom,
                                        cfg.total_box.height * zoom,
                                    ),
                                ),
                                cfg.total_box.bg,
                                cfg.total_box.fg,
                            ));
                        }
                        if cfg.show_kps_box {
                            let (sx, sy) = match cfg.layout_mode {
                                LayoutMode::FreeCanvas => (cfg.kps_box.x, cfg.kps_box.y),
                                LayoutMode::AutoRow => {
                                    let x = auto_x;
                                    auto_x += cfg.kps_box.width + cfg.key_spacing;
                                    (x, cfg.key_spacing)
                                }
                            };
                            stat_rects.push((
                                StatBoxKind::Kps,
                                "KPS",
                                Rect::from_min_size(
                                    origin + Vec2::new(sx * zoom, sy * zoom),
                                    Vec2::new(cfg.kps_box.width * zoom, cfg.kps_box.height * zoom),
                                ),
                                cfg.kps_box.bg,
                                cfg.kps_box.fg,
                            ));
                        }
                        if cfg.show_bpm_box {
                            let (sx, sy) = match cfg.layout_mode {
                                LayoutMode::FreeCanvas => (cfg.bpm_box.x, cfg.bpm_box.y),
                                LayoutMode::AutoRow => (auto_x, cfg.key_spacing),
                            };
                            stat_rects.push((
                                StatBoxKind::Bpm,
                                "BPM",
                                Rect::from_min_size(
                                    origin + Vec2::new(sx * zoom, sy * zoom),
                                    Vec2::new(cfg.bpm_box.width * zoom, cfg.bpm_box.height * zoom),
                                ),
                                cfg.bpm_box.bg,
                                cfg.bpm_box.fg,
                            ));
                        }

                        // 포인터 상호작용
                        let pointer_pos = response.interact_pointer_pos();
                        if response.drag_started() {
                            if let Some(pos) = pointer_pos {
                                let shift = ui.input(|i| i.modifiers.shift);
                                let mut hit = false;

                                for &(idx, id, _r, handle) in key_rects.iter().rev() {
                                    if self.selected_ids.contains(&id)
                                        && handle.expand(3.0).contains(pos)
                                    {
                                        self.drag_action = DragAction::ResizeKey {
                                            id,
                                            start_pointer: pos,
                                            orig_w: cfg.keys[idx].width,
                                            orig_h: cfg.keys[idx].height,
                                        };
                                        hit = true;
                                        break;
                                    }
                                }

                                if !hit {
                                    for &(_idx, id, r, _handle) in key_rects.iter().rev() {
                                        if r.contains(pos) {
                                            if shift {
                                                self.selected_ids.insert(id);
                                                self.primary_id = Some(id);
                                            } else if !self.selected_ids.contains(&id) {
                                                self.select_only(id);
                                            } else {
                                                self.primary_id = Some(id);
                                            }

                                            if cfg.layout_mode == LayoutMode::FreeCanvas {
                                                self.drag_origins = cfg
                                                    .keys
                                                    .iter()
                                                    .filter(|k| self.selected_ids.contains(&k.id))
                                                    .map(|k| (k.id, k.x, k.y))
                                                    .collect();
                                                self.drag_action = DragAction::MoveKeys {
                                                    start_pointer: pos,
                                                };
                                            }
                                            hit = true;
                                            break;
                                        }
                                    }
                                }

                                if !hit && cfg.layout_mode == LayoutMode::FreeCanvas {
                                    for &(kind, _, r, _, _) in stat_rects.iter().rev() {
                                        if r.contains(pos) {
                                            let (ox, oy) = match kind {
                                                StatBoxKind::Total => {
                                                    (cfg.total_box.x, cfg.total_box.y)
                                                }
                                                StatBoxKind::Kps => (cfg.kps_box.x, cfg.kps_box.y),
                                                StatBoxKind::Bpm => (cfg.bpm_box.x, cfg.bpm_box.y),
                                            };
                                            self.drag_action = DragAction::MoveStatBox {
                                                kind,
                                                start_pointer: pos,
                                                orig_x: ox,
                                                orig_y: oy,
                                            };
                                            break;
                                        }
                                    }
                                }
                            }
                        } else if response.clicked() {
                            if let Some(pos) = pointer_pos {
                                let shift = ui.input(|i| i.modifiers.shift);
                                let mut hit = false;
                                for &(_idx, id, r, _) in key_rects.iter().rev() {
                                    if r.contains(pos) {
                                        if shift {
                                            if self.selected_ids.contains(&id) {
                                                self.selected_ids.remove(&id);
                                            } else {
                                                self.selected_ids.insert(id);
                                                self.primary_id = Some(id);
                                            }
                                        } else {
                                            self.select_only(id);
                                        }
                                        hit = true;
                                        break;
                                    }
                                }
                                if !hit && !shift {
                                    self.selected_ids.clear();
                                    self.primary_id = None;
                                }
                            }
                        }

                        if response.dragged() {
                            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                                let grid_snap = cfg.grid_snap;
                                let grid_size = cfg.grid_size;
                                let snap = |v: f32| -> f32 {
                                    if grid_snap && grid_size >= 4.0 {
                                        (v / grid_size).round() * grid_size
                                    } else {
                                        v.round()
                                    }
                                };

                                match self.drag_action {
                                    DragAction::MoveKeys { start_pointer } => {
                                        let delta = (pos - start_pointer) / zoom;
                                        for &(id, ox, oy) in &self.drag_origins {
                                            if let Some(k) =
                                                cfg.keys.iter_mut().find(|k| k.id == id)
                                            {
                                                k.x = snap((ox + delta.x).max(0.0));
                                                k.y = snap((oy + delta.y).max(0.0));
                                            }
                                        }
                                        changed = true;
                                    }
                                    DragAction::ResizeKey {
                                        id,
                                        start_pointer,
                                        orig_w,
                                        orig_h,
                                    } => {
                                        let delta = (pos - start_pointer) / zoom;
                                        if let Some(k) = cfg.keys.iter_mut().find(|k| k.id == id) {
                                            k.width = snap((orig_w + delta.x).clamp(28.0, 400.0));
                                            k.height = snap((orig_h + delta.y).clamp(28.0, 240.0));
                                            changed = true;
                                        }
                                    }
                                    DragAction::MoveStatBox {
                                        kind,
                                        start_pointer,
                                        orig_x,
                                        orig_y,
                                    } => {
                                        let delta = (pos - start_pointer) / zoom;
                                        let nx = snap((orig_x + delta.x).max(0.0));
                                        let ny = snap((orig_y + delta.y).max(0.0));
                                        match kind {
                                            StatBoxKind::Total => {
                                                cfg.total_box.x = nx;
                                                cfg.total_box.y = ny;
                                            }
                                            StatBoxKind::Kps => {
                                                cfg.kps_box.x = nx;
                                                cfg.kps_box.y = ny;
                                            }
                                            StatBoxKind::Bpm => {
                                                cfg.bpm_box.x = nx;
                                                cfg.bpm_box.y = ny;
                                            }
                                        }
                                        changed = true;
                                    }
                                    DragAction::None => {}
                                }
                            }
                        }

                        if response.drag_stopped() {
                            self.drag_action = DragAction::None;
                        }

                        // 키 렌더링
                        let radius = cfg.key_radius * zoom;
                        for &(idx, id, r, handle) in &key_rects {
                            let k = &cfg.keys[idx];
                            let is_sel = self.selected_ids.contains(&id);
                            let is_down = state.keys.is_down(k.vk);
                            let fill_col = if is_down {
                                k.color_press.to_egui()
                            } else {
                                k.color_normal.to_egui()
                            };

                            if is_down {
                                painter.rect_filled(
                                    r.expand(4.0),
                                    radius + 2.0,
                                    k.color_press.with_alpha(90).to_egui(),
                                );
                            }

                            painter.rect_filled(r, radius, fill_col);

                            let border_stroke = if is_sel {
                                Stroke::new(2.5_f32, Color32::from_rgb(90, 180, 255))
                            } else {
                                Stroke::new(
                                    (cfg.key_border_w * zoom).max(1.0),
                                    k.color_font.with_alpha(160).to_egui(),
                                )
                            };
                            painter.rect_stroke(r, radius, border_stroke, StrokeKind::Inside);

                            painter.text(
                                r.center() - Vec2::new(0.0, 5.0 * zoom),
                                egui::Align2::CENTER_CENTER,
                                &k.label,
                                FontId::proportional((cfg.key_font_size * 0.72 * zoom).max(10.0)),
                                k.color_font.to_egui(),
                            );
                            painter.text(
                                Pos2::new(r.center().x, r.max.y - 11.0 * zoom),
                                egui::Align2::CENTER_CENTER,
                                format!("{}x{}", k.width as i32, k.height as i32),
                                FontId::proportional((9.5 * zoom).max(8.0)),
                                k.color_font.with_alpha(140).to_egui(),
                            );

                            if is_sel {
                                painter.rect_filled(handle, 2.0, Color32::from_rgb(90, 180, 255));
                            }
                        }

                        for &(_, title, r, bg, fg) in &stat_rects {
                            painter.rect_filled(r, radius, bg.to_egui());
                            painter.rect_stroke(
                                r,
                                radius,
                                Stroke::new(1.5_f32, Color32::from_rgb(255, 210, 90)),
                                StrokeKind::Inside,
                            );
                            painter.text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                title,
                                FontId::proportional((13.0 * zoom).max(10.0)),
                                fg.to_egui(),
                            );
                        }

                        painter.text(
                            Pos2::new(area_rect.min.x + 6.0, area_rect.max.y + 12.0),
                            egui::Align2::LEFT_CENTER,
                            "팁: 드래그 이동 · 파란 우하단 핸들 리사이즈 · Shift+클릭 다중 선택 · Delete 삭제",
                            FontId::proportional(11.0),
                            Color32::from_rgb(135, 145, 165),
                        );
                    });
            });

        // Delete 키로 선택된 키 삭제
        if !ui.ctx().wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
            && !self.selected_ids.is_empty()
        {
            let mut cfg = state.config.write();
            if cfg.keys.len() > self.selected_ids.len() {
                cfg.keys.retain(|k| !self.selected_ids.contains(&k.id));
                self.selected_ids.clear();
                self.primary_id = cfg.keys.first().map(|k| k.id);
                if let Some(id) = self.primary_id {
                    self.selected_ids.insert(id);
                }
                changed = true;
            }
        }

        if changed {
            state.mark_config_changed();
        }
    }

    /// 우측 인스펙터 사이드 패널 ([UX-5], [UX-6], [H-3])
    pub fn ui_inspector(&mut self, ui: &mut Ui, state: &SharedState) {
        let mut changed = false;
        {
            let mut cfg = state.config.write();

            let target_id = self
                .primary_id
                .filter(|id| cfg.keys.iter().any(|k| k.id == *id))
                .or_else(|| self.selected_ids.iter().copied().next());

            ui.heading("키 속성 인스펙터");
            ui.add_space(4.0);

            let Some(id) = target_id else {
                ui.label("캔버스에서 키를 클릭하면\n라벨, 크기, 위치, 색상을 바로 편집할 수 있습니다.");
                return;
            };

            let Some(idx) = cfg.keys.iter().position(|k| k.id == id) else {
                return;
            };

            let cur_cap = *state.capture_mode.lock();
            let rebinding = cur_cap == Some(CaptureMode::RebindKey(id));

            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.group(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label("감지 키:");
                            let vk_name = vk_to_label(cfg.keys[idx].vk);
                            ui.strong(format!("{vk_name} (0x{:02X})", cfg.keys[idx].vk));
                        });
                        let btn_txt = if rebinding {
                            "● 새 키/마우스 누르세요..."
                        } else {
                            "키 다시 지정"
                        };
                        if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(btn_txt)).clicked() {
                            let mut cap = state.capture_mode.lock();
                            *cap = if rebinding {
                                None
                            } else {
                                Some(CaptureMode::RebindKey(id))
                            };
                        }
                    });

                    ui.add_space(4.0);

                    ui.group(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label("표시 라벨");
                        if ui.text_edit_singleline(&mut cfg.keys[idx].label).changed() {
                            changed = true;
                        }
                    });

                    ui.add_space(4.0);

                    ui.group(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label("위치 & 크기 (px)");
                        egui::Grid::new("key_geom_grid").num_columns(2).show(ui, |ui| {
                            ui.label("X / Y:");
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.keys[idx].x)
                                            .range(0.0..=1200.0)
                                            .speed(1.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.keys[idx].y)
                                            .range(0.0..=900.0)
                                            .speed(1.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();

                            ui.label("너비 / 높이:");
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.keys[idx].width)
                                            .range(28.0..=400.0)
                                            .speed(1.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut cfg.keys[idx].height)
                                            .range(28.0..=240.0)
                                            .speed(1.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                            ui.end_row();
                        });

                        ui.horizontal_wrapped(|ui| {
                            for (name, w, h) in [
                                ("1U", 64.0, 64.0),
                                ("Shift", 96.0, 64.0),
                                ("Space", 208.0, 56.0),
                                ("마우스", 58.0, 116.0),
                            ] {
                                if ui.small_button(name).clicked() {
                                    cfg.keys[idx].width = w;
                                    cfg.keys[idx].height = h;
                                    changed = true;
                                }
                            }
                        });
                    });

                    ui.add_space(4.0);

                    ui.group(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label("개별 키 색상");
                        if color_row(ui, "글자 색", &mut cfg.keys[idx].color_font) {
                            changed = true;
                        }
                        if color_row(ui, "기본 배경", &mut cfg.keys[idx].color_normal) {
                            changed = true;
                        }
                        if color_row(ui, "눌림 배경", &mut cfg.keys[idx].color_press) {
                            changed = true;
                        }

                        ui.add_space(4.0);
                        if ui
                            .add_sized(
                                [ui.available_width(), 24.0],
                                egui::Button::new("이 색상을 모든 키에 일괄 적용"),
                            )
                            .clicked()
                        {
                            let f = cfg.keys[idx].color_font;
                            let n = cfg.keys[idx].color_normal;
                            let p = cfg.keys[idx].color_press;
                            for k in &mut cfg.keys {
                                k.color_font = f;
                                k.color_normal = n;
                                k.color_press = p;
                            }
                            changed = true;
                        }
                    });

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        if ui.button("키 복제").clicked() {
                            let mut dup = cfg.keys[idx].clone();
                            let new_id = cfg.alloc_id();
                            dup.id = new_id;
                            dup.x += 16.0;
                            dup.y += 16.0;
                            cfg.keys.push(dup);
                            self.select_only(new_id);
                            changed = true;
                        }

                        let can_delete = cfg.keys.len() > 1;
                        if ui
                            .add_enabled(
                                can_delete,
                                egui::Button::new("✕ 삭제").fill(Color32::from_rgb(160, 48, 55)),
                            )
                            .clicked()
                        {
                            cfg.keys.remove(idx);
                            self.selected_ids.remove(&id);
                            self.primary_id = cfg.keys.first().map(|k| k.id);
                            if let Some(nid) = self.primary_id {
                                self.selected_ids.insert(nid);
                            }
                            changed = true;
                        }
                    });
                });
        } // [H-3] config.write 락을 해제한 뒤 mark_config_changed 호출

        if changed {
            state.mark_config_changed();
        }
    }
}

pub fn color_row(ui: &mut Ui, label: &str, col: &mut RgbaColor) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let mut c32 = col.to_egui();
        if ui.color_edit_button_srgba(&mut c32).changed() {
            *col = RgbaColor::from_egui(c32);
            changed = true;
        }
    });
    changed
}
