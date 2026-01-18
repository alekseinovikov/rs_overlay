use std::ptr::null_mut;

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::{
        Direct2D::{
            D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
            D2D1_COLOR_F, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
            D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_PIXEL_FORMAT, D2D1_RECT_F,
            D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device,
            ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
        },
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
            ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
        },
        DirectComposition::{
            DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
        },
        DirectWrite::{
            DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_WEIGHT_REGULAR, DWRITE_MEASURING_MODE_NATURAL, DWriteCreateFactory,
            IDWriteFactory, IDWriteTextFormat,
        },
        Dxgi::{
            DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SCALING_STRETCH,
            DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
        },
        Dxgi_Common::DXGI_SAMPLE_DESC,
    },
    System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
    UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
        GetSystemMetrics, HWND_TOPMOST, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage,
        RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
        SWP_NOSENDCHANGING, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, TranslateMessage, WM_DESTROY,
        WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    },
};

use crate::FpsTracker;

const D3D_DRIVER_TYPE_HARDWARE: i32 = 1;

pub fn run() -> anyhow::Result<()> {
    unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED) };
    let _com_guard = ComGuard;

    let class_name = widestring::U16CString::from_str("rs_overlay_window").expect("class name");
    let hinstance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null_mut()) };

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: hinstance,
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }

    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            0,
            0,
            hinstance,
            null_mut(),
        )
    };

    if hwnd == 0 {
        return Err(anyhow::anyhow!("failed to create window"));
    }

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOSENDCHANGING | SWP_SHOWWINDOW,
        );
    }

    let mut gfx = D3DState::new(hwnd, width as u32, height as u32)?;
    let mut fps_tracker = FpsTracker::new();

    'running: loop {
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_DESTROY {
                    break 'running;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        fps_tracker.tick();
        gfx.render(fps_tracker.fps());
    }

    Ok(())
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct D3DState {
    swap_chain: *mut IDXGISwapChain1,
    device: *mut ID3D11Device,
    context: *mut ID3D11DeviceContext,
    rtv: *mut ID3D11RenderTargetView,
    composition: *mut IDCompositionDevice,
    target: *mut IDCompositionTarget,
    visual: *mut IDCompositionVisual,
    d2d_context: *mut ID2D1DeviceContext,
    d2d_target: *mut ID2D1Bitmap1,
    d2d_brush: *mut ID2D1SolidColorBrush,
    dwrite_factory: *mut IDWriteFactory,
    text_format: *mut IDWriteTextFormat,
    width: u32,
    height: u32,
}

