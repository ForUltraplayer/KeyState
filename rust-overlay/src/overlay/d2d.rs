use windows::core::{Interface, Result, HSTRING};
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1,
    ID2D1SolidColorBrush, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_TEXT_ALIGNMENT, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1, DXGI_ERROR_DEVICE_REMOVED,
    DXGI_ERROR_DEVICE_RESET, DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

/// [H-6] Direct2D / DXGI 디바이스 분실 에러 판별
pub fn is_device_lost(err: &windows::core::Error) -> bool {
    let code = err.code();
    code == D2DERR_RECREATE_TARGET
        || code == DXGI_ERROR_DEVICE_REMOVED
        || code == DXGI_ERROR_DEVICE_RESET
}

pub struct D2dShared {
    pub dcomp_device: IDCompositionDevice,
    pub dwrite_factory: IDWriteFactory,
    pub d2d_device: ID2D1Device,
    pub d2d_factory: ID2D1Factory1,
    pub dxgi_factory: IDXGIFactory2,
    pub dxgi_device: IDXGIDevice,
    pub _d3d_device: ID3D11Device,
}

impl D2dShared {
    pub fn new() -> Result<Self> {
        unsafe {
            let feature_levels = [
                D3D_FEATURE_LEVEL_11_1,
                D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_10_1,
                D3D_FEATURE_LEVEL_10_0,
            ];
            let mut d3d_device: Option<ID3D11Device> = None;
            let hw = D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                None,
            );
            if hw.is_err() {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut d3d_device),
                    None,
                    None,
                )?;
            }
            let d3d_device = d3d_device.unwrap();
            let dxgi_device: IDXGIDevice = d3d_device.cast()?;
            let dxgi_adapter = dxgi_device.GetAdapter()?;
            let dxgi_factory: IDXGIFactory2 = dxgi_adapter.GetParent()?;

            let d2d_factory: ID2D1Factory1 = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?;
            let d2d_device = d2d_factory.CreateDevice(&dxgi_device)?;
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let dcomp_device: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;

            Ok(Self {
                dcomp_device,
                dwrite_factory,
                d2d_device,
                d2d_factory,
                dxgi_factory,
                dxgi_device,
                _d3d_device: d3d_device,
            })
        }
    }

    pub fn create_text_format(
        &self,
        font_name: &str,
        size: f32,
        weight: DWRITE_FONT_WEIGHT,
        align: DWRITE_TEXT_ALIGNMENT,
        valign: DWRITE_PARAGRAPH_ALIGNMENT,
    ) -> Result<IDWriteTextFormat> {
        unsafe {
            let family = HSTRING::from(font_name);
            let locale = HSTRING::from("ko-KR");
            let fmt = self.dwrite_factory.CreateTextFormat(
                &family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size.max(6.0),
                &locale,
            )?;
            fmt.SetTextAlignment(align)?;
            fmt.SetParagraphAlignment(valign)?;
            fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            Ok(fmt)
        }
    }
}

/// [H-5] 재사용 가능한 공용 브러시 캐시 + [H-6] 안전한 COM 해제 순서 보장
pub struct D2dSurface {
    bitmap: Option<ID2D1Bitmap1>,
    solid_brush: ID2D1SolidColorBrush,
    target: IDCompositionTarget,
    visual: IDCompositionVisual,
    pub dc: ID2D1DeviceContext,
    pub swap_chain: IDXGISwapChain1,
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
}

impl D2dSurface {
    pub fn new(shared: &D2dShared, hwnd: HWND, width: u32, height: u32, dpi: f32) -> Result<Self> {
        let w = width.max(1);
        let h = height.max(1);
        let dpi = dpi.max(96.0);
        unsafe {
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: w,
                Height: h,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };

            let swap_chain = shared.dxgi_factory.CreateSwapChainForComposition(
                &shared.dxgi_device,
                &desc,
                None,
            )?;
            let dc = shared
                .d2d_device
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;

            let solid_brush = dc.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )?;

            let target = shared.dcomp_device.CreateTargetForHwnd(hwnd, true)?;
            let visual = shared.dcomp_device.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            shared.dcomp_device.Commit()?;

            let mut surf = Self {
                bitmap: None,
                solid_brush,
                target,
                visual,
                dc,
                swap_chain,
                width: w,
                height: h,
                dpi,
            };
            surf.bind_backbuffer()?;
            Ok(surf)
        }
    }

    /// [H-5] 매 프레임 COM 할당 없이 단일 브러시 색상만 변경해 재사용
    #[inline]
    pub unsafe fn solid_brush(&self, color: &D2D1_COLOR_F) -> &ID2D1SolidColorBrush {
        self.solid_brush.SetColor(color);
        &self.solid_brush
    }

    fn bind_backbuffer(&mut self) -> Result<()> {
        unsafe {
            let dxgi_surface: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            let props = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: self.dpi,
                dpiY: self.dpi,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            };
            let bmp = self
                .dc
                .CreateBitmapFromDxgiSurface(&dxgi_surface, Some(&props))?;
            self.dc.SetDpi(self.dpi, self.dpi);
            self.dc.SetTarget(&bmp);
            self.bitmap = Some(bmp);
            Ok(())
        }
    }

    pub fn resize(&mut self, shared: &D2dShared, width: u32, height: u32, dpi: f32) -> Result<()> {
        let w = width.max(1);
        let h = height.max(1);
        let dpi = dpi.max(96.0);
        if self.width == w && self.height == h && (self.dpi - dpi).abs() < 0.5 {
            return Ok(());
        }
        unsafe {
            self.dc.SetTarget(None);
            self.bitmap = None;
            self.swap_chain.ResizeBuffers(
                2,
                w,
                h,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
            self.width = w;
            self.height = h;
            self.dpi = dpi;
            self.bind_backbuffer()?;
            shared.dcomp_device.Commit()?;
        }
        Ok(())
    }

    pub fn present(&self) -> Result<()> {
        unsafe { self.swap_chain.Present(0, DXGI_PRESENT(0)).ok() }
    }
}

impl Drop for D2dSurface {
    fn drop(&mut self) {
        unsafe {
            self.dc.SetTarget(None);
            self.bitmap = None;
            let _ = self.visual.SetContent(None);
            let _ = self.target.SetRoot(None);
        }
    }
}
