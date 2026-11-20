use crate::config::{AppConfig, KeyConfig, LayoutMode, RgbaColor};
use crate::input::hook::WM_SHOW_TRAY_MENU;
use crate::input::state::KeyStateManager;
use crate::overlay::anim::AnimTracker;
use crate::overlay::d2d::{D2dShared, D2dSurface};
use crate::state::SharedState;
use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use windows::core::{w, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ROUNDED_RECT,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteTextFormat, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetWindowLongPtrW,
    GetWindowLongW, LoadCursorW, PostMessageW, RegisterClassExW, SetWindowLongPtrW,
    SetWindowLongW, SetWindowPos, ShowWindow, CREATESTRUCTW, GWL_EXSTYLE, GWLP_USERDATA,
    HTCLIENT, HTTRANSPARENT, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WM_CAPTURECHANGED,
    WM_CREATE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_NCHITTEST,
    WM_RBUTTONUP, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

struct HitRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// [C-3] wnd_proc 재진입 시 &mut aliasing UB를 원천 차단하기 위해 내부 가변성(Cell/RefCell) 사용
struct DisplayWindowContext {
    state: Arc<SharedState>,
    dragging: Cell<bool>,
    drag_offset_x: Cell<i32>,
    drag_offset_y: Cell<i32>,
    win_x: Cell<i32>,
    win_y: Cell<i32>,
    dpi_scale: Cell<f32>,
    hit_rects: RefCell<Vec<HitRect>>,
    allow_whole_drag: Cell<bool>,
}

pub struct DisplayOverlay {
    hwnd: HWND,
    surface: Option<D2dSurface>,
    ctx_ptr: *mut DisplayWindowContext,
    anim: AnimTracker,
    key_fmt: Option<IDWriteTextFormat>,
    stat_fmt: Option<IDWriteTextFormat>,
    cached_font_name: String,
    cached_font_size: f32,
    last_topmost_check: Instant,
    last_click_through: bool,
    last_always_on_top: bool,
    last_visible: bool,
    last_rect: (i32, i32, u32, u32),
}

impl DisplayOverlay {
    pub fn new(shared: &D2dShared, state: Arc<SharedState>) -> Result<Self> {
        let cfg = state.config.read().clone();
        let (init_w, init_h) = cfg.compute_overlay_size();

        let ctx_box = Box::new(DisplayWindowContext {
            state: state.clone(),
            dragging: Cell::new(false),
            drag_offset_x: Cell::new(0),
            drag_offset_y: Cell::new(0),
            win_x: Cell::new(cfg.display_x),
            win_y: Cell::new(cfg.display_y),
            dpi_scale: Cell::new(1.0),
            hit_rects: RefCell::new(Vec::new()),
            allow_whole_drag: Cell::new(false),
        });
        let ctx_ptr = Box::into_raw(ctx_box);

        let hwnd = unsafe {
            let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
            let class_name = w!("KeyStateRsDisplayOverlay");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(display_wnd_proc),
                hInstance: hinst,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassExW(&wc);

            let mut ex = WS_EX_NOREDIRECTIONBITMAP
                | WS_EX_TOPMOST
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE;
            if cfg.click_through {
                ex |= WS_EX_TRANSPARENT;
            }

            match CreateWindowExW(
                ex,
                class_name,
                w!("KeyState-RS Overlay"),
                WS_POPUP,
                cfg.display_x,
                cfg.display_y,
                init_w as i32,
                init_h as i32,
                None,
                None,
                hinst,
                Some(ctx_ptr as *const core::ffi::c_void),
            ) {
                Ok(h) => h,
                Err(e) => {
                    // [C-3] 생성 실패 시 ctx_ptr 누수 방지
                    drop(Box::from_raw(ctx_ptr));
                    return Err(e);
                }
            }
        };

        let dpi = unsafe { GetDpiForWindow(hwnd).max(96) as f32 };
        let scale = dpi / 96.0;
        let phys_w = ((init_w as f32 * scale).round() as u32).max(1);
        let phys_h = ((init_h as f32 * scale).round() as u32).max(1);

        let surface = match D2dSurface::new(shared, hwnd, phys_w, phys_h, dpi) {
            Ok(s) => s,
            Err(e) => {
                // [C-3] 서피스 생성 실패 시 HWND 및 ctx_ptr 정리
                unsafe {
                    let _ = DestroyWindow(hwnd);
                    drop(Box::from_raw(ctx_ptr));
                }
                return Err(e);
            }
        };

        unsafe {
            (*ctx_ptr).dpi_scale.set(scale);
            let _ = ShowWindow(hwnd, if cfg.show_display { SW_SHOWNA } else { SW_HIDE });
        }

        Ok(Self {
            hwnd,
            surface: Some(surface),
            ctx_ptr,
            anim: AnimTracker::new(),
            key_fmt: None,
            stat_fmt: None,
            cached_font_name: String::new(),
            cached_font_size: 0.0,
            last_topmost_check: Instant::now(),
            last_click_through: cfg.click_through,
            last_always_on_top: cfg.always_on_top,
            last_visible: cfg.show_display,
            last_rect: (cfg.display_x, cfg.display_y, phys_w, phys_h),
        })
    }

    pub fn is_dragging(&self) -> bool {
        unsafe { (*self.ctx_ptr).dragging.get() }
    }

    pub fn last_window_rect(&self) -> (i32, i32, u32, u32) {
        self.last_rect
    }

    fn ensure_fonts(&mut self, shared: &D2dShared, cfg: &AppConfig) {
        if self.key_fmt.is_none()
            || self.cached_font_name != cfg.key_font_name
            || (self.cached_font_size - cfg.key_font_size).abs() > 0.1
        {
            self.key_fmt = shared
                .create_text_format(
                    &cfg.key_font_name,
                    cfg.key_font_size,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )
                .or_else(|_| {
                    shared.create_text_format(
                        "Segoe UI",
                        cfg.key_font_size,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    )
                })
                .ok();

            self.stat_fmt = shared
                .create_text_format(
                    &cfg.key_font_name,
                    11.5,
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                )
                .or_else(|_| {
                    shared.create_text_format(
                        "Segoe UI",
                        11.5,
                        DWRITE_FONT_WEIGHT_REGULAR,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    )
                })
                .ok();

            self.cached_font_name = cfg.key_font_name.clone();
            self.cached_font_size = cfg.key_font_size;
        }
    }

    pub fn update_and_render(
        &mut self,
        shared: &D2dShared,
        cfg: &AppConfig,
        ksm: &KeyStateManager,
        cfg_changed: bool,
        now: Instant,
    ) -> Result<bool> {
        if cfg.show_display != self.last_visible {
            self.last_visible = cfg.show_display;
            unsafe {
                let _ = ShowWindow(self.hwnd, if cfg.show_display { SW_SHOWNA } else { SW_HIDE });
            }
        }
        if !cfg.show_display {
            return Ok(false);
        }

        if cfg.click_through != self.last_click_through {
            self.last_click_through = cfg.click_through;
            unsafe {
                let mut ex = GetWindowLongW(self.hwnd, GWL_EXSTYLE);
                if cfg.click_through {
                    ex |= WS_EX_TRANSPARENT.0 as i32;
                } else {
                    ex &= !(WS_EX_TRANSPARENT.0 as i32);
                }
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, ex);
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
                );
            }
        }

        if cfg.always_on_top != self.last_always_on_top
            || (cfg.always_on_top && now.duration_since(self.last_topmost_check).as_millis() > 500)
        {
            self.last_always_on_top = cfg.always_on_top;
            self.last_topmost_check = now;
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    if cfg.always_on_top {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }

        // [M-5] Per-Monitor DPI 대응 + [UX-2] 화면 상단 음수 클램핑
        let dpi = unsafe { GetDpiForWindow(self.hwnd).max(96) as f32 };
        let scale = dpi / 96.0;
        let (dip_w, dip_h) = cfg.compute_overlay_size();
        let phys_w = ((dip_w as f32 * scale).round() as u32).max(1);
        let phys_h = ((dip_h as f32 * scale).round() as u32).max(1);

        let track_offset_phys = if cfg.show_history && !cfg.keys.is_empty() {
            ((cfg.history_track_h + cfg.history_track_gap) * scale).round() as i32
        } else {
            0
        };
        let win_x = cfg.display_x;
        let win_y = (cfg.display_y - track_offset_phys).max(0);

        if cfg_changed || self.last_rect != (win_x, win_y, phys_w, phys_h) {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    win_x,
                    win_y,
                    phys_w as i32,
                    phys_h as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                let ctx = &*self.ctx_ptr;
                ctx.win_x.set(win_x);
                ctx.win_y.set(win_y);
                ctx.dpi_scale.set(scale);
            }
            if let Some(surf) = self.surface.as_mut() {
                surf.resize(shared, phys_w, phys_h, dpi)?;
            }
            self.last_rect = (win_x, win_y, phys_w, phys_h);
        }

        self.ensure_fonts(shared, cfg);

        let pad = 12.0f32;
        let has_history = cfg.show_history && !cfg.keys.is_empty();
        let track_h = if has_history { cfg.history_track_h } else { 0.0 };
        let track_gap = if has_history { cfg.history_track_gap } else { 0.0 };
        let key_y_offset = pad + track_h + track_gap;

        let mut key_rects: Vec<(&KeyConfig, f32, f32, f32, f32)> = Vec::with_capacity(cfg.keys.len());
        let mut hit_rects: Vec<HitRect> = Vec::with_capacity(cfg.keys.len() + 3);

        match cfg.layout_mode {
            LayoutMode::AutoRow => {
                let mut cx = pad + cfg.key_spacing;
                let cy = key_y_offset + cfg.key_spacing;
                for k in &cfg.keys {
                    key_rects.push((k, cx, cy, k.width, k.height));
                    hit_rects.push(HitRect {
                        x: cx,
                        y: cy,
                        w: k.width,
                        h: k.height,
                    });
                    cx += k.width + cfg.key_spacing;
                }
            }
            LayoutMode::FreeCanvas => {
                for k in &cfg.keys {
                    let rx = pad + k.x;
                    let ry = key_y_offset + k.y;
                    key_rects.push((k, rx, ry, k.width, k.height));
                    hit_rects.push(HitRect {
                        x: rx,
                        y: ry,
                        w: k.width,
                        h: k.height,
                    });
                }
            }
        }

        let mut stat_x_cursor = pad + cfg.key_spacing;
        if cfg.layout_mode == LayoutMode::AutoRow {
            for k in &cfg.keys {
                stat_x_cursor += k.width + cfg.key_spacing;
            }
        }

        let mut stat_boxes: Vec<(&str, String, f32, f32, f32, f32, RgbaColor, RgbaColor)> = Vec::new();

        // [L-2] 동일 VK 중복 등록 시 Total / KPS / BPM 이중 합산 방지
        let mut unique_vks: Vec<u32> = cfg.keys.iter().map(|k| k.vk).collect();
        unique_vks.sort_unstable();
        unique_vks.dedup();

        let mut sum_total = 0u64;
        let mut sum_kps = 0usize;
        for &vk in &unique_vks {
            sum_total += ksm.get_total(vk);
            sum_kps += ksm.get_kps(vk);
        }
        let bpm_kps = if cfg.show_bpm_box && !unique_vks.is_empty() && cfg.bpm_merge_ms > 0 {
            let ts = ksm.collect_recent_timestamps(&unique_vks, now);
            let mut merged = 0usize;
            let mut last_t: Option<Instant> = None;
            for t in ts {
                if let Some(prev) = last_t {
                    if t.duration_since(prev).as_millis() as u64 >= cfg.bpm_merge_ms {
                        merged += 1;
                        last_t = Some(t);
                    }
                } else {
                    merged += 1;
                    last_t = Some(t);
                }
            }
            merged as f64
        } else {
            sum_kps as f64
        };

        if cfg.show_summary_box {
            let (bx, by) = match cfg.layout_mode {
                LayoutMode::AutoRow => {
                    let x = stat_x_cursor;
                    stat_x_cursor += cfg.total_box.width + cfg.key_spacing;
                    (x, key_y_offset + cfg.key_spacing)
                }
                LayoutMode::FreeCanvas => (pad + cfg.total_box.x, key_y_offset + cfg.total_box.y),
            };
            hit_rects.push(HitRect {
                x: bx,
                y: by,
                w: cfg.total_box.width,
                h: cfg.total_box.height,
            });
            stat_boxes.push((
                "합계",
                sum_total.to_string(),
                bx,
                by,
                cfg.total_box.width,
                cfg.total_box.height,
                cfg.total_box.bg,
                cfg.total_box.fg,
            ));
        }

        if cfg.show_kps_box {
            let (bx, by) = match cfg.layout_mode {
                LayoutMode::AutoRow => {
                    let x = stat_x_cursor;
                    stat_x_cursor += cfg.kps_box.width + cfg.key_spacing;
                    (x, key_y_offset + cfg.key_spacing)
                }
                LayoutMode::FreeCanvas => (pad + cfg.kps_box.x, key_y_offset + cfg.kps_box.y),
            };
            hit_rects.push(HitRect {
                x: bx,
                y: by,
                w: cfg.kps_box.width,
                h: cfg.kps_box.height,
            });
            stat_boxes.push((
                "KPS/s",
                sum_kps.to_string(),
                bx,
                by,
                cfg.kps_box.width,
                cfg.kps_box.height,
                cfg.kps_box.bg,
                cfg.kps_box.fg,
            ));
        }

        if cfg.show_bpm_box {
            let (bx, by) = match cfg.layout_mode {
                LayoutMode::AutoRow => (stat_x_cursor, key_y_offset + cfg.key_spacing),
                LayoutMode::FreeCanvas => (pad + cfg.bpm_box.x, key_y_offset + cfg.bpm_box.y),
            };
            let bpm = (bpm_kps * 240.0 / cfg.bpm_note_div.max(1) as f64).round() as u32;
            hit_rects.push(HitRect {
                x: bx,
                y: by,
                w: cfg.bpm_box.width,
                h: cfg.bpm_box.height,
            });
            stat_boxes.push((
                match cfg.bpm_note_div {
                    8 => "BPM | 8",
                    32 => "BPM | 32",
                    64 => "BPM | 64",
                    _ => "BPM | 16",
                },
                bpm.to_string(),
                bx,
                by,
                cfg.bpm_box.width,
                cfg.bpm_box.height,
                cfg.bpm_box.bg,
                cfg.bpm_box.fg,
            ));
        }

        unsafe {
            let ctx = &*self.ctx_ptr;
            *ctx.hit_rects.borrow_mut() = hit_rects;
            ctx.allow_whole_drag
                .set(cfg.layout_mode == LayoutMode::FreeCanvas && cfg.show_canvas_boundary);
        }

        // === GPU Direct2D 렌더링 ([H-5] 공용 브러시 1개 재사용) ===
        let Some(surf) = self.surface.as_ref() else {
            return Ok(false);
        };
        let opacity = cfg.overlay_opacity.clamp(0.05, 1.0);
        let mut any_animating = false;

        unsafe {
            let dc = &surf.dc;
            dc.BeginDraw();
            dc.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            dc.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            dc.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));

            // 자유 캔버스 경계선 표시 옵션
            if cfg.layout_mode == LayoutMode::FreeCanvas && cfg.show_canvas_boundary {
                let rect = D2D_RECT_F {
                    left: pad,
                    top: key_y_offset,
                    right: pad + cfg.canvas_w,
                    bottom: key_y_offset + cfg.canvas_h,
                };
                dc.FillRectangle(
                    &rect,
                    surf.solid_brush(&D2D1_COLOR_F {
                        r: 1.0,
                        g: 0.35,
                        b: 0.35,
                        a: 0.07,
                    }),
                );
                dc.DrawRectangle(
                    &rect,
                    surf.solid_brush(&D2D1_COLOR_F {
                        r: 1.0,
                        g: 0.45,
                        b: 0.45,
                        a: 0.55,
                    }),
                    1.2,
                    None,
                );
            }

            // 1. 히스토리 트랙 ([UX-2] 각 키의 실제 상단(ry - gap - h)에 정렬)
            if has_history {
                for (kc, rx, ry, kw, _kh) in &key_rects {
                    let bottom_y = *ry - track_gap;
                    let top_y = (bottom_y - track_h).max(pad);
                    let eff_h = (bottom_y - top_y).max(16.0);
                    let track_active = Self::draw_history_track(
                        surf,
                        cfg,
                        kc.vk,
                        ksm,
                        *rx,
                        top_y,
                        *kw,
                        eff_h,
                        kc.color_press,
                        opacity,
                        now,
                    );
                    if track_active {
                        any_animating = true;
                    }
                }
            }

            // 2. 키 박스 + Smoothstep 이징 + 3단 글로우
            for (kc, rx, ry, kw, kh) in &key_rects {
                let pressed = ksm.is_down(kc.vk);
                let (t, anim_running) = self.anim.sample(
                    kc.vk,
                    pressed,
                    now,
                    cfg.press_ease_ms,
                    cfg.release_ease_ms,
                );
                if anim_running || pressed {
                    any_animating = true;
                }
                Self::draw_key_box(
                    surf,
                    self.key_fmt.as_ref(),
                    self.stat_fmt.as_ref(),
                    cfg,
                    kc,
                    ksm,
                    *rx,
                    *ry,
                    *kw,
                    *kh,
                    t,
                    opacity,
                );
            }

            // 3. 통계 박스 (Total / KPS / BPM)
            for (sub, val, bx, by, bw, bh, bg, fg) in &stat_boxes {
                Self::draw_stat_box(
                    surf,
                    self.key_fmt.as_ref(),
                    self.stat_fmt.as_ref(),
                    cfg,
                    *bx,
                    *by,
                    *bw,
                    *bh,
                    *bg,
                    *fg,
                    val,
                    sub,
                    opacity,
                );
            }

            dc.EndDraw(None, None)?;
        }

        surf.present()?;

        let max_track_age = if has_history && cfg.history_float_speed > 1.0 {
            (cfg.history_track_h / cfg.history_float_speed) + 1.5
        } else {
            1.2
        };
        if !any_animating && ksm.has_active_activity(&unique_vks, max_track_age) {
            any_animating = true;
        }

        Ok(any_animating)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_key_box(
        surf: &D2dSurface,
        key_fmt: Option<&IDWriteTextFormat>,
        stat_fmt: Option<&IDWriteTextFormat>,
        cfg: &AppConfig,
        kc: &KeyConfig,
        ksm: &KeyStateManager,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        t: f32,
        opacity: f32,
    ) {
        let dc = &surf.dc;
        let cur_bg = kc.color_normal.lerp(kc.color_press, t);
        let cur_font = kc.color_font;
        let r = cfg.key_radius.min(w * 0.5).min(h * 0.5).max(0.0);

        let glow_t = (t * cfg.glow_intensity).clamp(0.0, 1.5);
        if glow_t > 0.002 {
            let outer_a = ((42.0 * glow_t).min(120.0)) as u8;
            if outer_a > 0 {
                let rr = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: x - 6.0,
                        top: y - 6.0,
                        right: x + w + 6.0,
                        bottom: y + h + 6.0,
                    },
                    radiusX: r + 3.0,
                    radiusY: r + 3.0,
                };
                dc.FillRoundedRectangle(&rr, surf.solid_brush(&cur_bg.with_alpha(outer_a).to_d2d(opacity)));
            }
            let mid_a = ((88.0 * glow_t).min(180.0)) as u8;
            if mid_a > 0 {
                let rr = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: x - 2.5,
                        top: y - 2.5,
                        right: x + w + 2.5,
                        bottom: y + h + 2.5,
                    },
                    radiusX: r + 1.5,
                    radiusY: r + 1.5,
                };
                dc.FillRoundedRectangle(&rr, surf.solid_brush(&cur_bg.with_alpha(mid_a).to_d2d(opacity)));
            }
        }

        let main_rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            },
            radiusX: r,
            radiusY: r,
        };
        dc.FillRoundedRectangle(&main_rr, surf.solid_brush(&cur_bg.to_d2d(opacity)));

        if glow_t > 0.002 {
            let bright_a = ((225.0 * glow_t).min(255.0)) as u8;
            let bright = cur_bg.brighten(70).with_alpha(bright_a);
            let inner_rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x + 2.0,
                    top: y + 2.0,
                    right: x + w - 2.0,
                    bottom: y + h - 2.0,
                },
                radiusX: (r - 1.5).max(0.0),
                radiusY: (r - 1.5).max(0.0),
            };
            dc.DrawRoundedRectangle(&inner_rr, surf.solid_brush(&bright.to_d2d(opacity)), 2.0, None);
        }

        if cfg.key_border_w > 0.1 {
            let border_col = cur_font.with_alpha(180);
            dc.DrawRoundedRectangle(
                &main_rr,
                surf.solid_brush(&border_col.to_d2d(opacity)),
                cfg.key_border_w,
                None,
            );
        }

        let show_stats = cfg.show_total_in_key || cfg.show_kps_in_key;
        let label_h = if show_stats { h * 0.62 } else { h };

        if let Some(fmt) = key_fmt {
            let text_utf16: Vec<u16> = kc.label.encode_utf16().collect();
            let rect = D2D_RECT_F {
                left: x + 2.0,
                top: y + 2.0,
                right: x + w - 2.0,
                bottom: y + label_h,
            };
            dc.DrawText(
                &text_utf16,
                fmt,
                &rect,
                surf.solid_brush(&cur_font.to_d2d(opacity)),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }

        if show_stats {
            let mut stats = String::new();
            if cfg.show_total_in_key {
                stats.push_str(&ksm.get_total(kc.vk).to_string());
            }
            if cfg.show_kps_in_key {
                if !stats.is_empty() {
                    stats.push_str(" | ");
                }
                stats.push_str(&format!("{}/s", ksm.get_kps(kc.vk)));
            }
            let stat_col = cur_font.with_alpha((cur_font.a as f32 * 0.72) as u8);
            if let Some(fmt) = stat_fmt {
                let text_utf16: Vec<u16> = stats.encode_utf16().collect();
                let rect = D2D_RECT_F {
                    left: x + 2.0,
                    top: y + label_h - 2.0,
                    right: x + w - 2.0,
                    bottom: y + h - 2.0,
                };
                dc.DrawText(
                    &text_utf16,
                    fmt,
                    &rect,
                    surf.solid_brush(&stat_col.to_d2d(opacity)),
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_stat_box(
        surf: &D2dSurface,
        key_fmt: Option<&IDWriteTextFormat>,
        stat_fmt: Option<&IDWriteTextFormat>,
        cfg: &AppConfig,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg: RgbaColor,
        fg: RgbaColor,
        val: &str,
        sub: &str,
        opacity: f32,
    ) {
        let dc = &surf.dc;
        let r = cfg.key_radius.min(w * 0.5).min(h * 0.5).max(0.0);
        let rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            },
            radiusX: r,
            radiusY: r,
        };
        dc.FillRoundedRectangle(&rr, surf.solid_brush(&bg.to_d2d(opacity)));

        let top_h = h * 0.58;
        if let Some(fmt) = key_fmt {
            let u16s: Vec<u16> = val.encode_utf16().collect();
            let rect = D2D_RECT_F {
                left: x + 2.0,
                top: y + 2.0,
                right: x + w - 2.0,
                bottom: y + top_h,
            };
            dc.DrawText(
                &u16s,
                fmt,
                &rect,
                surf.solid_brush(&fg.to_d2d(opacity)),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }

        let sub_col = fg.with_alpha((fg.a as f32 * 0.72) as u8);
        if let Some(fmt) = stat_fmt {
            let u16s: Vec<u16> = sub.encode_utf16().collect();
            let rect = D2D_RECT_F {
                left: x + 2.0,
                top: y + top_h - 2.0,
                right: x + w - 2.0,
                bottom: y + h - 2.0,
            };
            dc.DrawText(
                &u16s,
                fmt,
                &rect,
                surf.solid_brush(&sub_col.to_d2d(opacity)),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_history_track(
        surf: &D2dSurface,
        cfg: &AppConfig,
        vk: u32,
        ksm: &KeyStateManager,
        x: f32,
        y: f32,
        key_w: f32,
        track_h: f32,
        press_color: RgbaColor,
        opacity: f32,
        now: Instant,
    ) -> bool {
        let dc = &surf.dc;
        let mut active = false;

        let ta = ((cfg.history_track_alpha as f32) * 2.55).clamp(0.0, 255.0) as u8;
        if ta > 0 {
            dc.FillRectangle(
                &D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + key_w,
                    bottom: y + track_h,
                },
                surf.solid_brush(&RgbaColor::new(255, 255, 255, ta).to_d2d(opacity)),
            );
        }

        if cfg.history_show_lines {
            let br = surf.solid_brush(&RgbaColor::new(255, 255, 255, 40).to_d2d(opacity));
            dc.DrawLine(D2D_POINT_2F { x, y }, D2D_POINT_2F { x, y: y + track_h }, br, 1.0, None);
            dc.DrawLine(
                D2D_POINT_2F { x: x + key_w, y },
                D2D_POINT_2F {
                    x: x + key_w,
                    y: y + track_h,
                },
                br,
                1.0,
                None,
            );
        }

        let block_w = (key_w - 4.0).max(2.0);
        let cx = x + 2.0;
        let grow_speed = cfg.history_grow_speed.max(10.0);
        let float_speed = cfg.history_float_speed.max(10.0);
        let max_block_h = (track_h * (cfg.history_block_max_pct / 100.0)).max(4.0);

        if ksm.is_down(vk) {
            if let Some(start) = ksm.get_press_start(vk) {
                active = true;
                let held_s = now.duration_since(start).as_secs_f32();
                let block_h = (held_s * grow_speed).clamp(3.0, max_block_h);
                let block_y = y + track_h - block_h;
                let col = press_color.with_alpha(cfg.history_block_alpha);
                let r = 3.0f32.min(block_w * 0.5).min(block_h * 0.5);
                dc.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx,
                            top: block_y,
                            right: cx + block_w,
                            bottom: block_y + block_h,
                        },
                        radiusX: r,
                        radiusY: r,
                    },
                    surf.solid_brush(&col.to_d2d(opacity)),
                );
            }
        }

        for ps in ksm.get_press_durations(vk) {
            let age_s = now.duration_since(ps.release_time).as_secs_f32();
            let mut block_h = (ps.duration_secs * grow_speed).clamp(3.0, max_block_h);
            let floated = age_s * float_speed;
            let mut block_y = y + track_h - block_h - floated;

            if block_y + block_h <= y {
                continue;
            }
            active = true;

            if block_y < y {
                block_h -= y - block_y;
                block_y = y;
            }
            if block_h <= 0.5 {
                continue;
            }

            let fade = 1.0 - (floated / (track_h + block_h).max(1.0)) * 0.78;
            let alpha = ((cfg.history_block_alpha as f32) * fade)
                .clamp(15.0, cfg.history_block_alpha as f32) as u8;
            let col = press_color.with_alpha(alpha);
            let r = 3.0f32.min(block_w * 0.5).min(block_h * 0.5);
            dc.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx,
                        top: block_y,
                        right: cx + block_w,
                        bottom: block_y + block_h,
                    },
                    radiusX: r,
                    radiusY: r,
                },
                surf.solid_brush(&col.to_d2d(opacity)),
            );
        }

        active
    }
}

