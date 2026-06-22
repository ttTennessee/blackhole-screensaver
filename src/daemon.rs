// Background daemon: every CAPTURE_INTERVAL it grabs the desktop into a
// named shared memory region for /s screensaver instances to read. The
// tray icon lets the user pause, exit, or toggle autostart.
//
// Capture is skipped (without dropping the previous snapshot) when:
//   * the user has paused us
//   * a fullscreen exclusive app is in the foreground (game / video)
//
// Single-instance enforced by a named mutex; a second --daemon exits.

#![cfg(windows)]

use crate::autostart;
use crate::desktop;
use crate::fullscreen_check;
use crate::shared_mem::{self, SharedMem};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, CheckMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

const CAPTURE_INTERVAL: Duration = Duration::from_secs(5);
const MUTEX_NAME: &str = "Local\\BlackholeScreensaver_Daemon_SingleInstance";

pub fn run() -> Result<()> {
    // Wrap so any early error gets a MessageBox -- the daemon runs without
    // a console, so otherwise it would just silently die and the user
    // would never know why no tray icon appeared.
    if let Err(e) = run_inner() {
        show_error(&format!("Blackhole daemon failed to start:\n\n{e:#}"));
        return Err(e);
    }
    Ok(())
}

fn show_error(msg: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(msg),
            &HSTRING::from("Blackhole Screensaver"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn run_inner() -> Result<()> {
    if !acquire_single_instance() {
        log::info!("another daemon is already running, exiting");
        return Ok(());
    }

    let mut shm = SharedMem::create()?;
    let paused = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));

    // ---- tray icon + menu ----
    let menu = Menu::new();
    let pause_item = CheckMenuItem::new("Pause capture", true, false, None);
    let autostart_item = CheckMenuItem::new(
        "Start with Windows",
        true,
        autostart::is_enabled(),
        None,
    );
    let quit_item = MenuItem::new("Quit", true, None);
    let _ = menu.append(&pause_item);
    let _ = menu.append(&autostart_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit_item);

    let _tray = TrayIconBuilder::new()
        .with_tooltip("Blackhole Screensaver — capturing desktop")
        .with_icon(default_icon())
        .with_menu(Box::new(menu))
        .build()?;

    let pause_id = pause_item.id().clone();
    let autostart_id = autostart_item.id().clone();
    let quit_id = quit_item.id().clone();

    // ---- main loop: pump tray messages + capture on a timer ----
    let menu_rx = MenuEvent::receiver();
    let mut last_capture = Instant::now() - CAPTURE_INTERVAL;

    while !quit.load(Ordering::Relaxed) {
        // Drain tray menu events (non-blocking).
        while let Ok(ev) = menu_rx.try_recv() {
            if ev.id == pause_id {
                let now_paused = !paused.load(Ordering::Relaxed);
                paused.store(now_paused, Ordering::Relaxed);
                pause_item.set_checked(now_paused);
            } else if ev.id == autostart_id {
                if autostart::is_enabled() {
                    let _ = autostart::disable();
                    autostart_item.set_checked(false);
                } else {
                    let _ = autostart::enable();
                    autostart_item.set_checked(true);
                }
            } else if ev.id == quit_id {
                quit.store(true, Ordering::Relaxed);
            }
        }

        // Capture if it's time AND we're allowed to.
        if last_capture.elapsed() >= CAPTURE_INTERVAL {
            last_capture = Instant::now();
            if !paused.load(Ordering::Relaxed) && !fullscreen_check::foreground_is_fullscreen() {
                if let Err(e) = capture_into(&mut shm) {
                    log::warn!("desktop capture failed: {e:?}");
                }
            }
        }

        // Pump Windows messages so the tray icon stays responsive.
        pump_messages(Duration::from_millis(100));
    }

    Ok(())
}

fn capture_into(shm: &mut SharedMem) -> Result<()> {
    let shot = desktop::capture().ok_or_else(|| anyhow::anyhow!("BitBlt returned nothing"))?;
    let needed = shared_mem::HEADER_SIZE + shot.bgra.len();
    let buf = shm.as_mut_slice();
    if buf.len() < needed {
        return Err(anyhow::anyhow!(
            "shared mapping too small ({} < {})",
            buf.len(),
            needed
        ));
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    shared_mem::write_header(&mut buf[..shared_mem::HEADER_SIZE], shot.width, shot.height, ts);
    buf[shared_mem::HEADER_SIZE..shared_mem::HEADER_SIZE + shot.bgra.len()]
        .copy_from_slice(&shot.bgra);
    Ok(())
}

fn acquire_single_instance() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        let h = CreateMutexW(None, false, &HSTRING::from(MUTEX_NAME));
        let already = GetLastError() == ERROR_ALREADY_EXISTS;
        if already {
            // We're not the owner -- drop the handle; the original holder
            // keeps theirs. Then signal failure to the caller.
            if let Ok(handle) = h {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
            false
        } else {
            // We own it. Intentionally LEAK the handle (Box::leak idiom on
            // a wrapper) so the kernel keeps the mutex alive for the
            // process lifetime. CloseHandle here would release the lock.
            let handle = h.expect("first CreateMutexW must succeed");
            Box::leak(Box::new(handle));
            true
        }
    }
}

fn pump_messages(timeout: Duration) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MsgWaitForMultipleObjects, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
        QS_ALLINPUT,
    };
    unsafe {
        let _ = MsgWaitForMultipleObjects(
            None,
            false,
            timeout.as_millis() as u32,
            QS_ALLINPUT,
        );
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn default_icon() -> Icon {
    // 16x16 solid white circle on transparent, drawn by hand so we don't
    // need a .ico file in the source tree. RGBA.
    let mut rgba = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let r = (dx * dx + dy * dy).sqrt();
            let i = (y * 16 + x) * 4;
            if r < 6.5 {
                // black hole shadow + thin warm ring
                let ring = (r - 5.6).abs() < 1.0;
                if ring {
                    rgba[i] = 255;
                    rgba[i + 1] = 160;
                    rgba[i + 2] = 60;
                    rgba[i + 3] = 255;
                } else if r < 5.0 {
                    rgba[i + 3] = 255; // opaque black
                } else {
                    rgba[i] = 255;
                    rgba[i + 1] = 120;
                    rgba[i + 2] = 40;
                    rgba[i + 3] = 200;
                }
            }
        }
    }
    Icon::from_rgba(rgba, 16, 16).expect("icon")
}
