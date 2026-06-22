// Hooks the saver window up to the tiny preview HWND that the Windows
// screensaver settings panel hands us via `/p <hwnd>`. We strip our own
// borders, SetParent into the panel control, then size to its client rect.

#![cfg(windows)]

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub fn attach_to_parent(window: &Window, parent_hwnd_raw: isize) {
    if parent_hwnd_raw == 0 {
        return;
    }
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let child_hwnd = h.hwnd.get();

    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongPtrW, MoveWindow, SetParent, SetWindowLongPtrW, ShowWindow,
        GWL_STYLE, SW_SHOW, WS_CHILD, WS_POPUP, WS_VISIBLE,
    };

    unsafe {
        let parent = HWND(parent_hwnd_raw as *mut _);
        let child = HWND(child_hwnd as *mut _);

        // Win32 child windows must have WS_CHILD set and WS_POPUP cleared,
        // otherwise SetParent's behavior is "undefined" (in practice: nothing
        // renders inside the preview pane).
        let mut style = GetWindowLongPtrW(child, GWL_STYLE);
        style &= !(WS_POPUP.0 as isize);
        style |= (WS_CHILD.0 | WS_VISIBLE.0) as isize;
        SetWindowLongPtrW(child, GWL_STYLE, style);

        let _ = SetParent(child, parent);

        let mut rect = RECT::default();
        if GetClientRect(parent, &mut rect).is_ok() {
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            let _ = MoveWindow(child, 0, 0, w, h, true);

            // Mirror the new size into winit's tracked inner size so wgpu
            // resizes its surface on the next frame.
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                w.max(1) as u32,
                h.max(1) as u32,
            ));
        }
        let _ = ShowWindow(child, SW_SHOW);
    }
}
