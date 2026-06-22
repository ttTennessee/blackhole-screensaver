use crate::config::Config;
use glam::Vec2;

/// Driving state the renderer needs per frame.
#[derive(Debug, Clone, Copy)]
pub struct HoleState {
    /// Master 0..1 intensity — feeds time dilation and disk glow.
    pub intensity: f32,
    /// Shadow radius as a fraction of screen height (the GLSL's `rh = HOLE_RADIUS * sz`).
    pub shadow_radius: f32,
    /// uv-space center, with y running top-down (same convention as wgpu fragment coords).
    pub center: Vec2,
}

/// Slow grow-and-reset cycle + Lissajous drift.
pub struct Animator {
    pub cycle_seconds: f32,
    pub radius_min: f32,
    pub radius_max: f32,
    pub drift_amp: Vec2,
    pub drift_speed: Vec2,
    pub center_home: Vec2,
}

impl Animator {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            cycle_seconds: cfg.cycle_seconds.max(1.0),
            radius_min: cfg.radius_min.clamp(0.01, 0.5),
            radius_max: cfg.radius_max.clamp(0.05, 0.9),
            drift_amp: Vec2::new(cfg.drift_amp_x, cfg.drift_amp_y),
            drift_speed: Vec2::new(cfg.drift_speed_x, cfg.drift_speed_y),
            center_home: Vec2::new(0.5, 0.45),
        }
    }

    pub fn sample(&self, t: f32) -> HoleState {
        let phase = (t / self.cycle_seconds).fract();
        let eased = phase.powf(1.6);
        let radius = self.radius_min + (self.radius_max - self.radius_min) * eased;
        let intensity = 0.1 + 0.9 * eased;

        let cx = self.center_home.x
            + self.drift_amp.x * (std::f32::consts::TAU * self.drift_speed.x * t).sin();
        let cy = self.center_home.y
            + self.drift_amp.y * (std::f32::consts::TAU * self.drift_speed.y * t).cos();

        HoleState {
            intensity,
            shadow_radius: radius,
            center: Vec2::new(cx, cy),
        }
    }
}
