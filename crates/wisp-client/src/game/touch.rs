//! Touch controls (the phone app; any touchscreen once a finger touches it).
//!
//! Playing: a joystick where the left thumb lands (pushed to its edge, it sprints), the right
//! side turns the view, a short tap there uses or places, holding still there breaks or attacks
//! (as phones play Minecraft), buttons for jump, sneak, inventory, drop, chat, perspective and
//! pause, and a tap on the hotbar picks its slot. Over a menu a finger is the mouse: a tap clicks,
//! holding still right-clicks, a drag scrolls.
//!
//! Everything goes through the paths the keyboard, mouse and controller take (`mapped_key`,
//! `play_mouse`, `look_turn`, the move vector), so key bindings, toggles and servers see the same.
use super::event_loop::EventLoopCtl;
use super::{Game, Screen};
use std::time::{Duration, Instant};
use winit::dpi::PhysicalPosition;
use winit::event::{
    DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::keyboard::KeyCode;
use wisp_client::held::Source;
use wisp_client::hud::HudBatch;
use wisp_client::ui_theme::{Ui, color};

/// Holding still this long breaks (playing) or right-clicks (a menu).
const HOLD: Duration = Duration::from_millis(300);
/// A finger that moved less than this (in units, see [`Layout::unit`]) still taps or holds.
const STILL: f64 = 0.12;
/// Degrees of turn per unit of finger travel.
const LOOK_DEGREES_PER_UNIT: f64 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Jump,
    Sneak,
    Inventory,
    Drop,
    Chat,
    Perspective,
    Pause,
}

const BUTTONS: [Button; 7] = [
    Button::Jump,
    Button::Sneak,
    Button::Inventory,
    Button::Drop,
    Button::Chat,
    Button::Perspective,
    Button::Pause,
];

#[derive(Clone, Copy, Debug)]
enum Role {
    /// The move joystick, centred where the finger landed.
    Stick {
        origin: (f64, f64),
    },
    /// Turning the view; breaking once held still.
    Look {
        breaking: bool,
        moved: bool,
    },
    Button(Button),
    Hotbar,
    /// A finger over a menu.
    Pointer {
        scrolled: bool,
        right: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    role: Role,
    start: (f64, f64),
    at: (f64, f64),
    since: Instant,
}

/// The touch controls' state, one per game.
#[derive(Default)]
pub(super) struct TouchControls {
    /// A finger touched this screen: the controls are drawn from now on.
    pub(super) active: bool,
    fingers: Vec<Finger>,
    /// The joystick's tilt, x right and y forward, -1..1.
    stick: [f32; 2],
    sprinting: bool,
    sneak_latched: bool,
    /// Scroll left over below one line.
    scroll: f64,
}

/// Where the controls are, in physical pixels, from the window's size and the GUI scale.
struct Layout {
    w: f64,
    h: f64,
    /// The size everything is measured in: a ninth of the short side.
    unit: f64,
    gui: f64,
}

impl Layout {
    fn new(w: f64, h: f64, gui: f64) -> Self {
        Self {
            w,
            h,
            unit: w.min(h) / 9.0,
            gui,
        }
    }

    /// A button's centre and radius.
    fn button(&self, button: Button) -> ((f64, f64), f64) {
        let u = self.unit;
        let (w, h) = (self.w, self.h);
        // The hotbar's ends (vanilla: 182 × 22 GUI pixels, centred at the bottom).
        let bar = 91.0 * self.gui;
        // Level with the hotbar, but never off the bottom edge.
        let bar_y = (h - 11.0 * self.gui).min(h - 0.5 * u);
        match button {
            Button::Jump => ((w - 1.35 * u, h - 1.7 * u), 0.8 * u),
            Button::Sneak => ((w - 3.2 * u, h - 1.1 * u), 0.6 * u),
            Button::Inventory => ((w / 2.0 + bar + 0.6 * u, bar_y), 0.45 * u),
            Button::Drop => ((w / 2.0 - bar - 0.6 * u, bar_y), 0.45 * u),
            Button::Pause => ((w - 0.7 * u, 0.7 * u), 0.42 * u),
            Button::Chat => ((w - 1.85 * u, 0.7 * u), 0.42 * u),
            Button::Perspective => ((w - 3.0 * u, 0.7 * u), 0.42 * u),
        }
    }

    fn button_at(&self, at: (f64, f64)) -> Option<Button> {
        BUTTONS.into_iter().find(|b| {
            let (c, r) = self.button(*b);
            // A finger is wider than the drawing: a third more to hit.
            distance(c, at) <= r * 1.35
        })
    }

