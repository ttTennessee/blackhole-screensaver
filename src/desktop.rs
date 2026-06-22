// One-shot desktop snapshot: at screensaver start we BitBlt the virtual
// desktop into a DIB section and hand the raw BGRA pixels to wgpu as an
// sRGB texture. After that the GPU owns it; no per-frame capture.

#[cfg(windows)]
pub struct DesktopShot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>, // tightly packed, row-major
}

#[cfg(windows)]
pub fn capture() -> Option<DesktopShot> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HGDIOBJ, SRCCOPY,
    };
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    };

    unsafe {
        // Best-effort: make sure GetSystemMetrics returns physical pixels.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w <= 0 || h <= 0 {
            return None;
        }
        let (w, h) = (w as i32, h as i32);

        let screen_dc = GetDC(HWND(std::ptr::null_mut()));
        if screen_dc.is_invalid() {
            return None;
        }
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_invalid() {
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            return None;
        }
        let bmp = CreateCompatibleBitmap(screen_dc, w, h);
        if bmp.is_invalid() {
            let _ = DeleteDC(mem_dc);
            ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
            return None;
        }
        let old = SelectObject(mem_dc, HGDIOBJ(bmp.0));

        let ok = BitBlt(mem_dc, 0, 0, w, h, screen_dc, 0, 0, SRCCOPY).is_ok();

        let mut shot: Option<DesktopShot> = None;
        if ok {
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let stride = (w as usize) * 4;
            let mut bgra = vec![0u8; stride * (h as usize)];
            let copied = GetDIBits(
                mem_dc,
                bmp,
                0,
                h as u32,
                Some(bgra.as_mut_ptr() as *mut _),
                &mut info,
                DIB_RGB_COLORS,
            );
            if copied != 0 {
                shot = Some(DesktopShot {
                    width: w as u32,
                    height: h as u32,
                    bgra,
                });
            }
        }

        SelectObject(mem_dc, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(HWND(std::ptr::null_mut()), screen_dc);
        shot
    }
}

#[cfg(windows)]
pub fn read_from_shared_mem() -> Option<DesktopShot> {
    use crate::shared_mem::{read_snapshot, SharedMem};
    let shm = SharedMem::open_existing().ok()?;
    let snap = read_snapshot(shm.as_slice())?;
    Some(DesktopShot {
        width: snap.width,
        height: snap.height,
        bgra: snap.bgra,
    })
}

#[cfg(not(windows))]
pub struct DesktopShot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[cfg(not(windows))]
pub fn capture() -> Option<DesktopShot> {
    None
}

#[cfg(not(windows))]
pub fn read_from_shared_mem() -> Option<DesktopShot> {
    None
}