impl Drop for DisplayOverlay {
    fn drop(&mut self) {
        // [H-6] COM 타겟/스왑체인을 먼저 해제한 뒤 HWND 파괴
        self.surface = None;
        unsafe {
            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
            if !self.ctx_ptr.is_null() {
                drop(Box::from_raw(self.ctx_ptr));
                self.ctx_ptr = std::ptr::null_mut();
            }
        }
    }
}

unsafe extern "system" fn display_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let cs = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return LRESULT(0);
    }
    if msg == WM_NCDESTROY {
        // [C-3] 창 파괴 시 포인터 분리
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        return DefWindowProcW(hwnd, msg, wp, lp);
    }

    let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const DisplayWindowContext;
    if ctx_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    // [C-3] 불변 참조(&)만 사용하여 재진입(WM_CAPTURECHANGED 등) 시 aliasing UB 완전 방지
    let ctx = &*ctx_ptr;

    match msg {
        WM_NCHITTEST => {
            if ctx.allow_whole_drag.get() {
                return LRESULT(HTCLIENT as isize);
            }
            let screen_x = (lp.0 & 0xFFFF) as i16 as i32;
            let screen_y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
            let scale = ctx.dpi_scale.get().max(0.5);
            let lx = (screen_x - ctx.win_x.get()) as f32 / scale;
            let ly = (screen_y - ctx.win_y.get()) as f32 / scale;
            let rects = ctx.hit_rects.borrow();
            for r in rects.iter() {
                if lx >= r.x && lx <= r.x + r.w && ly >= r.y && ly <= r.y + r.h {
                    return LRESULT(HTCLIENT as isize);
                }
            }
            return LRESULT(HTTRANSPARENT as isize);
        }
        WM_RBUTTONUP => {
            let host = ctx.state.hook_thread_hwnd.load(Ordering::Acquire);
            if host != 0 {
                let _ = PostMessageW(
                    HWND(host as *mut core::ffi::c_void),
                    WM_SHOW_TRAY_MENU,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            ctx.dragging.set(true);
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            {
                let cfg = ctx.state.config.read();
                ctx.drag_offset_x.set(pt.x - cfg.display_x);
                ctx.drag_offset_y.set(pt.y - cfg.display_y);
            }
            SetCapture(hwnd);
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            if ctx.dragging.get() {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                {
                    let mut cfg = ctx.state.config.write();
                    cfg.display_x = pt.x - ctx.drag_offset_x.get();
                    cfg.display_y = pt.y - ctx.drag_offset_y.get();
                }
                ctx.state.mark_config_changed();
            }
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            if ctx.dragging.get() {
                ctx.dragging.set(false);
                let _ = ReleaseCapture();
                ctx.state.mark_config_changed();
            }
            return LRESULT(0);
        }
        WM_CAPTURECHANGED => {
            ctx.dragging.set(false);
            return LRESULT(0);
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
