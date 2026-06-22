// Heuristic: is the foreground window a fullscreen exclusive app?
// Used by the daemon to skip captures while a game / video is fullscreen.
//
// "Fullscreen" here means: foreground window covers an entire monitor's
// bounds AND it is not the shell (Progman / WorkerW / taskbar). This
// catches games, fullscreen videos, and slideshow presentations alike.

#![cfg(windows)]

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowRect,
};

pub fn foreground_is_fullscreen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }

        // Skip the shell so the bare desktop doesn't count.
        let mut class_buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class_buf);
        if n > 0 {
            let class = String::from_utf16_lossy(&class_buf[..n as usize]);
            if matches!(
                class.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return false;
            }
        }

        let mut wr = RECT::default();
        if GetWindowRect(hwnd, &mut wr).is_err() {
            return false;
        }
        let mon: HMONITOR = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return false;
        }
        let mr = mi.rcMonitor;
        // Window covers the monitor in both dimensions (allow 1px slack).
        wr.left <= mr.left + 1
            && wr.top <= mr.top + 1
            && wr.right >= mr.right - 1
            && wr.bottom >= mr.bottom - 1
    }
}
