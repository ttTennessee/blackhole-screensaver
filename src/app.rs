use crate::animator::Animator;
use crate::config::Config;
use crate::mode::Mode;
use crate::renderer::Renderer;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct App {
    mode: Mode,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    animator: Animator,
    start: Instant,
    last_cursor: Option<(f64, f64)>,
    input_armed_at: Option<Instant>,
}

impl App {
    pub fn new(mode: Mode, cfg: Config) -> Self {
        Self {
            mode,
            window: None,
            renderer: None,
            animator: Animator::from_config(&cfg),
            start: Instant::now(),
            last_cursor: None,
            input_armed_at: None,
        }
    }

    fn exit(&self, event_loop: &ActiveEventLoop) {
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title("Blackhole Screensaver")
            .with_decorations(false)
            .with_resizable(false);

        match self.mode {
            Mode::Saver => {
                // Span the entire virtual desktop (all monitors) instead of
                // relying on Fullscreen::Borderless, which on multi-monitor
                // setups picks a single display and on some drivers fails
                // outright. A topmost borderless window sized to the virtual
                // desktop rect gives us one continuous canvas across all
                // physical screens.
                #[cfg(windows)]
                {
                    if let Some(vd) = crate::desktop_layout::virtual_desktop() {
                        log::info!(
                            "virtual desktop: origin=({}, {}) size={}x{}",
                            vd.x, vd.y, vd.width, vd.height
                        );
                        attrs = attrs
                            .with_position(winit::dpi::PhysicalPosition::new(vd.x, vd.y))
                            .with_inner_size(winit::dpi::PhysicalSize::new(vd.width, vd.height))
                            .with_visible(true);
                    } else {
                        log::warn!("virtual_desktop() returned None, falling back to borderless fullscreen");
                        attrs = attrs
                            .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                            .with_visible(true);
                    }
                }
                #[cfg(not(windows))]
                {
                    attrs = attrs
                        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                        .with_visible(true);
                }
            }
            Mode::Preview(hwnd) => {
                // Settings panel preview: we will be reparented under the
                // panel's tiny preview HWND right after creation. Start the
                // window small + invisible so it doesn't flash on the desktop.
                attrs = attrs
                    .with_visible(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(200u32, 150u32));
                let _ = hwnd;
            }
            Mode::Config | Mode::Daemon => unreachable!(),
        }

        // Prefer the daemon's recent snapshot from shared memory. If the
        // daemon isn't running we still try a live BitBlt, which works when
        // the user launched us manually (the screensaver subsystem itself
        // blacks the screen out before invoking /s, so the BitBlt is mostly
        // a fallback for manual testing).
        let shot = if matches!(self.mode, Mode::Saver) {
            #[cfg(windows)]
            {
                crate::desktop::read_from_shared_mem()
                    .or_else(|| crate::desktop::capture())
            }
            #[cfg(not(windows))]
            {
                crate::desktop::capture()
            }
        } else {
            None
        };

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("create_window failed: {e:?}");
                event_loop.exit();
                return;
            }
        };

        if matches!(self.mode, Mode::Saver) {
            window.set_cursor_visible(false);
            window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
            window.focus_window();
        }

        // Preview: reparent under the system-supplied HWND and size to fit it.
        #[cfg(windows)]
        if let Mode::Preview(parent_hwnd) = self.mode {
            crate::preview::attach_to_parent(&window, parent_hwnd);
        }

        let renderer = match pollster::block_on(Renderer::new(window.clone(), shot)) {
            Ok(r) => r,
            Err(e) => {
                log::error!("renderer init failed: {e:?}");
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.start = Instant::now();
        // Arm input-to-exit detection after a brief delay to ignore
        // spurious cursor events generated by entering fullscreen.
        self.input_armed_at = Some(Instant::now() + std::time::Duration::from_millis(800));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let armed = self
            .input_armed_at
            .map(|t| Instant::now() >= t)
            .unwrap_or(false);

        match event {
            WindowEvent::CloseRequested => self.exit(event_loop),

            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size);
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if armed && matches!(self.mode, Mode::Saver) && event.state == ElementState::Pressed {
                    self.exit(event_loop);
                }
            }

            WindowEvent::MouseInput { .. } => {
                if armed && matches!(self.mode, Mode::Saver) {
                    self.exit(event_loop);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if !armed || !matches!(self.mode, Mode::Saver) {
                    self.last_cursor = Some((position.x, position.y));
                    return;
                }
                if let Some((px, py)) = self.last_cursor {
                    let dx = (position.x - px).abs();
                    let dy = (position.y - py).abs();
                    if dx + dy > 4.0 {
                        self.exit(event_loop);
                        return;
                    }
                }
                self.last_cursor = Some((position.x, position.y));
            }

            WindowEvent::RedrawRequested => {
                let t = self.start.elapsed().as_secs_f32();
                let state = self.animator.sample(t);
                if let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) {
                    r.render(t, &state);
                    w.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let armed = self
            .input_armed_at
            .map(|t| Instant::now() >= t)
            .unwrap_or(false);
        if !armed || !matches!(self.mode, Mode::Saver) {
            return;
        }

        if let DeviceEvent::MouseMotion { delta } = event {
            if delta.0.abs() + delta.1.abs() > 4.0 {
                self.exit(event_loop);
            }
        }
    }
}
