use super::{AppConfig, KeyConfig, LayoutMode, RgbaColor, ThemePreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPresetKind {
    WasdSpace,
    FpsStandard,
    Rhythm4K,
    Rhythm7K,
    ArrowsZx,
    LeagueQwer,
}

impl LayoutPresetKind {
    pub const ALL: &'static [LayoutPresetKind] = &[
        LayoutPresetKind::WasdSpace,
        LayoutPresetKind::FpsStandard,
        LayoutPresetKind::Rhythm4K,
        LayoutPresetKind::Rhythm7K,
        LayoutPresetKind::ArrowsZx,
        LayoutPresetKind::LeagueQwer,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::WasdSpace => "WASD + Space",
            Self::FpsStandard => "FPS (WASD + Shift/Ctrl/Space + 마우스)",
            Self::Rhythm4K => "리듬게임 4K (D F J K)",
            Self::Rhythm7K => "리듬게임 7K (S D F Space J K L)",
            Self::ArrowsZx => "방향키 + Z X",
            Self::LeagueQwer => "AOS (Q W E R + D F + 마우스)",
        }
    }
}

pub fn apply_layout_preset(cfg: &mut AppConfig, preset: LayoutPresetKind) {
    let theme = cfg
        .themes
        .get(cfg.active_theme)
        .cloned()
        .unwrap_or_default();

    // [L-4] 이전 프리셋의 트랙/통계 박스 잔류 플래그 초기화
    cfg.show_history = false;
    cfg.show_summary_box = false;
    cfg.show_kps_box = false;
    cfg.show_bpm_box = false;

    let mut next_id = cfg.next_key_id;
    let mut mk = |vk: u32, label: &str, x: f32, y: f32, w: f32, h: f32, press: Option<RgbaColor>| {
        let id = next_id;
        next_id += 1;
        KeyConfig {
            id,
            vk,
            label: label.to_string(),
            color_font: theme.font,
            color_normal: theme.normal,
            color_press: press.unwrap_or(theme.press),
            x,
            y,
            width: w,
            height: h,
            total_presses: 0,
        }
    };

    cfg.keys.clear();
    match preset {
        LayoutPresetKind::WasdSpace => {
            cfg.layout_mode = LayoutMode::FreeCanvas;
            cfg.canvas_w = 232.0;
            cfg.canvas_h = 216.0;
            cfg.keys.push(mk(b'W' as u32, "W", 84.0, 8.0, 64.0, 64.0, Some(RgbaColor::new(255, 95, 95, 255))));
            cfg.keys.push(mk(b'A' as u32, "A", 12.0, 80.0, 64.0, 64.0, Some(RgbaColor::new(95, 235, 130, 255))));
            cfg.keys.push(mk(b'S' as u32, "S", 84.0, 80.0, 64.0, 64.0, Some(RgbaColor::new(95, 150, 255, 255))));
            cfg.keys.push(mk(b'D' as u32, "D", 156.0, 80.0, 64.0, 64.0, Some(RgbaColor::new(255, 205, 70, 255))));
            cfg.keys.push(mk(0x20, "Space", 12.0, 152.0, 208.0, 52.0, Some(RgbaColor::new(190, 120, 255, 255))));
        }
        LayoutPresetKind::FpsStandard => {
            cfg.layout_mode = LayoutMode::FreeCanvas;
            cfg.canvas_w = 424.0;
            cfg.canvas_h = 200.0;
            cfg.keys.push(mk(b'Q' as u32, "Q", 96.0, 8.0, 56.0, 56.0, None));
            cfg.keys.push(mk(b'W' as u32, "W", 160.0, 8.0, 56.0, 56.0, None));
            cfg.keys.push(mk(b'E' as u32, "E", 224.0, 8.0, 56.0, 56.0, None));
            cfg.keys.push(mk(b'R' as u32, "R", 288.0, 8.0, 56.0, 56.0, None));

            cfg.keys.push(mk(0xA0, "Shift", 8.0, 72.0, 80.0, 56.0, None));
            cfg.keys.push(mk(b'A' as u32, "A", 96.0, 72.0, 56.0, 56.0, None));
            cfg.keys.push(mk(b'S' as u32, "S", 160.0, 72.0, 56.0, 56.0, None));
            cfg.keys.push(mk(b'D' as u32, "D", 224.0, 72.0, 56.0, 56.0, None));

            cfg.keys.push(mk(0xA2, "Ctrl", 8.0, 136.0, 80.0, 52.0, None));
            cfg.keys.push(mk(0x20, "Space", 96.0, 136.0, 184.0, 52.0, None));

            cfg.keys.push(mk(0x01, "LMB", 296.0, 72.0, 56.0, 116.0, Some(RgbaColor::new(70, 210, 255, 255))));
            cfg.keys.push(mk(0x02, "RMB", 358.0, 72.0, 56.0, 116.0, Some(RgbaColor::new(70, 210, 255, 255))));
        }
        LayoutPresetKind::Rhythm4K => {
            cfg.layout_mode = LayoutMode::AutoRow;
            cfg.show_history = true;
            cfg.show_kps_box = true;
            cfg.show_bpm_box = true;
            let cols = [
                RgbaColor::new(80, 200, 255, 255),
                RgbaColor::new(255, 110, 170, 255),
                RgbaColor::new(255, 110, 170, 255),
                RgbaColor::new(80, 200, 255, 255),
            ];
            for (i, (vk, name)) in [(b'D', "D"), (b'F', "F"), (b'J', "J"), (b'K', "K")]
                .into_iter()
                .enumerate()
            {
                cfg.keys.push(mk(
                    vk as u32,
                    name,
                    8.0 + i as f32 * 74.0,
                    8.0,
                    64.0,
                    64.0,
                    Some(cols[i]),
                ));
            }
            cfg.canvas_w = 320.0;
            cfg.canvas_h = 88.0;
        }
        LayoutPresetKind::Rhythm7K => {
            cfg.layout_mode = LayoutMode::AutoRow;
            cfg.show_history = true;
            cfg.show_kps_box = true;
            let items: [(u32, &str, f32, RgbaColor); 7] = [
                (b'S' as u32, "S", 58.0, RgbaColor::new(230, 235, 255, 255)),
                (b'D' as u32, "D", 58.0, RgbaColor::new(90, 180, 255, 255)),
                (b'F' as u32, "F", 58.0, RgbaColor::new(230, 235, 255, 255)),
                (0x20, "Space", 76.0, RgbaColor::new(255, 200, 70, 255)),
                (b'J' as u32, "J", 58.0, RgbaColor::new(230, 235, 255, 255)),
                (b'K' as u32, "K", 58.0, RgbaColor::new(90, 180, 255, 255)),
                (b'L' as u32, "L", 58.0, RgbaColor::new(230, 235, 255, 255)),
            ];
            let mut x = 8.0;
            for (vk, name, w, c) in items {
                cfg.keys.push(mk(vk, name, x, 8.0, w, 64.0, Some(c)));
                x += w + 8.0;
            }
            cfg.canvas_w = x + 8.0;
            cfg.canvas_h = 88.0;
        }
        LayoutPresetKind::ArrowsZx => {
            cfg.layout_mode = LayoutMode::FreeCanvas;
            cfg.canvas_w = 376.0;
            cfg.canvas_h = 152.0;
            cfg.keys.push(mk(b'Z' as u32, "Z", 8.0, 80.0, 64.0, 64.0, Some(RgbaColor::new(255, 110, 110, 255))));
            cfg.keys.push(mk(b'X' as u32, "X", 80.0, 80.0, 64.0, 64.0, Some(RgbaColor::new(110, 220, 255, 255))));
            cfg.keys.push(mk(0x26, "↑", 232.0, 8.0, 64.0, 64.0, None));
            cfg.keys.push(mk(0x25, "←", 160.0, 80.0, 64.0, 64.0, None));
            cfg.keys.push(mk(0x28, "↓", 232.0, 80.0, 64.0, 64.0, None));
            cfg.keys.push(mk(0x27, "→", 304.0, 80.0, 64.0, 64.0, None));
        }
        LayoutPresetKind::LeagueQwer => {
            cfg.layout_mode = LayoutMode::FreeCanvas;
            cfg.canvas_w = 424.0;
            cfg.canvas_h = 152.0;
            for (i, (vk, label)) in [(b'Q', "Q"), (b'W', "W"), (b'E', "E"), (b'R', "R")]
                .into_iter()
                .enumerate()
            {
                cfg.keys.push(mk(vk as u32, label, 8.0 + i as f32 * 68.0, 8.0, 60.0, 60.0, None));
            }
            cfg.keys.push(mk(b'D' as u32, "D", 76.0, 78.0, 60.0, 60.0, Some(RgbaColor::new(255, 200, 60, 255))));
            cfg.keys.push(mk(b'F' as u32, "F", 144.0, 78.0, 60.0, 60.0, Some(RgbaColor::new(255, 140, 60, 255))));
            cfg.keys.push(mk(0x01, "LMB", 292.0, 8.0, 58.0, 130.0, Some(RgbaColor::new(90, 210, 255, 255))));
            cfg.keys.push(mk(0x02, "RMB", 356.0, 8.0, 58.0, 130.0, Some(RgbaColor::new(90, 210, 255, 255))));
        }
    }
    cfg.next_key_id = next_id;

    // [UX-1] 통계 박스 기본 좌표를 프리셋 영역 하단에 자연스럽게 정렬 (예기치 못한 가로 급확장 방지)
    let stat_y = cfg.canvas_h + 8.0;
    cfg.total_box.x = 8.0;
    cfg.total_box.y = stat_y;
    cfg.kps_box.x = 8.0 + cfg.total_box.width + 8.0;
    cfg.kps_box.y = stat_y;
    cfg.bpm_box.x = 8.0 + (cfg.total_box.width + 8.0) * 2.0;
    cfg.bpm_box.y = stat_y;
}

