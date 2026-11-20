use crate::config::{AppConfig, ChartType, RgbaColor};
use crate::input::hook::WM_SHOW_TRAY_MENU;
use crate::input::state::KeyStateManager;
use crate::overlay::d2d::{D2dShared, D2dSurface};
use crate::state::SharedState;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::{w, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_GRADIENT_STOP,
    D2D_POINT_2F, D2D_RECT_F,
};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteTextFormat, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetWindowLongPtrW,
    GetWindowLongW, LoadCursorW, PostMessageW, RegisterClassExW, SetWindowLongPtrW,
    SetWindowLongW, SetWindowPos, ShowWindow, CREATESTRUCTW, GWL_EXSTYLE, GWLP_USERDATA,
    HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WM_CAPTURECHANGED, WM_CREATE, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY, WM_RBUTTONUP, WNDCLASSEXW, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

#[derive(Clone, Copy)]
struct KpsSample {
    time: Instant,
    kps: f32,
}

/// [C-3] wnd_proc 재진입 시 aliasing UB 방지를 위해 Cell 기반 내부 가변성 사용
struct ChartWindowContext {
    state: Arc<SharedState>,
    dragging: Cell<bool>,
    drag_offset_x: Cell<i32>,
    drag_offset_y: Cell<i32>,
}

pub struct ChartOverlay {
    hwnd: HWND,
    surface: Option<D2dSurface>,
    ctx_ptr: *mut ChartWindowContext,
    history: VecDeque<KpsSample>,
    last_sample: Option<Instant>,
    y_max_smooth: f32,
    fmt_right: Option<IDWriteTextFormat>,
    fmt_center: Option<IDWriteTextFormat>,
    fmt_left: Option<IDWriteTextFormat>,
    fmt_cur: Option<IDWriteTextFormat>,
    last_visible: bool,
    last_click_through: bool,
    last_rect: (i32, i32, u32, u32),
}

impl ChartOverlay {
    pub fn new(shared: &D2dShared, state: Arc<SharedState>) -> Result<Self> {
        let cfg = state.config.read().clone();
        let dip_w = cfg.chart_w.max(120) as u32;
        let dip_h = cfg.chart_h.max(80) as u32;

        let ctx_ptr = Box::into_raw(Box::new(ChartWindowContext {
            state: state.clone(),
            dragging: Cell::new(false),
            drag_offset_x: Cell::new(0),
            drag_offset_y: Cell::new(0),
        }));

        let hwnd = unsafe {
            let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
            let class_name = w!("KeyStateRsChartOverlay");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(chart_wnd_proc),
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
                w!("KeyState-RS Chart"),
                WS_POPUP,
                cfg.chart_x,
                cfg.chart_y,
                dip_w as i32,
                dip_h as i32,
                None,
                None,
                hinst,
                Some(ctx_ptr as *const core::ffi::c_void),
            ) {
                Ok(h) => h,
                Err(e) => {
                    drop(Box::from_raw(ctx_ptr));
                    return Err(e);
                }
            }
        };

        let dpi = unsafe { GetDpiForWindow(hwnd).max(96) as f32 };
        let scale = dpi / 96.0;
        let phys_w = ((dip_w as f32 * scale).round() as u32).max(1);
        let phys_h = ((dip_h as f32 * scale).round() as u32).max(1);