impl D3DState {
    fn new(hwnd: HWND, width: u32, height: u32) -> anyhow::Result<Self> {
        let mut device: *mut ID3D11Device = null_mut();
        let mut context: *mut ID3D11DeviceContext = null_mut();
        let mut feature_level = 0;

        let hr = unsafe {
            D3D11CreateDevice(
                null_mut(),
                D3D_DRIVER_TYPE_HARDWARE,
                0,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                null_mut(),
                0,
                D3D11_SDK_VERSION,
                &mut device,
                &mut feature_level,
                &mut context,
            )
        };
        if hr < 0 {
            return Err(anyhow::anyhow!("D3D11CreateDevice failed: {hr:#x}"));
        }

        let mut dxgi_device: *mut IDXGIDevice = null_mut();
        let hr = unsafe {
            (*device).QueryInterface(&IDXGIDevice::IID, &mut dxgi_device as *mut _ as *mut _)
        };
        if hr < 0 {
            return Err(anyhow::anyhow!(
                "QueryInterface IDXGIDevice failed: {hr:#x}"
            ));
        }

        let mut factory: *mut IDXGIFactory2 = null_mut();
        let hr = unsafe {
            (*dxgi_device).GetParent(&IDXGIFactory2::IID, &mut factory as *mut _ as *mut _)
        };
        if hr < 0 {
            return Err(anyhow::anyhow!("GetParent IDXGIFactory2 failed: {hr:#x}"));
        }

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: 0,
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

        let mut swap_chain: *mut IDXGISwapChain1 = null_mut();
        let hr = unsafe {
            (*factory).CreateSwapChainForComposition(
                device as *mut _,
                &desc,
                null_mut(),
                &mut swap_chain,
            )
        };
        if hr < 0 {
            return Err(anyhow::anyhow!(
                "CreateSwapChainForComposition failed: {hr:#x}"
            ));
        }

        let mut composition: *mut IDCompositionDevice = null_mut();
        let hr = unsafe {
            DCompositionCreateDevice(
                dxgi_device as *mut _,
                &IDCompositionDevice::IID,
                &mut composition as *mut _ as *mut _,
            )
        };
        if hr < 0 {
            return Err(anyhow::anyhow!("DCompositionCreateDevice failed: {hr:#x}"));
        }

        let mut target: *mut IDCompositionTarget = null_mut();
        let hr = unsafe { (*composition).CreateTargetForHwnd(hwnd, 1, &mut target) };
        if hr < 0 {
            return Err(anyhow::anyhow!("CreateTargetForHwnd failed: {hr:#x}"));
        }

        let mut visual: *mut IDCompositionVisual = null_mut();
        let hr = unsafe { (*composition).CreateVisual(&mut visual) };
        if hr < 0 {
            return Err(anyhow::anyhow!("CreateVisual failed: {hr:#x}"));
        }

        let hr = unsafe { (*visual).SetContent(swap_chain as *mut _) };
        if hr < 0 {
            return Err(anyhow::anyhow!("SetContent failed: {hr:#x}"));
        }

        let hr = unsafe { (*target).SetRoot(visual) };
        if hr < 0 {
            return Err(anyhow::anyhow!("SetRoot failed: {hr:#x}"));
        }

        let hr = unsafe { (*composition).Commit() };
        if hr < 0 {
            return Err(anyhow::anyhow!("DComposition Commit failed: {hr:#x}"));
        }

        let rtv = unsafe { create_rtv(device, swap_chain)? };
        let (d2d_context, d2d_target, d2d_brush, dwrite_factory, text_format) =
            unsafe { create_text_pipeline(dxgi_device, swap_chain)? };

        Ok(Self {
            swap_chain,
            device,
            context,
            rtv,
            composition,
            target,
            visual,
            d2d_context,
            d2d_target,
            d2d_brush,
            dwrite_factory,
            text_format,
            width,
            height,
        })
    }

