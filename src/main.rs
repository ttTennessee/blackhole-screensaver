#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod animator;
mod app;
mod config;
mod desktop;
mod mode;
mod renderer;

#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod daemon;
#[cfg(windows)]
mod fullscreen_check;
#[cfg(windows)]
mod preview;
#[cfg(windows)]
mod shared_mem;

use anyhow::Result;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mode = mode::parse_args(std::env::args());
    log::info!("starting in mode: {:?}", mode);

    match mode {
        mode::Mode::Config => {
            mode::show_config_dialog();
            return Ok(());
        }
        #[cfg(windows)]
        mode::Mode::Daemon => {
            daemon::run()?;
            return Ok(());
        }
        #[cfg(not(windows))]
        mode::Mode::Daemon => {
            log::error!("--daemon is Windows-only");
            return Ok(());
        }
        mode::Mode::Saver | mode::Mode::Preview(_) => {
            let cfg = config::Config::load();
            let event_loop = EventLoop::new()?;
            let mut app = app::App::new(mode, cfg);
            event_loop.run_app(&mut app)?;
        }
    }

    Ok(())
}
