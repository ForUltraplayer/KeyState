pub mod presets;
pub mod storage;

use egui::Color32;
use serde::{Deserialize, Serialize};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * t).round() as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * t).round() as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * t).round() as u8,
            a: (self.a as f32 + (other.a as f32 - self.a as f32) * t).round() as u8,
        }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn brighten(self, delta: u8) -> Self {
        Self {
            r: self.r.saturating_add(delta),
            g: self.g.saturating_add(delta),
            b: self.b.saturating_add(delta),
            a: self.a,
        }
    }

    pub fn to_d2d(self, opacity_scale: f32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: self.r as f32 / 255.0,
            g: self.g as f32 / 255.0,
            b: self.b as f32 / 255.0,
            a: (self.a as f32 / 255.0) * opacity_scale.clamp(0.0, 1.0),
        }
    }

    pub fn to_egui(self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }

    pub fn from_egui(c: Color32) -> Self {
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        Self { r, g, b, a }
    }
}

impl Default for RgbaColor {
    fn default() -> Self {
        Self::new(235, 235, 245, 255)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemePreset {
    pub name: String,
    pub font: RgbaColor,
    pub normal: RgbaColor,
    pub press: RgbaColor,
    pub chart_bg: RgbaColor,
    pub chart_line: RgbaColor,
}

impl Default for ThemePreset {
    fn default() -> Self {
        Self {
            name: "커스텀".to_string(),
            font: RgbaColor::new(235, 235, 245, 255),
            normal: RgbaColor::new(38, 40, 48, 215),
            press: RgbaColor::new(255, 95, 95, 255),
            chart_bg: RgbaColor::new(18, 18, 24, 230),
            chart_line: RgbaColor::new(100, 255, 140, 255),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub id: u64,
    pub vk: u32,
    pub label: String,
    pub color_font: RgbaColor,
    pub color_normal: RgbaColor,
    pub color_press: RgbaColor,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub total_presses: u64,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            id: 0,
            vk: 0,
            label: String::new(),
            color_font: RgbaColor::new(235, 235, 245, 255),
            color_normal: RgbaColor::new(40, 42, 50, 210),
            color_press: RgbaColor::new(255, 95, 95, 255),
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 64.0,
            total_presses: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    AutoRow,
    FreeCanvas,
}

impl Default for LayoutMode {
    fn default() -> Self {
        Self::FreeCanvas
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Scatter,
    Bar,
}

impl Default for ChartType {
    fn default() -> Self {
        Self::Line
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMode {
    /// Windows Raw Input (RIDEV_INPUTSINK) — 게임 DirectInput/RawInput/IME 호환 최상
    RawInput,
    /// Raw Input + WH_KEYBOARD_LL/WH_MOUSE_LL 병행 (하드웨어 + 가상/매크로 입력)
    RawInputAndHook,
    /// GetAsyncKeyState 500Hz 고속 폴링 (안티치트가 훅·RawInput을 차단하는 특수 게임용)
    AsyncPolling,
    /// 기존 WH_KEYBOARD_LL / WH_MOUSE_LL 단독
    LowLevelHook,
}

impl Default for InputMode {
    fn default() -> Self {
        Self::RawInput
    }
}

impl InputMode {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::RawInput => 0,
            Self::RawInputAndHook => 1,
            Self::AsyncPolling => 2,
            Self::LowLevelHook => 3,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::RawInput,
            1 => Self::RawInputAndHook,
            2 => Self::AsyncPolling,
            _ => Self::LowLevelHook,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatBoxConfig {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub bg: RgbaColor,
    pub fg: RgbaColor,
}

impl Default for StatBoxConfig {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 68.0,
            height: 64.0,
            bg: RgbaColor::new(245, 245, 250, 220),
            fg: RgbaColor::new(20, 22, 28, 255),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub settings_vk: u32,
    pub toggle_display_vk: u32,
    pub next_theme_vk: u32,
    pub prev_theme_vk: u32,
    pub toggle_track_vk: u32,
    pub toggle_chart_vk: u32,
    pub record_vk: u32,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            settings_vk: b'K' as u32,
            toggle_display_vk: b'M' as u32,
            next_theme_vk: b'N' as u32,
            prev_theme_vk: b'P' as u32,
            toggle_track_vk: b'H' as u32,
            toggle_chart_vk: b'U' as u32,
            record_vk: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub active_profile: String,
    pub next_key_id: u64,

    // 표시 옵션
    pub show_display: bool,
    pub show_total_in_key: bool,
    pub show_kps_in_key: bool,
    pub show_summary_box: bool,
    pub show_kps_box: bool,
    pub show_bpm_box: bool,
    pub bpm_note_div: u32,
    pub bpm_merge_ms: u64,

    // 히스토리 트랙
    pub show_history: bool,
    pub history_track_h: f32,
    pub history_grow_speed: f32,
    pub history_float_speed: f32,
    pub history_block_max_pct: f32,
    pub history_track_gap: f32,
    pub history_track_alpha: u8,
    pub history_block_alpha: u8,
    pub history_show_lines: bool,

    // 애니메이션
    pub press_ease_ms: f32,
    pub release_ease_ms: f32,
    pub glow_intensity: f32,

    // 오버레이 동작 및 외형
    pub input_mode: InputMode,
    pub fps: u32,
    pub click_through: bool,
    pub always_on_top: bool,
    pub overlay_opacity: f32,
    pub layout_mode: LayoutMode,
    pub key_spacing: f32,
    pub key_radius: f32,
    pub key_border_w: f32,
    pub key_font_size: f32,
    pub key_font_name: String,

    // 오버레이 창 위치 & 자유 캔버스 영역
    pub display_x: i32,
    pub display_y: i32,
    pub canvas_w: f32,
    pub canvas_h: f32,
    pub show_canvas_boundary: bool,
    pub grid_snap: bool,
    pub grid_size: f32,

    // 통계 박스
    pub total_box: StatBoxConfig,
    pub kps_box: StatBoxConfig,
    pub bpm_box: StatBoxConfig,

    // KPS 차트 오버레이
    pub show_chart: bool,
    pub chart_x: i32,
    pub chart_y: i32,
    pub chart_w: i32,
    pub chart_h: i32,
    pub chart_time_range_ms: u64,
    pub chart_type: ChartType,
    pub chart_gradient_fill: bool,
    pub chart_bg: RgbaColor,
    pub chart_line: RgbaColor,
    pub chart_radius: f32,
    pub chart_show_grid: bool,
    pub chart_snap: bool,
    pub chart_snap_offset_x: i32,
    pub chart_snap_offset_y: i32,

    // 테마 & 단축키
    pub active_theme: usize,
    pub themes: Vec<ThemePreset>,
    pub hotkeys: HotkeyConfig,

    // 키 목록
    pub keys: Vec<KeyConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut cfg = Self {
            version: 1,
            active_profile: "기본 (WASD)".to_string(),
            next_key_id: 10,

            show_display: true,
            show_total_in_key: true,
            show_kps_in_key: true,
            show_summary_box: false,
            show_kps_box: false,
            show_bpm_box: false,
            bpm_note_div: 16,
            bpm_merge_ms: 30,

            show_history: false,
            history_track_h: 240.0,
            history_grow_speed: 300.0,
            history_float_speed: 300.0,
            history_block_max_pct: 100.0,
            history_track_gap: 10.0,
            history_track_alpha: 18,
            history_block_alpha: 255,
            history_show_lines: true,

            press_ease_ms: 80.0,
            release_ease_ms: 100.0,
            glow_intensity: 1.0,

            input_mode: InputMode::RawInput,
            fps: 120,
            click_through: false,
            always_on_top: true,
            overlay_opacity: 0.96,
            layout_mode: LayoutMode::FreeCanvas,
            key_spacing: 10.0,
            key_radius: 12.0,
            key_border_w: 2.0,
            key_font_size: 24.0,
            key_font_name: "Segoe UI".to_string(),

            display_x: 120,
            display_y: 280,
            canvas_w: 440.0,
            canvas_h: 240.0,
            show_canvas_boundary: false,
            grid_snap: true,
            grid_size: 8.0,

            total_box: StatBoxConfig {
                x: 264.0,
                y: 8.0,
                ..Default::default()
            },
            kps_box: StatBoxConfig {
                x: 344.0,
                y: 8.0,
                ..Default::default()
            },
            bpm_box: StatBoxConfig {
                x: 264.0,
                y: 84.0,
                ..Default::default()
            },

            show_chart: false,
            chart_x: 120,
            chart_y: 380,
            chart_w: 420,
            chart_h: 170,
            chart_time_range_ms: 10_000,
            chart_type: ChartType::Line,
            chart_gradient_fill: true,
            chart_bg: RgbaColor::new(18, 18, 24, 230),
            chart_line: RgbaColor::new(100, 255, 140, 255),
            chart_radius: 10.0,
            chart_show_grid: true,
            chart_snap: true,
            chart_snap_offset_x: 0,
            chart_snap_offset_y: 12,

            active_theme: 0,
            themes: presets::default_themes(),
            hotkeys: HotkeyConfig::default(),
            keys: Vec::new(),
        };
        presets::apply_layout_preset(&mut cfg, presets::LayoutPresetKind::WasdSpace);
        cfg
    }
}

impl AppConfig {
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_key_id;
        self.next_key_id += 1;
        id
    }

    pub fn apply_theme(&mut self, index: usize) {
        if let Some(theme) = self.themes.get(index).cloned() {
            self.active_theme = index;
            for k in &mut self.keys {
                k.color_font = theme.font;
                k.color_normal = theme.normal;
                k.color_press = theme.press;
            }
            self.chart_bg = theme.chart_bg;
            self.chart_line = theme.chart_line;
        }
    }

    pub fn cycle_theme(&mut self, delta: isize) {
        if self.themes.is_empty() {
            return;
        }
        let len = self.themes.len() as isize;
        let next = (self.active_theme as isize + delta).rem_euclid(len) as usize;
        self.apply_theme(next);
    }

    /// 에디터 캔버스에서 사용할 순수 키/통계 박스 영역 크기 (히스토리 트랙 제외)
    pub fn compute_editor_size(&self) -> (f32, f32) {
        match self.layout_mode {
            LayoutMode::AutoRow => {
                let gap = self.key_spacing;
                let mut total_w = gap;
                let mut max_h: f32 = 64.0;
                for k in &self.keys {
                    total_w += k.width + gap;
                    max_h = max_h.max(k.height);
                }
                if self.show_summary_box {
                    total_w += self.total_box.width + gap;
                    max_h = max_h.max(self.total_box.height);
                }
                if self.show_kps_box {
                    total_w += self.kps_box.width + gap;
                    max_h = max_h.max(self.kps_box.height);
                }
                if self.show_bpm_box {
                    total_w += self.bpm_box.width + gap;
                    max_h = max_h.max(self.bpm_box.height);
                }
                (total_w.max(240.0), (max_h + gap * 2.0).max(96.0))
            }
            LayoutMode::FreeCanvas => {
                let mut max_x = self.canvas_w.max(200.0);
                let mut max_y = self.canvas_h.max(120.0);
                for k in &self.keys {
                    max_x = max_x.max(k.x + k.width + 8.0);
                    max_y = max_y.max(k.y + k.height + 8.0);
                }
                if self.show_summary_box {
                    max_x = max_x.max(self.total_box.x + self.total_box.width + 8.0);
                    max_y = max_y.max(self.total_box.y + self.total_box.height + 8.0);
                }
                if self.show_kps_box {
                    max_x = max_x.max(self.kps_box.x + self.kps_box.width + 8.0);
                    max_y = max_y.max(self.kps_box.y + self.kps_box.height + 8.0);
                }
                if self.show_bpm_box {
                    max_x = max_x.max(self.bpm_box.x + self.bpm_box.width + 8.0);
                    max_y = max_y.max(self.bpm_box.y + self.bpm_box.height + 8.0);
                }
                (max_x, max_y)
            }
        }
    }

    /// 현재 레이아웃 모드에서 오버레이 콘텐츠의 전체 (너비, 높이) 계산
    pub fn compute_overlay_size(&self) -> (u32, u32) {
        let pad = 12.0; // 글로우 여유 공간
        let track_extra = if self.show_history && !self.keys.is_empty() {
            self.history_track_h + self.history_track_gap
        } else {
            0.0
        };

        match self.layout_mode {
            LayoutMode::AutoRow => {
                let gap = self.key_spacing;
                let mut total_w = pad + gap;
                let mut max_h: f32 = 64.0;
                for k in &self.keys {
                    total_w += k.width + gap;
                    max_h = max_h.max(k.height);
                }
                if self.show_summary_box {
                    total_w += self.total_box.width + gap;
                    max_h = max_h.max(self.total_box.height);
                }
                if self.show_kps_box {
                    total_w += self.kps_box.width + gap;
                    max_h = max_h.max(self.kps_box.height);
                }
                if self.show_bpm_box {
                    total_w += self.bpm_box.width + gap;
                    max_h = max_h.max(self.bpm_box.height);
                }
                total_w += pad;
                let total_h = max_h + gap * 2.0 + pad * 2.0 + track_extra;
                (total_w.ceil().max(80.0) as u32, total_h.ceil().max(80.0) as u32)
            }
            LayoutMode::FreeCanvas => {
                let mut max_x = self.canvas_w;
                let mut max_y = self.canvas_h;
                for k in &self.keys {
                    max_x = max_x.max(k.x + k.width);
                    max_y = max_y.max(k.y + k.height);
                }
                if self.show_summary_box {
                    max_x = max_x.max(self.total_box.x + self.total_box.width);
                    max_y = max_y.max(self.total_box.y + self.total_box.height);
                }
                if self.show_kps_box {
                    max_x = max_x.max(self.kps_box.x + self.kps_box.width);
                    max_y = max_y.max(self.kps_box.y + self.kps_box.height);
                }
                if self.show_bpm_box {
                    max_x = max_x.max(self.bpm_box.x + self.bpm_box.width);
                    max_y = max_y.max(self.bpm_box.y + self.bpm_box.height);
                }
                let w = max_x + pad * 2.0;
                let h = max_y + pad * 2.0 + track_extra;
                (w.ceil().max(100.0) as u32, h.ceil().max(100.0) as u32)
            }
        }
    }
}
