#[cfg(windows)]
mod windows {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub(super) fn configure(window: &winit::window::Window) {
        let handle = match window.window_handle() {
            Ok(handle) => handle,
            Err(_) => return,
        };
        let hwnd = match handle.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get() as _,
            _ => return,
        };
        unsafe {
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let new_style = ex_style
                | (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as i32;
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
            let _ = SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{msg_send, runtime::Object};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub(super) fn configure(window: &winit::window::Window) {
        let handle = match window.window_handle() {
            Ok(handle) => handle,
            Err(_) => return,
        };

        if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
            let ns_view = handle.ns_view.as_ptr().cast::<Object>();
            unsafe {
                let ns_window: *mut Object = msg_send![ns_view, window];
                if !ns_window.is_null() {
                    let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
                }
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod unix {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::protocol::shape::ConnectionExt as _;
    use x11rb::{
        connection::Connection as _,
        protocol::{shape, xproto},
        rust_connection::RustConnection,
    };

    pub(super) fn configure(window: &winit::window::Window) {
        let handle = match window.window_handle() {
            Ok(handle) => handle,
            Err(_) => return,
        };

        let window_id = match handle.as_raw() {
            RawWindowHandle::Xlib(handle) => handle.window as u32,
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            RawWindowHandle::Wayland(_) => return,
            _ => return,
        };

        let (conn, _) = match RustConnection::connect(None) {
            Ok(connection) => connection,
            Err(_) => return,
        };

        let _ = conn.shape_rectangles(
            shape::SO::SET,
            shape::SK::INPUT,
            xproto::ClipOrdering::UNSORTED,
            window_id,
            0,
            0,
            &[],
        );
        let _ = conn.flush();
    }
}

pub fn configure_overlay(window: &winit::window::Window) {
    #[cfg(windows)]
    {
        windows::configure(window);
    }
    #[cfg(target_os = "macos")]
    {
        macos::configure(window);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        unix::configure(window);
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}