        let surface = match D2dSurface::new(shared, hwnd, phys_w, phys_h, dpi) {
            Ok(s) => s,
            Err(e) => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                    drop(Box::from_raw(ctx_ptr));
                }
                return Err(e);
            }
        };

        let fmt_right = shared
            .create_text_format(
                "Segoe UI",
                10.0,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            )
            .ok();
        let fmt_center = shared
            .create_text_format(
                "Segoe UI",
                10.0,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            )
            .ok();
        let fmt_left = shared
            .create_text_format(
                "Segoe UI",
                10.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            )
            .ok();
        let fmt_cur = shared
            .create_text_format(
                "Segoe UI",
                13.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            )
            .ok();

        unsafe {
            let _ = ShowWindow(hwnd, if cfg.show_chart { SW_SHOWNA } else { SW_HIDE });
        }

        Ok(Self {
            hwnd,
            surface: Some(surface),
            ctx_ptr,
            history: VecDeque::new(),
            last_sample: None,
            y_max_smooth: 20.0,
            fmt_right,
            fmt_center,
            fmt_left,
            fmt_cur,
            last_visible: cfg.show_chart,
            last_click_through: cfg.click_through,
            last_rect: (cfg.chart_x, cfg.chart_y, phys_w, phys_h),
        })
    }

    pub fn is_dragging(&self) -> bool {
        unsafe { (*self.ctx_ptr).dragging.get() }
    }

    pub fn update_and_render(
        &mut self,
        shared: &D2dShared,
        cfg: &AppConfig,
        ksm: &KeyStateManager,
        display_rect: (i32, i32, u32, u32),
        cfg_changed: bool,
        now: Instant,
    ) -> Result<bool> {
        if cfg.show_chart != self.last_visible {
            self.last_visible = cfg.show_chart;
            unsafe {
                let _ = ShowWindow(self.hwnd, if cfg.show_chart { SW_SHOWNA } else { SW_HIDE });
            }
        }
        if !cfg.show_chart {
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

        let dpi = unsafe { GetDpiForWindow(self.hwnd).max(96) as f32 };
        let scale = dpi / 96.0;
        let dip_w = cfg.chart_w.clamp(140, 1200) as f32;
        let dip_h = cfg.chart_h.clamp(80, 600) as f32;
        let phys_w = ((dip_w * scale).round() as u32).max(1);
        let phys_h = ((dip_h * scale).round() as u32).max(1);

        let (cx, cy) = if cfg.chart_snap {
            (
                display_rect.0 + (cfg.chart_snap_offset_x as f32 * scale).round() as i32,
                display_rect.1
                    + display_rect.3 as i32
                    + (cfg.chart_snap_offset_y as f32 * scale).round() as i32,
            )
        } else {
            (cfg.chart_x, cfg.chart_y)
        };

        if cfg_changed || self.last_rect != (cx, cy, phys_w, phys_h) {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    if cfg.always_on_top {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    cx,
                    cy,
                    phys_w as i32,
                    phys_h as i32,
                    SWP_NOACTIVATE,
                );
            }
            if let Some(surf) = self.surface.as_mut() {
                surf.resize(shared, phys_w, phys_h, dpi)?;
            }
            self.last_rect = (cx, cy, phys_w, phys_h);
        }

        // [L-2] 중복 VK 이중 합산 방지
        let mut unique_vks: Vec<u32> = cfg.keys.iter().map(|k| k.vk).collect();
        unique_vks.sort_unstable();
        unique_vks.dedup();
        let cur_kps: f32 = unique_vks.iter().map(|&vk| ksm.get_kps(vk) as f32).sum();

        let t_range_ms = cfg.chart_time_range_ms.clamp(1000, 30_000);
        let mg_l = 34.0f32;
        let mg_r = 14.0f32;
        let mg_t = 18.0f32;
        let mg_b = 22.0f32;
        let plot_w = (dip_w - mg_l - mg_r).max(10.0);
        let plot_h = (dip_h - mg_t - mg_b).max(10.0);

        let sample_ms = (t_range_ms as f32 / plot_w).clamp(16.0, 200.0) as u64;
        let should_push = match self.last_sample {
            None => true,
            Some(prev) => now.duration_since(prev) >= Duration::from_millis(sample_ms),
        };
        if should_push {
            self.history.push_back(KpsSample {
                time: now,
                kps: cur_kps,
            });
            self.last_sample = Some(now);
        }
        while let Some(front) = self.history.front() {
            if now.duration_since(front.time).as_millis() as u64 > t_range_ms {
                self.history.pop_front();
            } else {
                break;
            }
        }

        let peak = self.history.iter().fold(0.0f32, |acc, s| acc.max(s.kps));
        let target_ymax = ((peak.max(20.0) / 5.0).ceil()) * 5.0;
        let blend = if target_ymax > self.y_max_smooth { 0.14 } else { 0.05 };
        self.y_max_smooth += (target_ymax - self.y_max_smooth) * blend;
        let y_max = self.y_max_smooth.max(5.0);

        let Some(surf) = self.surface.as_ref() else {
            return Ok(false);
        };
        let opacity = cfg.overlay_opacity.clamp(0.05, 1.0);
        let wf = dip_w;
        let hf = dip_h;

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

            let r = cfg.chart_radius.min(wf * 0.5).min(hf * 0.5).max(0.0);
            let bg_rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 1.0,
                    top: 1.0,
                    right: wf - 1.0,
                    bottom: hf - 1.0,
                },
                radiusX: r,
                radiusY: r,
            };
            dc.FillRoundedRectangle(&bg_rr, surf.solid_brush(&cfg.chart_bg.to_d2d(opacity)));
            dc.DrawRoundedRectangle(
                &bg_rr,
                surf.solid_brush(&RgbaColor::new(255, 255, 255, 35).to_d2d(opacity)),
                1.0,
                None,
            );

            let n_x = 5usize;
            if cfg.chart_show_grid {
                let grid_br =
                    surf.solid_brush(&RgbaColor::new(180, 190, 210, 32).to_d2d(opacity));
                for i in 0..=4 {
                    let gy = mg_t + plot_h - (plot_h * i as f32 / 4.0);
                    dc.DrawLine(
                        D2D_POINT_2F { x: mg_l, y: gy },
                        D2D_POINT_2F {
                            x: mg_l + plot_w,
                            y: gy,
                        },
                        grid_br,
                        0.75,
                        None,
                    );
                }
                for i in 0..=n_x {
                    let gx = mg_l + (plot_w * i as f32 / n_x as f32);
                    dc.DrawLine(
                        D2D_POINT_2F { x: gx, y: mg_t },
                        D2D_POINT_2F {
                            x: gx,
                            y: mg_t + plot_h,
                        },
                        grid_br,
                        0.75,
                        None,
                    );
                }
            }

            {
                let lbl_br =
                    surf.solid_brush(&RgbaColor::new(175, 185, 200, 175).to_d2d(opacity));
                if let Some(fmt) = self.fmt_right.as_ref() {
                    for i in 0..=4 {
                        let val = ((y_max * i as f32 / 4.0).round() as i32).to_string();
                        let u16s: Vec<u16> = val.encode_utf16().collect();
                        let gy = mg_t + plot_h - (plot_h * i as f32 / 4.0);
                        dc.DrawText(
                            &u16s,
                            fmt,
                            &D2D_RECT_F {
                                left: 2.0,
                                top: gy - 7.0,
                                right: mg_l - 5.0,
                                bottom: gy + 7.0,
                            },
                            lbl_br,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }
                if let Some(fmt) = self.fmt_left.as_ref() {
                    let u16s: Vec<u16> = "KPS".encode_utf16().collect();
                    dc.DrawText(
                        &u16s,
                        fmt,
                        &D2D_RECT_F {
                            left: mg_l,
                            top: 2.0,
                            right: mg_l + 60.0,
                            bottom: mg_t,
                        },
                        lbl_br,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
                if let Some(fmt) = self.fmt_center.as_ref() {
                    let t_sec = ((t_range_ms + 999) / 1000) as i32;
                    for i in 0..=n_x {
                        let sec = -t_sec + (t_sec * i as i32 / n_x as i32);
                        let txt = format!("{sec}s");
                        let u16s: Vec<u16> = txt.encode_utf16().collect();
                        let gx = mg_l + (plot_w * i as f32 / n_x as f32);
                        dc.DrawText(
                            &u16s,
                            fmt,
                            &D2D_RECT_F {
                                left: gx - 16.0,
                                top: mg_t + plot_h + 2.0,
                                right: gx + 16.0,
                                bottom: hf - 2.0,
                            },
                            lbl_br,
                            D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
                    }
                }
            }

            let map_x = |t: Instant| -> f32 {
                let age_ms = now.duration_since(t).as_secs_f32() * 1000.0;
                let ratio = (1.0 - age_ms / t_range_ms as f32).clamp(0.0, 1.0);
                mg_l + ratio * plot_w
            };
            let map_y = |kps: f32| -> f32 {
                let ratio = (kps / y_max).clamp(0.0, 1.0);
                mg_t + plot_h - ratio * plot_h
            };

            let lc = cfg.chart_line;
            dc.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: mg_l,
                    top: mg_t,
                    right: mg_l + plot_w,
                    bottom: mg_t + plot_h,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );

            match cfg.chart_type {
                ChartType::Line => {
                    if self.history.len() >= 2 {
                        if let Ok(path) = shared.d2d_factory.CreatePathGeometry() {
                            if let Ok(sink) = path.Open() {
                                let first = self.history.front().unwrap();
                                sink.BeginFigure(
                                    D2D_POINT_2F {
                                        x: map_x(first.time),
                                        y: mg_t + plot_h,
                                    },
                                    D2D1_FIGURE_BEGIN_FILLED,
                                );
                                for s in &self.history {
                                    sink.AddLine(D2D_POINT_2F {
                                        x: map_x(s.time),
                                        y: map_y(s.kps),
                                    });
                                }
                                let last = self.history.back().unwrap();
                                sink.AddLine(D2D_POINT_2F {
                                    x: map_x(last.time),
                                    y: mg_t + plot_h,
                                });
                                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                                let _ = sink.Close();

                                if cfg.chart_gradient_fill {
                                    let stops = [
                                        D2D1_GRADIENT_STOP {
                                            position: 0.0,
                                            color: lc.with_alpha(95).to_d2d(opacity),
                                        },
                                        D2D1_GRADIENT_STOP {
                                            position: 1.0,
                                            color: lc.with_alpha(8).to_d2d(opacity),
                                        },
                                    ];
                                    if let Ok(stop_col) =
                                        ID2D1RenderTarget::CreateGradientStopCollection(
                                            dc,
                                            &stops,
                                            D2D1_GAMMA_2_2,
                                            D2D1_EXTEND_MODE_CLAMP,
                                        )
                                    {
                                        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                                            startPoint: D2D_POINT_2F { x: mg_l, y: mg_t },
                                            endPoint: D2D_POINT_2F {
                                                x: mg_l,
                                                y: mg_t + plot_h,
                                            },
                                        };
                                        if let Ok(gbr) =
                                            dc.CreateLinearGradientBrush(&props, None, &stop_col)
                                        {
                                            dc.FillGeometry(&path, &gbr, None);
                                        }
                                    }
                                } else {
                                    dc.FillGeometry(
                                        &path,
                                        surf.solid_brush(&lc.with_alpha(40).to_d2d(opacity)),
                                        None,
                                    );
                                }
                            }
                        }

                        let line_br = surf.solid_brush(&lc.to_d2d(opacity));
                        let mut iter = self.history.iter();
                        if let Some(mut prev) = iter.next() {
                            for cur in iter {
                                dc.DrawLine(
                                    D2D_POINT_2F {
                                        x: map_x(prev.time),
                                        y: map_y(prev.kps),
                                    },
                                    D2D_POINT_2F {
                                        x: map_x(cur.time),
                                        y: map_y(cur.kps),
                                    },
                                    line_br,
                                    2.0,
                                    None,
                                );
                                prev = cur;
                            }
                        }
                        if let Some(last) = self.history.back() {
                            dc.FillEllipse(
                                &D2D1_ELLIPSE {
                                    point: D2D_POINT_2F {
                                        x: map_x(last.time),
                                        y: map_y(last.kps),
                                    },
                                    radiusX: 3.0,
                                    radiusY: 3.0,
                                },
                                line_br,
                            );
                        }
                    }
                }
                ChartType::Scatter => {
                    let dot_br = surf.solid_brush(&lc.to_d2d(opacity));
                    for s in &self.history {
                        dc.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: D2D_POINT_2F {
                                    x: map_x(s.time),
                                    y: map_y(s.kps),
                                },
                                radiusX: 3.0,
                                radiusY: 3.0,
                            },
                            dot_br,
                        );
                    }
                }
                ChartType::Bar => {
                    if !self.history.is_empty() {
                        let bar_w = (plot_w / self.history.len() as f32 * 0.72).max(1.5);
                        let bar_br = surf.solid_brush(&lc.with_alpha(200).to_d2d(opacity));
                        for s in &self.history {
                            let bx = map_x(s.time) - bar_w * 0.5;
                            let by = map_y(s.kps);
                            dc.FillRectangle(
                                &D2D_RECT_F {
                                    left: bx,
                                    top: by,
                                    right: bx + bar_w,
                                    bottom: mg_t + plot_h,
                                },
                                bar_br,
                            );
                        }
                    }
                }
            }

            dc.PopAxisAlignedClip();

            if let Some(fmt) = self.fmt_cur.as_ref() {
                let txt = format!("{} KPS", cur_kps as u32);
                let u16s: Vec<u16> = txt.encode_utf16().collect();
                dc.DrawText(
                    &u16s,
                    fmt,
                    &D2D_RECT_F {
                        left: mg_l + plot_w - 80.0,
                        top: 1.0,
                        right: mg_l + plot_w,
                        bottom: mg_t + 2.0,
                    },
                    surf.solid_brush(&lc.to_d2d(opacity)),
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            dc.EndDraw(None, None)?;
        }

        surf.present()?;
        Ok(peak > 0.01 || cur_kps > 0.01 || (self.y_max_smooth - 20.0).abs() > 0.2)
    }
}

impl Drop for ChartOverlay {
    fn drop(&mut self) {
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

unsafe extern "system" fn chart_wnd_proc(
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
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        return DefWindowProcW(hwnd, msg, wp, lp);
    }

    let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ChartWindowContext;
    if ctx_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let ctx = &*ctx_ptr;

    match msg {
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
            if ctx.state.config.read().chart_snap {
                return LRESULT(0);
            }
            ctx.dragging.set(true);
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            {
                let cfg = ctx.state.config.read();
                ctx.drag_offset_x.set(pt.x - cfg.chart_x);
                ctx.drag_offset_y.set(pt.y - cfg.chart_y);
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
                    cfg.chart_x = pt.x - ctx.drag_offset_x.get();
                    cfg.chart_y = pt.y - ctx.drag_offset_y.get();
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
