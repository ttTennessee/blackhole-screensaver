// Optional user-facing tunables. Lives at:
//   %APPDATA%\BlackholeScreensaver\config.toml      (Windows)
//   ~/.config/blackhole-screensaver/config.toml     (other)
// Missing file -> sensible defaults. Bad file -> log warning, use defaults
// (a screensaver should never refuse to start because of a typo).

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Seconds for the hole to grow from minimum to maximum, then reset.
    pub cycle_seconds: f32,
    /// Smallest shadow radius, fraction of screen height.
    pub radius_min: f32,
    /// Largest shadow radius, fraction of screen height.
    pub radius_max: f32,
    /// Lissajous drift amplitude (uv units, x and y).
    pub drift_amp_x: f32,
    pub drift_amp_y: f32,
    /// Lissajous drift frequencies (Hz).
    pub drift_speed_x: f32,
    pub drift_speed_y: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cycle_seconds: 300.0,
            radius_min: 0.05,
            radius_max: 0.55,
            drift_amp_x: 0.30,
            drift_amp_y: 0.20,
            drift_speed_x: 0.013,
            drift_speed_y: 0.017,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        match Self::path().and_then(|p| std::fs::read_to_string(&p).ok()) {
            Some(text) => match toml::from_str::<Config>(&text) {
                Ok(c) => {
                    log::info!("loaded config from disk");
                    c
                }
                Err(e) => {
                    log::warn!("config parse error, using defaults: {e}");
                    Self::default()
                }
            },
            None => Self::default(),
        }
    }

    fn path() -> Option<PathBuf> {
        let base = dirs::config_dir()?;
        Some(base.join("BlackholeScreensaver").join("config.toml"))
    }
}