    /// The hotbar slot under `at`, if it is on the hotbar.
    fn hotbar_slot(&self, at: (f64, f64)) -> Option<usize> {
        let left = self.w / 2.0 - 91.0 * self.gui;
        let top = self.h - 22.0 * self.gui;
        if at.1 < top || at.0 < left || at.0 >= left + 182.0 * self.gui {
            return None;
        }
        Some((((at.0 - left) / (20.0 * self.gui)) as usize).min(8))
    }

    /// The joystick's radius.
    fn stick_radius(&self) -> f64 {
        1.1 * self.unit
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The joystick's tilt for a finger at `at` (x right, y forward), and whether it is at the edge.
fn tilt(origin: (f64, f64), at: (f64, f64), radius: f64) -> ([f32; 2], bool) {
    let (dx, dy) = ((at.0 - origin.0) / radius, (origin.1 - at.1) / radius);
    let length = (dx * dx + dy * dy).sqrt();
    let (dx, dy) = if length > 1.0 {
        (dx / length, dy / length)
    } else {
        (dx, dy)
    };
    // A small dead zone: a resting thumb doesn't creep.
    if length < 0.15 {
        return ([0.0; 2], false);
    }
    ([dx as f32, dy as f32], length >= 0.95 && dy > 0.6)
}

impl Game<'_> {
    fn touch_layout(&self) -> Option<Layout> {
        let renderer = self.renderer.as_ref()?;
        let batch = HudBatch::new([renderer.config.width, renderer.config.height]);
        Some(Layout::new(
            f64::from(renderer.config.width),
            f64::from(renderer.config.height),
            f64::from(batch.scale),
        ))
    }

    /// The touch controls play, rather than a menu taking the finger.
    fn touch_playing(&self) -> bool {
        self.title.is_none()
            && self.screen == Screen::Playing
            && self.player.is_some()
            && !self.dialog_open()
    }

    /// The joystick's move vector while playing (like the controller's left stick).
    pub(super) fn touch_move(&self) -> Option<[f32; 2]> {
        (self.touch.active && self.touch.stick != [0.0; 2] && self.touch_playing())
            .then_some(self.touch.stick)
    }

    /// A finger on the window.
    pub(super) fn touch_event(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        touch: winit::event::Touch,
    ) -> super::Result<()> {
        self.touch.active = true;
        self.last_input = Instant::now();
        let at = (touch.location.x, touch.location.y);
        let Some(layout) = self.touch_layout() else {
            return Ok(());
        };
        match touch.phase {
            TouchPhase::Started => {
                let role = if !self.touch_playing() {
                    self.touch_pointer_to(event_loop, at);
                    Role::Pointer {
                        scrolled: false,
                        right: false,
                    }
                } else if let Some(button) = layout.button_at(at) {
                    self.touch_button(button, true)?;
                    Role::Button(button)
                } else if let Some(slot) = layout.hotbar_slot(at) {
                    self.selected = slot;
                    Role::Hotbar
                } else if at.0 < layout.w * 0.42 && at.1 > layout.h * 0.2 {
                    Role::Stick { origin: at }
                } else {
                    Role::Look {
                        breaking: false,
                        moved: false,
                    }
                };
                self.touch.fingers.retain(|f| f.id != touch.id);
                self.touch.fingers.push(Finger {
                    id: touch.id,
                    role,
                    start: at,
                    at,
                    since: Instant::now(),
                });
            }
            TouchPhase::Moved => {
                let Some(index) = self.touch.fingers.iter().position(|f| f.id == touch.id) else {
                    return Ok(());
                };
                let mut finger = self.touch.fingers[index];
                let previous = finger.at;
                finger.at = at;
                let far = distance(finger.start, at) > STILL * layout.unit;
                match &mut finger.role {
                    Role::Stick { origin } => {
                        let (stick, edge) = tilt(*origin, at, layout.stick_radius());
                        self.touch.stick = stick;
                        if edge != self.touch.sprinting {
                            self.touch.sprinting = edge;
                            self.mapped_key(
                                KeyCode::ControlLeft,
                                edge,
                                false,
                                KeyCode::ControlLeft,
                                Source::Pad,
                            )?;
                        }
                    }
                    Role::Look { moved, .. } => {
                        *moved |= far;
                        let degrees = LOOK_DEGREES_PER_UNIT / layout.unit
                            * (f64::from(self.settings.sensitivity) * 2.0).max(0.2);
                        let (dx, dy) = (at.0 - previous.0, at.1 - previous.1);
                        // `look_turn` takes Entity.turn's units (0.15 degrees each).
                        self.look_turn(dx * degrees / 0.15, dy * degrees / 0.15);
                    }
                    Role::Hotbar => {
                        if let Some(slot) = layout.hotbar_slot(at) {
                            self.selected = slot;
                        }
                    }
                    Role::Pointer { scrolled, .. } => {
                        let dy = at.1 - previous.1;
                        if *scrolled || far {
                            *scrolled = true;
                            self.touch.scroll += dy / (0.4 * layout.unit);
                            let lines = self.touch.scroll.trunc();
                            if lines != 0.0 {
                                self.touch.scroll -= lines;
                                self.touch_wheel(event_loop, lines as f32);
                            }
                        }
                    }
                    Role::Button(_) => {}
                }
                self.touch.fingers[index] = finger;
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some(index) = self.touch.fingers.iter().position(|f| f.id == touch.id) else {
                    return Ok(());
                };
                let finger = self.touch.fingers.remove(index);
                let cancelled = touch.phase == TouchPhase::Cancelled;
                match finger.role {
                    Role::Stick { .. } => {
                        self.touch.stick = [0.0; 2];
                        if self.touch.sprinting {
                            self.touch.sprinting = false;
                            self.mapped_key(
                                KeyCode::ControlLeft,
                                false,
                                false,
                                KeyCode::ControlLeft,
                                Source::Pad,
                            )?;
                        }
                    }
                    Role::Look { breaking, moved } => {
                        if breaking {
                            self.play_mouse(MouseButton::Left, false)?;
                        } else if !moved && !cancelled && finger.since.elapsed() < HOLD {
                            // A tap: use the item, place the block, talk to the villager.
                            self.play_mouse(MouseButton::Right, true)?;
                            self.play_mouse(MouseButton::Right, false)?;
                        }
                    }
                    Role::Button(button) => self.touch_button(button, false)?,
                    Role::Hotbar => {}
                    Role::Pointer { scrolled, right } => {
                        self.touch.scroll = 0.0;
                        if !scrolled && !cancelled {
                            self.touch_pointer_to(event_loop, at);
                            if right {
                                self.touch_pointer_button(event_loop, MouseButton::Right, false);
                            } else {
                                self.touch_pointer_button(event_loop, MouseButton::Left, true);
                                self.touch_pointer_button(event_loop, MouseButton::Left, false);
                            }
                        }
                        // No finger, no hover: the pointer leaves, so no button stays lit and
                        // no tooltip stays open where the last tap was.
                        if self.touch.fingers.is_empty() {
                            self.touch_pointer_to(event_loop, (-1.0e4, -1.0e4));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Once a frame: fingers held still turn into breaking (playing) or a right-click (a menu);
    /// a screen opening lets go of what the fingers held in the world.
    pub(super) fn touch_frame(&mut self, event_loop: &dyn EventLoopCtl) -> super::Result<()> {
        if !self.touch.active {
            return Ok(());
        }
        let Some(layout) = self.touch_layout() else {
            return Ok(());
        };
        let playing = self.touch_playing();
        if !playing {
            self.touch_release_world()?;
        }
        for i in 0..self.touch.fingers.len() {
            let finger = self.touch.fingers[i];
            let still = distance(finger.start, finger.at) <= STILL * layout.unit;
            let held = finger.since.elapsed() >= HOLD;
            match finger.role {
                Role::Look {
                    breaking: false,
                    moved: false,
                } if playing && still && held => {
                    self.play_mouse(MouseButton::Left, true)?;
                    self.touch.fingers[i].role = Role::Look {
                        breaking: true,
                        moved: false,
                    };
                }
                Role::Pointer {
                    scrolled: false,
                    right: false,
                } if still && held && !playing => {
                    self.touch_pointer_to(event_loop, finger.at);
                    self.touch_pointer_button(event_loop, MouseButton::Right, true);
                    self.touch.fingers[i].role = Role::Pointer {
                        scrolled: false,
                        right: true,
                    };
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// A screen opened over the world: the stick, a held break and held buttons let go, as the
    /// keyboard's keys do (`KeyMapping.releaseAll`).
    fn touch_release_world(&mut self) -> super::Result<()> {
        let world: Vec<Finger> = self
            .touch
            .fingers
            .iter()
            .copied()
            .filter(|f| !matches!(f.role, Role::Pointer { .. }))
            .collect();
        if world.is_empty() {
            return Ok(());
        }
        self.touch
            .fingers
            .retain(|f| matches!(f.role, Role::Pointer { .. }));
        for finger in world {
            match finger.role {
                Role::Look { breaking: true, .. } => self.play_mouse(MouseButton::Left, false)?,
                Role::Button(button @ (Button::Jump | Button::Drop)) => {
                    self.touch_button(button, false)?;
                }
                _ => {}
            }
        }
        self.touch.stick = [0.0; 2];
        if self.touch.sprinting {
            self.touch.sprinting = false;
            self.mapped_key(
                KeyCode::ControlLeft,
                false,
                false,
                KeyCode::ControlLeft,
                Source::Pad,
            )?;
        }
        Ok(())
    }

    fn touch_key(&mut self, key: KeyCode, down: bool) -> super::Result<()> {
        self.mapped_key(key, down, false, key, Source::Pad)
    }

    fn touch_button(&mut self, button: Button, down: bool) -> super::Result<()> {
        match button {
            Button::Jump => self.touch_key(KeyCode::Space, down),
            // Drop throws one item a press (key.drop), held keeps throwing like a held key.
            Button::Drop => self.touch_key(KeyCode::KeyQ, down),
            // Sneak latches: one tap down, one tap up (holding a button while moving is hard).
            Button::Sneak => {
                if !down {
                    return Ok(());
                }
                self.touch.sneak_latched = !self.touch.sneak_latched;
                self.touch_key(KeyCode::ShiftLeft, self.touch.sneak_latched)
            }
            Button::Inventory | Button::Chat | Button::Perspective | Button::Pause => {
                if !down {
                    return Ok(());
                }
                let key = match button {
                    Button::Inventory => KeyCode::KeyE,
                    Button::Chat => KeyCode::KeyT,
                    Button::Perspective => KeyCode::F5,
                    _ => KeyCode::Escape,
                };
                if key == KeyCode::Escape {
                    return self.key_pressed(KeyCode::Escape, KeyCode::Escape);
                }
                self.touch_key(key, true)?;
                self.touch_key(key, false)
            }
        }
    }

    fn touch_window_event(&mut self, event_loop: &dyn EventLoopCtl, event: WindowEvent) {
        if let Some(id) = self.window.as_ref().map(|w| w.id()) {
            self.handle_window_event(event_loop, id, event, super::input_trace::InputOrigin::User);
        }
    }

    fn touch_pointer_to(&mut self, event_loop: &dyn EventLoopCtl, at: (f64, f64)) {
        self.touch_window_event(
            event_loop,
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(at.0, at.1),
            },
        );
    }

    fn touch_pointer_button(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        button: MouseButton,
        down: bool,
    ) {
        let state = if down {
            ElementState::Pressed
        } else {
            ElementState::Released
        };
        self.touch_window_event(
            event_loop,
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state,
                button,
            },
        );
    }

    fn touch_wheel(&mut self, event_loop: &dyn EventLoopCtl, lines: f32) {
        self.touch_window_event(
            event_loop,
            WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, lines),
                phase: TouchPhase::Moved,
            },
        );
    }

    /// The controls over the HUD while playing.
    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        if !self.touch.active || !self.touch_playing() {
            return;
        }
        let Some(layout) = self.touch_layout() else {
            return;
        };
        // One dp to a physical pixel: the layout is in physical pixels.
        let mut ui = Ui::with_density(&self.gui.atlas, batch, (-1.0e6, -1.0e6), 1.0);
        let plate = color::with_alpha(color::hex(0x000000, 255), 0.30);
        let ink = color::with_alpha(color::hex(0xffffff, 255), 0.85);
        let lit = color::with_alpha(color::ACCENT, 0.55);
        let held: Vec<Button> = self
            .touch
            .fingers
            .iter()
            .filter_map(|f| match f.role {
                Role::Button(b) => Some(b),
                _ => None,
            })
            .collect();
        for button in BUTTONS {
            let ((x, y), r) = layout.button(button);
            let (x, y, r) = (x as f32, y as f32, r as f32);
            let on =
                held.contains(&button) || (button == Button::Sneak && self.touch.sneak_latched);
            ui.circle(x, y, r, if on { lit } else { plate });
            draw_glyph(&mut ui, button, x, y, r, ink);
        }
        // The joystick: its ring where the thumb landed, the knob under the thumb.
        for finger in &self.touch.fingers {
            if let Role::Stick { origin } = finger.role {
                let r = layout.stick_radius() as f32;
                let (ox, oy) = (origin.0 as f32, origin.1 as f32);
                ui.circle(ox, oy, r, plate);
                let knob = (ox + self.touch.stick[0] * r, oy - self.touch.stick[1] * r);
                let knob_color = if self.touch.sprinting { lit } else { ink };
                ui.circle(knob.0, knob.1, r * 0.42, color::with_alpha(knob_color, 0.6));
            }
        }
        ui.finish();
    }
}

/// A button's symbol, drawn from shapes (no font has them all).
fn draw_glyph(ui: &mut Ui, button: Button, x: f32, y: f32, r: f32, ink: u32) {
    let s = r * 0.45;
    let line = (r * 0.12).max(2.0);
    match button {
        Button::Jump => ui.polyline(
            &[[x - s, y + s * 0.4], [x, y - s * 0.6], [x + s, y + s * 0.4]],
            line,
            ink,
        ),
        Button::Sneak => ui.polyline(
            &[[x - s, y - s * 0.4], [x, y + s * 0.6], [x + s, y - s * 0.4]],
            line,
            ink,
        ),
        Button::Inventory => {
            for dx in [-s * 0.8, 0.0, s * 0.8] {
                ui.circle(x + dx, y, line * 0.9, ink);
            }
        }
        Button::Drop => {
            ui.polyline(&[[x, y - s], [x, y + s * 0.5]], line, ink);
            ui.polyline(
                &[[x - s * 0.6, y], [x, y + s * 0.6], [x + s * 0.6, y]],
                line,
                ink,
            );
        }
        Button::Chat => {
            ui.rounded(x - s, y - s * 0.7, 2.0 * s, 1.3 * s, s * 0.35, ink);
            ui.quad(
                [
                    [x - s * 0.5, y + s * 0.5],
                    [x - s * 0.1, y + s * 0.5],
                    [x - s * 0.6, y + s],
                    [x - s * 0.6, y + s],
                ],
                ink,
            );
        }
        Button::Perspective => {
            ui.circle(x, y, s * 0.9, ink);
            ui.circle(
                x,
                y,
                s * 0.55,
                color::with_alpha(color::hex(0x000000, 255), 0.6),
            );
            ui.circle(x, y, s * 0.3, ink);
        }
        Button::Pause => {
            ui.rounded(x - s * 0.6, y - s * 0.75, s * 0.4, s * 1.5, s * 0.1, ink);
            ui.rounded(x + s * 0.2, y - s * 0.75, s * 0.4, s * 1.5, s * 0.1, ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone() -> Layout {
        // A 2400 × 1080 phone at GUI scale 4.
        Layout::new(2400.0, 1080.0, 4.0)
    }

    #[test]
    fn buttons_do_not_overlap_and_stay_on_screen() {
        let l = phone();
        for (i, a) in BUTTONS.iter().enumerate() {
            let ((x, y), r) = l.button(*a);
            assert!(
                x - r >= 0.0 && x + r <= l.w && y - r >= 0.0 && y + r <= l.h,
                "{a:?}"
            );
            for b in &BUTTONS[i + 1..] {
                let (c, rb) = l.button(*b);
                assert!(distance((x, y), c) > r + rb, "{a:?} and {b:?} overlap");
            }
            assert_eq!(l.button_at((x, y)), Some(*a));
        }
    }

    #[test]
    fn the_hotbar_slots() {
        let l = phone();
        let left = 1200.0 - 91.0 * 4.0;
        assert_eq!(l.hotbar_slot((left + 2.0, 1075.0)), Some(0));
        assert_eq!(l.hotbar_slot((left + 182.0 * 4.0 - 2.0, 1075.0)), Some(8));
        assert_eq!(l.hotbar_slot((1200.0, 900.0)), None);
        // The inventory and drop buttons sit beside the hotbar, not on it.
        for b in [Button::Inventory, Button::Drop] {
            assert_eq!(l.hotbar_slot(l.button(b).0), None);
        }
    }

    #[test]
    fn the_stick_tilts_and_sprints_at_its_edge() {
        let r = 100.0;
        assert_eq!(tilt((0.0, 0.0), (5.0, -5.0), r), ([0.0, 0.0], false));
        let (forward, edge) = tilt((0.0, 0.0), (0.0, -50.0), r);
        assert_eq!((forward, edge), ([0.0, 0.5], false));
        let (full, edge) = tilt((0.0, 0.0), (0.0, -300.0), r);
        assert_eq!((full, edge), ([0.0, 1.0], true));
        // Full to the side moves sideways without sprinting.
        assert!(!tilt((0.0, 0.0), (300.0, 0.0), r).1);
    }
}
