use std::env::Args;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Saver,
    Preview(isize),
    Config,
    Daemon,
}

pub fn parse_args(mut args: Args) -> Mode {
    let _exe = args.next();
    let Some(flag) = args.next() else {
        return Mode::Config;
    };

    // --daemon (long form, no slash) takes priority before the /letter parse.
    if flag == "--daemon" {
        return Mode::Daemon;
    }

    let normalized = flag.trim_start_matches(['/', '-']).to_ascii_lowercase();
    let head: String = normalized.chars().take(1).collect();

    match head.as_str() {
        "s" => Mode::Saver,
        "p" => {
            let hwnd = args
                .next()
                .or_else(|| {
                    normalized
                        .strip_prefix("p:")
                        .map(|s| s.to_string())
                })
                .and_then(|s| s.parse::<isize>().ok())
                .unwrap_or(0);
            Mode::Preview(hwnd)
        }
        "c" | "a" => Mode::Config,
        _ => Mode::Config,
    }
}

#[cfg(windows)]
pub fn show_config_dialog() {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO,
    };

    // (1) Always make sure the daemon is running. If a daemon is already
    // alive, the spawned copy will detect that via the single-instance
    // mutex and exit silently; otherwise this is how the user starts
    // capturing without rebooting.
    let _ = spawn_daemon_detached();

    // (2) On the very first config visit, prompt about boot-time autostart.
    // We do NOT use this marker to gate the daemon spawn above -- the
    // daemon must be best-effort started on every Config invocation so
    // that a user who killed it can revive it just by double-clicking.
    let marker = dirs::config_dir()
        .map(|p| p.join("BlackholeScreensaver").join(".autostart_asked"));
    let asked_before = marker.as_ref().map(|p| p.exists()).unwrap_or(false);

    if !asked_before && !crate::autostart::is_enabled() {
        let prompt = "Blackhole Screensaver\n\nA background process has just \
            started; it grabs a fresh desktop screenshot every few seconds so \
            the screensaver can show your real desktop instead of a black \
            screen.\n\n\
            Should it start automatically with Windows from now on?\n\n\
            (You can toggle this anytime from the tray icon.)";
        let choice = unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(prompt),
                &HSTRING::from("Blackhole Screensaver — first run"),
                MB_YESNO | MB_ICONQUESTION,
            )
        };
        if choice == IDYES {
            let _ = crate::autostart::enable();
        }
        if let Some(m) = marker {
            if let Some(parent) = m.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&m, b"1");
        }
    }

    let path = dirs::config_dir()
        .map(|p| p.join("BlackholeScreensaver").join("config.toml"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "%APPDATA%\\BlackholeScreensaver\\config.toml".to_string());

    let autostart_state = if crate::autostart::is_enabled() {
        "ENABLED"
    } else {
        "disabled"
    };

    let body = format!(
        "Blackhole Screensaver\n\nAutostart: {state}\n\
        Background daemon: just (re)started — look for the tray icon.\n\n\
        To tune the effect, create or edit:\n{path}\n\n\
        Available keys (all optional):\n\
        cycle_seconds   = 300\n\
        radius_min      = 0.05\n\
        radius_max      = 0.55\n\
        drift_amp_x     = 0.30\n\
        drift_amp_y     = 0.20\n\
        drift_speed_x   = 0.013\n\
        drift_speed_y   = 0.017",
        state = autostart_state,
        path = path,
    );

    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(body),
            &HSTRING::from("Blackhole Screensaver"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Spawn `current_exe() --daemon` fully detached so it survives this
/// process exiting. CREATE_NO_WINDOW prevents a console flash; DETACHED_PROCESS
/// hands the child its own process group so closing this dialog does not
/// drag it down.
#[cfg(windows)]
fn spawn_daemon_detached() -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe()?;
    // 0x00000008 DETACHED_PROCESS | 0x08000000 CREATE_NO_WINDOW
    const FLAGS: u32 = 0x0000_0008 | 0x0800_0000;
    std::process::Command::new(exe)
        .arg("--daemon")
        .creation_flags(FLAGS)
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
pub fn show_config_dialog() {
    println!("Blackhole Screensaver - configuration not supported on this platform");
}