    fn render(&mut self, fps: f32) {
        unsafe {
            let clear = [0.0, 0.0, 0.0, 0.0];
            (*self.context).ClearRenderTargetView(self.rtv, clear.as_ptr());

            (*self.d2d_context).BeginDraw();
            let clear_color = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
            (*self.d2d_context).Clear(&clear_color);

            let text = format!("FPS: {:.1}", fps);
            let text_w = widestring::U16CString::from_str(text).expect("fps text");
            let rect = D2D1_RECT_F {
                left: 12.0,
                top: 12.0,
                right: self.width as f32,
                bottom: 48.0,
            };
            (*self.d2d_context).DrawTextW(
                text_w.as_ptr(),
                text_w.len() as u32,
                self.text_format,
                &rect,
                self.d2d_brush as *mut _,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            let _ = (*self.d2d_context).EndDraw(null_mut(), null_mut());
            let _ = (*self.swap_chain).Present(1, 0);
            let _ = (*self.composition).Commit();
        }
    }
}

unsafe fn create_rtv(
    device: *mut ID3D11Device,
    swap_chain: *mut IDXGISwapChain1,
) -> anyhow::Result<*mut ID3D11RenderTargetView> {
    let mut back_buffer: *mut ID3D11Texture2D = null_mut();
    let hr = (*swap_chain).GetBuffer(
        0,
        &ID3D11Texture2D::IID,
        &mut back_buffer as *mut _ as *mut _,
    );
    if hr < 0 {
        return Err(anyhow::anyhow!("GetBuffer failed: {hr:#x}"));
    }

    let mut rtv: *mut ID3D11RenderTargetView = null_mut();
    let hr = (*device).CreateRenderTargetView(back_buffer as *mut _, null_mut(), &mut rtv);
    if hr < 0 {
        return Err(anyhow::anyhow!("CreateRenderTargetView failed: {hr:#x}"));
    }

    Ok(rtv)
}

unsafe fn create_text_pipeline(
    dxgi_device: *mut IDXGIDevice,
    swap_chain: *mut IDXGISwapChain1,
) -> anyhow::Result<(
    *mut ID2D1DeviceContext,
    *mut ID2D1Bitmap1,
    *mut ID2D1SolidColorBrush,
    *mut IDWriteFactory,
    *mut IDWriteTextFormat,
)> {
    let mut d2d_factory: *mut ID2D1Factory1 = null_mut();
    let hr = D2D1CreateFactory(
        D2D1_FACTORY_TYPE_SINGLE_THREADED,
        &ID2D1Factory1::IID,
        null_mut(),
        &mut d2d_factory as *mut _ as *mut _,
    );
    if hr < 0 {
        return Err(anyhow::anyhow!("D2D1CreateFactory failed: {hr:#x}"));
    }

    let mut d2d_device: *mut ID2D1Device = null_mut();
    let hr = (*d2d_factory).CreateDevice(dxgi_device as *mut _, &mut d2d_device);
    if hr < 0 {
        return Err(anyhow::anyhow!("CreateDevice (D2D) failed: {hr:#x}"));
    }

    let mut d2d_context: *mut ID2D1DeviceContext = null_mut();
    let hr = (*d2d_device).CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, &mut d2d_context);
    if hr < 0 {
        return Err(anyhow::anyhow!("CreateDeviceContext failed: {hr:#x}"));
    }

    let mut surface: *mut windows_sys::Win32::Graphics::Dxgi::IDXGISurface = null_mut();
    let hr = (*swap_chain).GetBuffer(
        0,
        &windows_sys::Win32::Graphics::Dxgi::IDXGISurface::IID,
        &mut surface as *mut _ as *mut _,
    );
    if hr < 0 {
        return Err(anyhow::anyhow!("GetBuffer IDXGISurface failed: {hr:#x}"));
    }

    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        colorContext: null_mut(),
    };

    let mut target: *mut ID2D1Bitmap1 = null_mut();
    let hr = (*d2d_context).CreateBitmapFromDxgiSurface(surface, &props, &mut target);
    if hr < 0 {
        return Err(anyhow::anyhow!(
            "CreateBitmapFromDxgiSurface failed: {hr:#x}"
        ));
    }
    (*d2d_context).SetTarget(target as *mut _);
    (*d2d_context).SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);

    let mut dwrite_factory: *mut IDWriteFactory = null_mut();
    let hr = DWriteCreateFactory(
        DWRITE_FACTORY_TYPE_SHARED,
        &IDWriteFactory::IID,
        &mut dwrite_factory as *mut _ as *mut _,
    );
    if hr < 0 {
        return Err(anyhow::anyhow!("DWriteCreateFactory failed: {hr:#x}"));
    }

    let font_name = widestring::U16CString::from_str("Segoe UI").expect("font");
    let mut text_format: *mut IDWriteTextFormat = null_mut();
    let hr = (*dwrite_factory).CreateTextFormat(
        font_name.as_ptr(),
        null_mut(),
        DWRITE_FONT_WEIGHT_REGULAR,
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        18.0,
        null_mut(),
        &mut text_format,
    );
    if hr < 0 {
        return Err(anyhow::anyhow!("CreateTextFormat failed: {hr:#x}"));
    }

    let mut brush: *mut ID2D1SolidColorBrush = null_mut();
    let text_color = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let hr = (*d2d_context).CreateSolidColorBrush(&text_color, null_mut(), &mut brush);
    if hr < 0 {
        return Err(anyhow::anyhow!("CreateSolidColorBrush failed: {hr:#x}"));
    }

    Ok((d2d_context, target, brush, dwrite_factory, text_format))
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
