//! The phone app's setup screen in a window of its own (`wisp_client::phone::setup`): the
//! phone's launcher before the game. Taps on a phone, clicks on a PC (`--phone-private DIR`).
use super::*;
use winit::event::{MouseButton, TouchPhase};
use winit::keyboard::{Key, NamedKey};
use wisp_client::phone::setup::{Outcome, Setup};

struct Screen {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui: UiResources,
    setup: Setup,
    /// The finger or pointer in physical pixels; far away when nothing touches.
    pointer: (f64, f64),
    api: wisp_client::graphics::GraphicsApi,
    /// A network job ran at the last look: one more frame once it ends shows its result.
    animating: bool,
    play: bool,
    error: Option<Box<dyn std::error::Error>>,
}

const AWAY: (f64, f64) = (-1.0e6, -1.0e6);

/// Shows the setup until the player presses Play (true) or leaves (false).
pub(super) fn show(paths: &wisp_client::phone::Paths, settings: &Settings) -> Result<bool> {
    let mut screen = Screen {
        window: None,
        renderer: None,
        gui: UiResources {
            atlas: HudAtlas::build(&paths.resources)?,
            panorama: Arc::new(wisp_client::gfx::panorama::Faces::colour([0; 4])),
        },
        setup: Setup::new(paths.clone()),
        pointer: AWAY,
        api: settings.wisp.api,
        animating: false,
        play: false,
        error: None,
    };
    wisp_client::hud::GUI_SCALE.store(settings.gui_scale, Ordering::Relaxed);
    super::run_app(&mut screen)?;
    if let Some(error) = screen.error.take() {
        return Err(error);
    }
    // The game makes its own renderer on the same window.
    drop(screen.renderer.take());
    drop(screen.window.take());
    Ok(screen.play)
}

impl Screen {
    fn size(&self) -> [u32; 2] {
        self.renderer
            .as_ref()
            .map_or([1280, 720], |r| [r.config.width, r.config.height])
    }

    fn with_ui<R>(&mut self, f: impl FnOnce(&mut Setup, &mut wisp_client::ui_theme::Ui) -> R) -> R {
        let mut batch = HudBatch::new(self.size());
        let cursor = (
            self.pointer.0 as f32 / batch.scale,
            self.pointer.1 as f32 / batch.scale,
        );
        let mut ui = wisp_client::ui_theme::Ui::new(&self.gui.atlas, &mut batch, cursor);
        let result = f(&mut self.setup, &mut ui);
        ui.finish();
        result
    }

    fn draw(&mut self) {
        let mut batch = HudBatch::new(self.size());
        {
            let cursor = (
                self.pointer.0 as f32 / batch.scale,
                self.pointer.1 as f32 / batch.scale,
            );
            let mut ui = wisp_client::ui_theme::Ui::new(&self.gui.atlas, &mut batch, cursor);
            self.setup.draw(&mut ui);
            ui.finish();
        }
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        match renderer.render(&loading_frame(&batch)) {
            Ok(true) => return,
            Ok(false) => {}
            Err(e) => log::warn!("Wisp: frame failed: {e}"),
        }
        wisp_client::ui_theme::reset_atlas();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn tap(&mut self, event_loop: &ActiveEventLoop) {
        if self.with_ui(|setup, ui| setup.tap(ui)) == Outcome::Play {
            self.play = true;
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for Screen {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            if let Some(renderer) = &mut self.renderer
                && let Err(e) = renderer.resume_surface()
            {
                log::warn!("Wisp renderer: the window surface did not come back: {e}");
            }
            window.request_redraw();
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Wisp")
            .with_window_icon(wisp_client::window::icon())
            .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 560.0));
        let made = event_loop
            .create_window(attributes)
            .map_err(Into::into)
            .and_then(|window| {
                let window = Arc::new(window);
                let renderer = loading_renderer(
                    window.clone(),
                    false,
                    Some(&self.gui.atlas),
                    Some(&self.gui.panorama),
                    None,
                    self.api,
                )?;
                Ok((window, renderer))
            });
        match made {
            Ok((window, renderer)) => {
                window.request_redraw();
                self.window = Some(window);
                self.renderer = Some(renderer);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = &mut self.renderer {
            renderer.suspend_surface();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone().filter(|w| w.id() == id) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (position.x, position.y);
                window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.tap(event_loop),
            WindowEvent::Touch(touch) => {
                self.pointer = (touch.location.x, touch.location.y);
                match touch.phase {
                    TouchPhase::Ended => {
                        self.tap(event_loop);
                        self.pointer = AWAY;
                    }
                    TouchPhase::Cancelled => self.pointer = AWAY,
                    TouchPhase::Started | TouchPhase::Moved => {}
                }
                window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Android's back button (and Escape on a PC): a step back. On the phone's
                // home it does nothing: a stray back gesture mustn't close the app (Android's
                // home and app switcher still leave it).
                let back = matches!(event.logical_key, Key::Named(NamedKey::BrowserBack))
                    || event.physical_key == PhysicalKey::Code(KeyCode::Escape);
                if back && !self.setup.back() && !wisp_client::phone::on_phone() {
                    event_loop.exit();
                }
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if wisp_threads::stop_requests() > 0 {
            event_loop.exit();
            return;
        }
        // Progress from the network threads: redraw ten times a second while one runs.
        let animating = self.setup.animating();
        if (animating || self.animating)
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
        self.animating = animating;
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
        ));
    }
}
