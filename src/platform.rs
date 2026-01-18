#[cfg(windows)]
mod windows {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongW, SetWindowLongW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    use winit::platform::windows::WindowExtWindows;

    pub(super) fn configure(window: &winit::window::Window) {
        let hwnd = window.hwnd();
        unsafe {
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let new_style =
                ex_style | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}

pub fn configure_overlay(window: &winit::window::Window) {
    #[cfg(windows)]
    {
        windows::configure(window);
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}