pub fn default_themes() -> Vec<ThemePreset> {
    let mk = |name: &str, f: RgbaColor, n: RgbaColor, p: RgbaColor, cb: RgbaColor, cl: RgbaColor| {
        ThemePreset {
            name: name.to_string(),
            font: f,
            normal: n,
            press: p,
            chart_bg: cb,
            chart_line: cl,
        }
    };
    let w = RgbaColor::new(255, 255, 255, 255);
    vec![
        mk("다크 모던", RgbaColor::new(235, 238, 248, 255), RgbaColor::new(34, 36, 44, 215), RgbaColor::new(255, 95, 95, 255), RgbaColor::new(18, 18, 24, 230), RgbaColor::new(100, 255, 140, 255)),
        mk("네온 핑크", RgbaColor::new(255, 220, 245, 255), RgbaColor::new(38, 18, 52, 220), RgbaColor::new(255, 64, 160, 255), RgbaColor::new(24, 12, 34, 235), RgbaColor::new(255, 80, 180, 255)),
        mk("사이버 시안", RgbaColor::new(220, 252, 255, 255), RgbaColor::new(14, 32, 42, 220), RgbaColor::new(0, 215, 220, 255), RgbaColor::new(10, 22, 30, 235), RgbaColor::new(0, 230, 235, 255)),
        mk("로얄 바이올렛", w, RgbaColor::new(48, 40, 96, 220), RgbaColor::new(124, 102, 255, 255), RgbaColor::new(24, 20, 48, 235), RgbaColor::new(140, 120, 255, 255)),
        mk("코랄 레드", w, RgbaColor::new(220, 88, 88, 220), RgbaColor::new(255, 130, 110, 255), RgbaColor::new(32, 18, 20, 230), RgbaColor::new(255, 107, 107, 255)),
        mk("에메랄드 포레스트", RgbaColor::new(225, 255, 235, 255), RgbaColor::new(24, 62, 48, 220), RgbaColor::new(56, 210, 135, 255), RgbaColor::new(14, 32, 24, 235), RgbaColor::new(72, 225, 150, 255)),
        mk("딥 블루", w, RgbaColor::new(32, 58, 118, 220), RgbaColor::new(74, 144, 255, 255), RgbaColor::new(16, 26, 48, 235), RgbaColor::new(90, 160, 255, 255)),
        mk("선셋 앰버", w, RgbaColor::new(74, 42, 18, 220), RgbaColor::new(255, 160, 45, 255), RgbaColor::new(34, 20, 12, 235), RgbaColor::new(255, 175, 60, 255)),
        mk("라벤더", w, RgbaColor::new(82, 66, 114, 220), RgbaColor::new(178, 145, 235, 255), RgbaColor::new(28, 24, 42, 235), RgbaColor::new(185, 155, 245, 255)),
        mk("사쿠라", RgbaColor::new(60, 30, 42, 255), RgbaColor::new(255, 210, 220, 225), RgbaColor::new(255, 130, 160, 255), RgbaColor::new(255, 242, 246, 235), RgbaColor::new(240, 110, 145, 255)),
        mk("미니멀 라이트", RgbaColor::new(36, 38, 44, 255), RgbaColor::new(235, 237, 242, 225), RgbaColor::new(140, 150, 170, 255), RgbaColor::new(245, 247, 250, 235), RgbaColor::new(80, 95, 120, 255)),
        mk("골드 러시", RgbaColor::new(255, 248, 220, 255), RgbaColor::new(56, 46, 16, 225), RgbaColor::new(255, 205, 30, 255), RgbaColor::new(30, 26, 14, 235), RgbaColor::new(255, 215, 50, 255)),
        mk("슬레이트 그레이", w, RgbaColor::new(62, 74, 84, 220), RgbaColor::new(135, 160, 178, 255), RgbaColor::new(26, 32, 38, 235), RgbaColor::new(150, 175, 195, 255)),
        mk("크림슨", w, RgbaColor::new(92, 22, 30, 225), RgbaColor::new(235, 48, 65, 255), RgbaColor::new(34, 12, 16, 235), RgbaColor::new(240, 60, 75, 255)),
        mk("민트 브리즈", RgbaColor::new(18, 48, 44, 255), RgbaColor::new(180, 245, 232, 225), RgbaColor::new(0, 200, 175, 255), RgbaColor::new(235, 252, 248, 235), RgbaColor::new(0, 185, 160, 255)),
        mk("미드나잇 퍼플", RgbaColor::new(230, 215, 255, 255), RgbaColor::new(34, 18, 64, 225), RgbaColor::new(150, 80, 255, 255), RgbaColor::new(18, 10, 34, 235), RgbaColor::new(165, 100, 255, 255)),
    ]
}
