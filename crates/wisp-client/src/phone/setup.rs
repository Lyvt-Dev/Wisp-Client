//! The phone's setup screen: sign in with Microsoft, get the game files from the PC, play.
//!
//! One layout ([`Setup::layout`]) both draws and answers taps, so they never disagree. Network
//! work runs on threads that report into [`Shared`]; the screen redraws while any is running.
use super::msa::{self, Account, DeviceCode};
use super::text::{t, tf};
use super::{Paths, open_url};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wisp_client::ui_theme::{Face, Ui, color};
use wisp_phone_link::{Found, Progress};

/// Where the setup is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Home,
    SignIn,
    FindPc,
    Address,
    Code,
    Download,
}

/// What a tap does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SignIn,
    SignOut,
    OpenLink,
    GetFiles,
    Search,
    Pick(usize),
    TypeAddress,
    Key(char),
    Erase,
    Next,
    Back,
    Play,
}

/// What the screen wants after a tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Play,
}

/// What the threads report.
#[derive(Default)]
struct Shared {
    account: Option<Account>,
    device: Option<DeviceCode>,
    /// What a running job is doing, for the player.
    status: Option<String>,
    error: Option<String>,
    found: Vec<Found>,
    searching: bool,
    progress: Option<Progress>,
    /// A sign-in or copy is running.
    busy: bool,
    /// The step to go to once a job finishes (a finished copy goes home).
    go: Option<Step>,
}

pub struct Setup {
    step: Step,
    paths: Paths,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    /// The digits (or address) typed so far.
    typed: String,
    /// The PC picked or typed.
    pc: Option<Found>,
}

const COLUMN: f32 = 620.0;
const TITLE: f32 = wisp_theme::TITLE.px;
const BODY: f32 = wisp_theme::BODY.px;
const LINE: f32 = 24.0;
const BUTTON: f32 = wisp_theme::UI.px + 1.0;
/// Buttons are taller than the desktop's: a finger, not a pointer.
const CONTROL: f32 = 52.0;
const KEY: f32 = 64.0;
const GAP: f32 = 12.0;
/// The home screen's Play button: the one thing to press.
const HERO: f32 = 76.0;
/// Room along the screen's edges (rounded corners, a camera cutout).
const MARGIN: f32 = 56.0;
const CARD_PAD: f32 = 24.0;
const LIVE: u32 = color::hex(wisp_theme::glass::LIVE, 255);
const WARN: u32 = color::hex(wisp_theme::glass::WARN, 255);
const DANGER: u32 = color::hex(wisp_theme::glass::DANGER, 255);

/// What a home card shows at its top right.
enum Badge {
    Avatar(Option<String>),
    Dot(u32),
}

/// The small Wisp mark at the top left of every step but home.
fn header() -> [Item; 2] {
    [
        Item::Feather {
            x: MARGIN + 6.0,
            y: 40.0,
            size: 28.0,
        },
        Item::Text {
            face: Face::Heading,
            size: 22.0,
            x: MARGIN + 26.0,
            y: 28.0,
            color: color::TEXT,
            text: wisp_theme::brand::NAME.into(),
        },
    ]
}

/// The page behind every step: dark, with a soft light behind the left half.
fn backdrop(ui: &mut Ui) {
    ui.rounded(0.0, 0.0, ui.w, ui.h, 0.0, color::BG);
    let (gx, gy) = (ui.w * 0.26, ui.h * 0.5);
    for i in 0..10 {
        let r = ui.h * (1.05 - i as f32 * 0.085);
        ui.circle(gx, gy, r, color::hex(0xffffff, 4));
    }
    // A second, fainter light at the bottom right.
    for i in 0..6 {
        let r = ui.h * (0.6 - i as f32 * 0.09);
        ui.circle(ui.w * 0.92, ui.h * 1.02, r, color::hex(0xffffff, 3));
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, (cx, cy): (f32, f32)) -> bool {
        cx >= self.x && cx < self.x + self.w && cy >= self.y && cy < self.y + self.h
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Home's Play: large, white.
    Hero,
    Primary,
    Secondary,
    Key,
}

enum Item {
    Text {
        face: Face,
        size: f32,
        x: f32,
        y: f32,
        color: u32,
        text: String,
    },
    Plate {
        rect: Rect,
        radius: f32,
        color: u32,
    },
    Button {
        rect: Rect,
        label: String,
        action: Action,
        kind: Kind,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
        color: u32,
    },
    /// Wisp's feather (`Ui::feather`), centred at `x`, `y`.
    Feather {
        x: f32,
        y: f32,
        size: f32,
    },
}

/// Builds the items top to bottom in a column.
struct Column<'u, 'a> {
    ui: &'u Ui<'a>,
    x: f32,
    y: f32,
    w: f32,
    items: Vec<Item>,
}

impl Column<'_, '_> {
    fn text(&mut self, face: Face, size: f32, line: f32, color: u32, text: &str) {
        for part in wrap(self.ui, face, size, text, self.w) {
            self.items.push(Item::Text {
                face,
                size,
                x: self.x,
                y: self.y,
                color,
                text: part,
            });
            self.y += line;
        }
    }
    fn gap(&mut self, h: f32) {
        self.y += h;
    }
    /// A row of buttons, left to right.
    fn buttons(&mut self, row: &[(String, Action, Kind)]) {
        let mut x = self.x;
        for (label, action, kind) in row {
            let w = (self.ui.measure(Face::Medium, BUTTON, label) + 48.0)
                .round()
                .max(120.0);
            self.items.push(Item::Button {
                rect: Rect {
                    x,
                    y: self.y,
                    w,
                    h: CONTROL,
                },
                label: label.clone(),
                action: *action,
                kind: *kind,
            });
            x += w + GAP;
        }
        self.y += CONTROL;
    }
}

/// `text` broken at spaces into lines no wider than `width` dp.
fn wrap(ui: &Ui, face: Face, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && ui.measure(face, size, &candidate) > width {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
}

impl Setup {
    pub fn new(paths: Paths) -> Self {
        let shared = Shared {
            account: Account::load(&paths.private),
            ..Shared::default()
        };
        Self {
            step: Step::Home,
            paths,
            shared: Arc::new(Mutex::new(shared)),
            cancel: Arc::new(AtomicBool::new(false)),
            typed: String::new(),
            pc: None,
        }
    }

    /// The account signed in now, if any.
    pub fn account(&self) -> Option<Account> {
        self.shared.lock().unwrap().account.clone()
    }

    /// The game files are on the phone.
    pub fn files_ready(&self) -> bool {
        self.paths.resources.join("wisp-resources.json").is_file()
    }

    /// A job is running: the screen redraws for its progress.
    pub fn animating(&self) -> bool {
        let shared = self.shared.lock().unwrap();
        shared.busy || shared.searching
    }

    /// Follows a finished job to its next step.
    fn settle(&mut self) {
        if let Some(step) = self.shared.lock().unwrap().go.take() {
            self.step = step;
        }
    }

    fn layout(&mut self, ui: &Ui) -> Vec<Item> {
        self.settle();
        let shared = self.shared.lock().unwrap();
        let keypad = matches!(self.step, Step::Address | Step::Code);
        let pad_w = 3.0 * KEY + 2.0 * GAP;
        // With the keypad, the text column and the keypad side by side.
        let total = if keypad {
            COLUMN.min(ui.w - pad_w - 96.0) + 48.0 + pad_w
        } else {
            COLUMN.min(ui.w - 48.0)
        };
        let x = ((ui.w - total) / 2.0).max(24.0).round();
        let w = if keypad { total - 48.0 - pad_w } else { total };
        let mut c = Column {
            ui,
            x,
            y: 0.0,
            w,
            items: Vec::new(),
        };
        let title = |c: &mut Column, key: &str| {
            c.text(Face::Heading, TITLE, 44.0, color::TEXT, t(key));
            c.gap(10.0);
        };
        match self.step {
            Step::Home => {
                let account = shared.account.clone();
                let error = shared.error.clone();
                drop(shared);
                return self.home(ui, account.as_ref(), error.as_deref());
            }
            Step::SignIn => {
                title(&mut c, "sign_in_title");
                match &shared.device {
                    Some(device) => {
                        c.text(Face::Regular, BODY, LINE, color::MUTED, t("sign_in_steps"));
                        c.gap(16.0);
                        c.text(Face::Mono, 40.0, 56.0, color::TEXT, &device.user_code);
                        c.gap(16.0);
                        c.buttons(&[
                            (t("open_browser").into(), Action::OpenLink, Kind::Primary),
                            (t("cancel").into(), Action::Back, Kind::Secondary),
                        ]);
                    }
                    None => {
                        c.buttons(&[(t("cancel").into(), Action::Back, Kind::Secondary)]);
                    }
                }
            }
            Step::FindPc => {
                title(&mut c, "pc_title");
                c.text(Face::Regular, BODY, LINE, color::MUTED, t("pc_steps"));
                c.gap(20.0);
                if shared.searching && shared.found.is_empty() {
                    c.text(Face::Regular, BODY, LINE, color::TEXT, t("searching"));
                } else if shared.found.is_empty() {
                    c.text(Face::Regular, BODY, LINE, color::TEXT, t("none_found"));
                }
                for (i, pc) in shared.found.iter().enumerate() {
                    let label = format!("{}  ·  {}", pc.name, pc.addr.ip());
                    c.buttons(&[(label, Action::Pick(i), Kind::Primary)]);
                    c.gap(GAP);
                }
                c.gap(16.0);
                c.buttons(&[
                    (t("search_again").into(), Action::Search, Kind::Secondary),
                    (
                        t("type_address").into(),
                        Action::TypeAddress,
                        Kind::Secondary,
                    ),
                    (t("back").into(), Action::Back, Kind::Secondary),
                ]);
            }
            Step::Address | Step::Code => {
                let (head, steps) = if self.step == Step::Address {
                    ("address_title", t("address_steps").to_owned())
                } else {
                    let name = self
                        .pc
                        .as_ref()
                        .map(|p| p.name.as_str())
                        .filter(|n| !n.is_empty())
                        .unwrap_or(t("your_pc"));
                    ("code_title", tf("code_steps", &[("name", name)]))
                };
                title(&mut c, head);
                c.text(Face::Regular, BODY, LINE, color::MUTED, &steps);
                c.gap(20.0);
                let field = Rect {
                    x: c.x,
                    y: c.y,
                    w: c.w.min(360.0),
                    h: 64.0,
                };
                c.items.push(Item::Plate {
                    rect: field,
                    radius: wisp_theme::RADIUS,
                    color: color::RAISED,
                });
                let shown = if self.step == Step::Code {
                    // Six boxes' worth: typed digits, then dots.
                    let mut s: String = self.typed.chars().flat_map(|d| [d, ' ']).collect();
                    for _ in self.typed.len()..6 {
                        s.push_str("· ");
                    }
                    s.trim_end().to_owned()
                } else {
                    self.typed.clone()
                };
                c.items.push(Item::Text {
                    face: Face::Mono,
                    size: 30.0,
                    x: field.x + 20.0,
                    y: field.y + 10.0,
                    color: color::TEXT,
                    text: shown,
                });
                c.gap(64.0 + 24.0);
                c.buttons(&[
                    (t("next").into(), Action::Next, Kind::Primary),
                    (t("back").into(), Action::Back, Kind::Secondary),
                ]);
                // The keypad: 1-9, then "." (address only), 0, erase.
                let px = c.x + c.w + 48.0;
                let mut keys: Vec<(String, Action)> = ('1'..='9')
                    .map(|d| (d.to_string(), Action::Key(d)))
                    .collect();
                keys.push(if self.step == Step::Address {
                    (".".into(), Action::Key('.'))
                } else {
                    (String::new(), Action::Erase)
                });
                keys.push(("0".into(), Action::Key('0')));
                keys.push(("<".into(), Action::Erase));
                let pad_top = ((ui.h - (4.0 * KEY + 3.0 * GAP)) / 2.0).max(24.0);
                for (i, (label, action)) in keys.into_iter().enumerate() {
                    if label.is_empty() {
                        continue;
                    }
                    let (row, col) = (i / 3, i % 3);
                    c.items.push(Item::Button {
                        rect: Rect {
                            x: px + col as f32 * (KEY + GAP),
                            y: pad_top + row as f32 * (KEY + GAP),
                            w: KEY,
                            h: KEY,
                        },
                        label,
                        action,
                        kind: Kind::Key,
                    });
                }
            }
            Step::Download => {
                title(&mut c, "downloading");
                c.text(Face::Regular, BODY, LINE, color::MUTED, t("download_note"));
                c.gap(24.0);
                let progress = shared.progress.unwrap_or_default();
                let fraction = if progress.total == 0 {
                    0.0
                } else {
                    progress.done as f32 / progress.total as f32
                };
                let bar = Rect {
                    x: c.x,
                    y: c.y,
                    w: c.w,
                    h: 10.0,
                };
                c.items.push(Item::Plate {
                    rect: bar,
                    radius: 5.0,
                    color: color::RAISED,
                });
                if fraction > 0.0 {
                    c.items.push(Item::Plate {
                        rect: Rect {
                            w: (bar.w * fraction).max(10.0),
                            ..bar
                        },
                        radius: 5.0,
                        color: color::ACCENT,
                    });
                }
                c.gap(10.0 + 12.0);
                let amount = format!(
                    "{} / {}  ({:.0} %)",
                    megabytes(progress.done),
                    megabytes(progress.total),
                    fraction * 100.0
                );
                c.text(Face::Mono, BODY, LINE, color::MUTED, &amount);
                c.gap(20.0);
                c.buttons(&[(t("cancel").into(), Action::Back, Kind::Secondary)]);
            }
        }
        if let Some(status) = &shared.status {
            c.gap(16.0);
            c.text(Face::Regular, BODY, LINE, color::MUTED, status);
        }
        if let Some(error) = &shared.error {
            c.gap(16.0);
            c.text(Face::Medium, BODY, LINE, color::hex(0xf07a68, 255), error);
        }
        // Centre the column vertically: shift everything but the keypad by the free space.
        let height = c.y;
        let shift = ((ui.h - height) / 2.0).max(24.0).round();
        let column_right = c.x + c.w + 1.0;
        let mut items = c.items;
        for item in &mut items {
            let (x, y) = match item {
                Item::Text { x, y, .. } => (*x, y),
                Item::Plate { rect, .. } | Item::Button { rect, .. } => (rect.x, &mut rect.y),
                Item::Circle { x, y, .. } | Item::Feather { x, y, .. } => (*x, y),
            };
            if x < column_right {
                *y += shift;
            }
        }
        items.extend(header());
        items
    }

    /// The home screen, the phone's launcher: Wisp and Play on the left, the account and the
    /// game files as cards on the right.
    fn home(&self, ui: &Ui, account: Option<&Account>, error: Option<&str>) -> Vec<Item> {
        let mut items = Vec::new();
        let ready = self.files_ready();
        let m = MARGIN;
        let split = (ui.w * 0.54).round();
        let caption = wisp_theme::CAPTION.px + 1.0;

        // Left: the feather, the name, Play (or the first step), and where things stand.
        let hero_w = (split - 2.0 * m).min(560.0);
        let hero_h = 88.0 + 14.0 + 64.0 + 30.0 + 34.0 + HERO + 18.0 + 24.0;
        let lx = m + 8.0;
        let mut y = ((ui.h - hero_h) / 2.0).max(m).round();
        items.push(Item::Feather {
            x: lx + 40.0,
            y: y + 44.0,
            size: 88.0,
        });
        y += 88.0 + 14.0;
        items.push(Item::Text {
            face: Face::Heading,
            size: 60.0,
            x: lx,
            y,
            color: color::TEXT,
            text: wisp_theme::brand::NAME.into(),
        });
        y += 64.0;
        items.push(Item::Text {
            face: Face::Regular,
            size: BODY + 2.0,
            x: lx,
            y,
            color: color::MUTED,
            text: tf("tagline", &[("version", wisp_core::VANILLA_VERSION)]),
        });
        y += 30.0 + 34.0;
        let (label, action) = if ready {
            (t("play"), Action::Play)
        } else {
            (t("get_files"), Action::GetFiles)
        };
        items.push(Item::Button {
            rect: Rect {
                x: lx,
                y,
                w: hero_w.min(380.0),
                h: HERO,
            },
            label: label.into(),
            action,
            kind: Kind::Hero,
        });
        y += HERO + 18.0;
        let (dot, state) = match (ready, account.is_some()) {
            (true, true) => (LIVE, t("ready_to_play")),
            (true, false) => (WARN, t("play_offline_note")),
            (false, _) => (WARN, t("setup_first")),
        };
        items.push(Item::Circle {
            x: lx + 6.0,
            y: y + 11.0,
            r: 5.0,
            color: dot,
        });
        let lines = wrap(ui, Face::Regular, caption, state, hero_w - 22.0);
        let below = y + 2.0 + lines.len() as f32 * 20.0 + 14.0;
        for (i, line) in lines.into_iter().enumerate() {
            items.push(Item::Text {
                face: Face::Regular,
                size: caption,
                x: lx + 22.0,
                y: y + 2.0 + i as f32 * 20.0,
                color: color::MUTED,
                text: line,
            });
        }
        if let Some(error) = error {
            for (i, line) in wrap(ui, Face::Medium, caption, error, hero_w)
                .into_iter()
                .enumerate()
            {
                items.push(Item::Text {
                    face: Face::Medium,
                    size: caption,
                    x: lx,
                    y: below + i as f32 * 20.0,
                    color: DANGER,
                    text: line,
                });
            }
        }

        // A faint rule between the halves.
        items.push(Item::Plate {
            rect: Rect {
                x: split,
                y: (ui.h * 0.22).round(),
                w: 1.0,
                h: (ui.h * 0.56).round(),
            },
            radius: 0.0,
            color: color::with_alpha(color::BORDER, 0.6),
        });

        // Right: the account and the game files, each a card.
        let card_w = (ui.w - split - 2.0 * m).clamp(260.0, 500.0);
        let cx = (ui.w - m - card_w).max(split + m * 0.5).round();
        let card_h = CARD_PAD * 2.0 + 18.0 + 6.0 + 28.0 + 18.0 + CONTROL;
        let mut y = ((ui.h - (2.0 * card_h + 20.0)) / 2.0).max(m).round();
        let (name, account_button) = match account {
            Some(a) => (a.name.clone(), (t("sign_out"), Action::SignOut)),
            None => (
                t("not_signed_in").to_owned(),
                (t("sign_in"), Action::SignIn),
            ),
        };
        let initial = account
            .and_then(|a| a.name.chars().next())
            .map(|c| c.to_uppercase().to_string());
        let files = if ready {
            tf("files_ready", &[("version", wisp_core::VANILLA_VERSION)])
        } else {
            t("files_missing").to_owned()
        };
        let files_button = if ready { "update_files" } else { "get_files" };
        let cards = [
            (t("account"), name, account_button, Badge::Avatar(initial)),
            (
                t("game_files"),
                files,
                (t(files_button), Action::GetFiles),
                Badge::Dot(if ready { LIVE } else { WARN }),
            ),
        ];
        for (label, value, (button, action), badge) in cards {
            items.push(Item::Plate {
                rect: Rect {
                    x: cx,
                    y,
                    w: card_w,
                    h: card_h,
                },
                radius: wisp_theme::glass::radius::PANEL,
                color: color::RAISED,
            });
            let (ix, iy) = (cx + CARD_PAD, y + CARD_PAD);
            // The badge: the account's initial in a circle, or the files' state as a dot.
            let bx = cx + card_w - CARD_PAD - 22.0;
            match badge {
                Badge::Avatar(initial) => {
                    let signed_in = initial.is_some();
                    items.push(Item::Circle {
                        x: bx,
                        y: iy + 22.0,
                        r: 22.0,
                        color: if signed_in {
                            color::ACCENT
                        } else {
                            color::HOVER
                        },
                    });
                    let letter = initial.unwrap_or_else(|| "?".into());
                    let lw = ui.measure(Face::Heading, 22.0, &letter);
                    items.push(Item::Text {
                        face: Face::Heading,
                        size: 22.0,
                        x: (bx - lw / 2.0).round(),
                        y: iy + 8.0,
                        color: if signed_in {
                            color::ON_ACCENT
                        } else {
                            color::MUTED
                        },
                        text: letter,
                    });
                }
                Badge::Dot(dot) => {
                    items.push(Item::Circle {
                        x: bx,
                        y: iy + 22.0,
                        r: 22.0,
                        color: color::with_alpha(dot, 0.16),
                    });
                    items.push(Item::Circle {
                        x: bx,
                        y: iy + 22.0,
                        r: 7.0,
                        color: dot,
                    });
                }
            }
            items.push(Item::Text {
                face: Face::Medium,
                size: wisp_theme::CAPTION.px,
                x: ix,
                y: iy,
                color: color::MUTED,
                text: label.to_uppercase(),
            });
            let value_w = card_w - 2.0 * CARD_PAD - 56.0;
            let value = wrap(ui, Face::Medium, BODY + 4.0, &value, value_w)
                .into_iter()
                .next()
                .unwrap_or_default();
            items.push(Item::Text {
                face: Face::Medium,
                size: BODY + 4.0,
                x: ix,
                y: iy + 18.0 + 6.0,
                color: color::TEXT,
                text: value,
            });
            let bw = (ui.measure(Face::Medium, BUTTON, button) + 48.0)
                .round()
                .clamp(140.0, card_w - 2.0 * CARD_PAD);
            items.push(Item::Button {
                rect: Rect {
                    x: ix,
                    y: iy + 18.0 + 6.0 + 28.0 + 18.0,
                    w: bw,
                    h: CONTROL,
                },
                label: button.into(),
                action,
                kind: Kind::Secondary,
            });
            y += card_h + 20.0;
        }
        items
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let items = self.layout(ui);
        backdrop(ui);
        for item in &items {
            match item {
                Item::Circle { x, y, r, color } => ui.circle(*x, *y, *r, *color),
                Item::Feather { x, y, size } => {
                    ui.feather(*x, *y, *size, color::TEXT, color::BG);
                }
                Item::Text {
                    face,
                    size,
                    x,
                    y,
                    color,
                    text,
                } => {
                    ui.text(*face, *size, *x, *y, *color, text);
                }
                Item::Plate {
                    rect,
                    radius,
                    color,
                } => ui.rounded(rect.x, rect.y, rect.w, rect.h, *radius, *color),
                Item::Button {
                    rect,
                    label,
                    kind,
                    action,
                } => {
                    let pressed = rect.contains(ui.cursor);
                    let (plate, ink, radius) = match kind {
                        Kind::Hero | Kind::Primary => {
                            (color::ACCENT, color::ON_ACCENT, rect.h / 2.0)
                        }
                        Kind::Secondary => (color::RAISED, color::TEXT, rect.h / 2.0),
                        Kind::Key => (color::RAISED, color::TEXT, wisp_theme::RADIUS_PLATE),
                    };
                    let plate = if pressed {
                        color::with_alpha(plate, 0.75)
                    } else {
                        plate
                    };
                    ui.rounded(rect.x, rect.y, rect.w, rect.h, radius, plate);
                    if *action == Action::Erase && *kind == Kind::Key {
                        // Backspace, drawn: the bundled fonts have no ⌫.
                        let (cx, cy, s) =
                            (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0, rect.h * 0.16);
                        ui.polyline(
                            &[
                                [cx + s, cy - s],
                                [cx - s * 0.2, cy - s],
                                [cx - s * 1.2, cy],
                                [cx - s * 0.2, cy + s],
                                [cx + s, cy + s],
                                [cx + s, cy - s],
                            ],
                            2.5,
                            ink,
                        );
                        continue;
                    }
                    let (face, size) = match kind {
                        Kind::Key => (Face::Medium, 26.0),
                        Kind::Hero => (Face::Heading, 24.0),
                        _ => (Face::Medium, BUTTON),
                    };
                    let lw = ui.measure(face, size, label);
                    ui.text_line(
                        face,
                        size,
                        rect.h,
                        rect.x + ((rect.w - lw) / 2.0).round(),
                        rect.y,
                        ink,
                        label,
                    );
                }
            }
        }
    }

    /// A tap at `ui.cursor`.
    pub fn tap(&mut self, ui: &Ui) -> Outcome {
        let action = self.layout(ui).into_iter().find_map(|item| match item {
            Item::Button { rect, action, .. } if rect.contains(ui.cursor) => Some(action),
            _ => None,
        });
        match action {
            Some(action) => self.act(action),
            None => Outcome::Stay,
        }
    }

    /// The Android back button: one step back, never out of the app from a step.
    pub fn back(&mut self) -> bool {
        if self.step == Step::Home {
            return false;
        }
        self.act(Action::Back);
        true
    }

    fn set_error(&self, error: Option<String>) {
        self.shared.lock().unwrap().error = error;
    }

    pub fn act(&mut self, action: Action) -> Outcome {
        match action {
            Action::Play => return Outcome::Play,
            Action::SignIn => {
                self.step = Step::SignIn;
                self.start_sign_in();
            }
            Action::SignOut => {
                Account::forget(&self.paths.private);
                self.shared.lock().unwrap().account = None;
            }
            Action::OpenLink => {
                let uri = self
                    .shared
                    .lock()
                    .unwrap()
                    .device
                    .as_ref()
                    .map(|d| d.verification_uri.clone());
                if let Some(uri) = uri {
                    open_url(&uri);
                }
            }
            Action::GetFiles => {
                self.set_error(None);
                self.step = Step::FindPc;
                self.search();
            }
            Action::Search => self.search(),
            Action::Pick(i) => {
                let found = self.shared.lock().unwrap().found.get(i).cloned();
                if let Some(pc) = found {
                    self.pc = Some(pc);
                    self.typed.clear();
                    self.set_error(None);
                    self.step = Step::Code;
                }
            }
            Action::TypeAddress => {
                self.typed.clear();
                self.set_error(None);
                self.step = Step::Address;
            }
            Action::Key(c) => {
                let limit = if self.step == Step::Code { 6 } else { 15 };
                if self.typed.len() < limit {
                    self.typed.push(c);
                }
            }
            Action::Erase => {
                self.typed.pop();
            }
            Action::Next => self.next(),
            Action::Back => {
                self.cancel.store(true, Ordering::Relaxed);
                let mut shared = self.shared.lock().unwrap();
                shared.error = None;
                shared.status = None;
                self.step = match self.step {
                    Step::Code if self.pc.as_ref().is_some_and(|p| p.name.is_empty()) => {
                        Step::Address
                    }
                    Step::Code | Step::Address => Step::FindPc,
                    _ => Step::Home,
                };
            }
        }
        Outcome::Stay
    }

    fn next(&mut self) {
        match self.step {
            Step::Address => match self.typed.parse::<IpAddr>() {
                Ok(ip) => {
                    self.pc = Some(Found {
                        addr: SocketAddr::new(ip, wisp_phone_link::TRANSFER_PORT),
                        name: String::new(),
                    });
                    self.typed.clear();
                    self.set_error(None);
                    self.step = Step::Code;
                }
                Err(_) => self.set_error(Some(tf(
                    "unreachable",
                    &[("error", &format!("\"{}\"", self.typed))],
                ))),
            },
            Step::Code if self.typed.len() == 6 => self.start_download(),
            _ => {}
        }
    }

    /// A fresh cancel flag for a new job (the old job's stays set).
    fn new_job(&mut self) -> Arc<AtomicBool> {
        self.cancel = Arc::new(AtomicBool::new(false));
        Arc::clone(&self.cancel)
    }

    fn search(&mut self) {
        {
            let mut shared = self.shared.lock().unwrap();
            if shared.searching {
                return;
            }
            shared.searching = true;
            shared.found.clear();
        }
        let shared = Arc::clone(&self.shared);
        let _ = std::thread::Builder::new()
            .name("phone-link-search".into())
            .spawn(move || {
                let found = wisp_phone_link::discover(Duration::from_secs(3)).unwrap_or_default();
                let mut shared = shared.lock().unwrap();
                shared.found = found;
                shared.searching = false;
            });
    }

    fn start_sign_in(&mut self) {
        let cancel = self.new_job();
        let shared = Arc::clone(&self.shared);
        let private = self.paths.private.clone();
        {
            let mut s = shared.lock().unwrap();
            s.busy = true;
            s.device = None;
            s.error = None;
            s.status = Some(t("getting_code").into());
        }
        let _ = std::thread::Builder::new()
            .name("phone-sign-in".into())
            .spawn(move || {
                let result = msa::device_code().and_then(|device| {
                    {
                        let mut s = shared.lock().unwrap();
                        s.device = Some(device.clone());
                        s.status = Some(t("waiting_browser").into());
                    }
                    msa::wait_for_sign_in(&device, &|| cancel.load(Ordering::Relaxed), &|phase| {
                        shared.lock().unwrap().status = Some(phase.into())
                    })
                });
                let result = result.and_then(|account| account.save(&private).map(|()| account));
                let mut s = shared.lock().unwrap();
                s.busy = false;
                s.status = None;
                s.device = None;
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match result {
                    Ok(account) => {
                        s.account = Some(account);
                        s.go = Some(Step::Home);
                    }
                    Err(e) => s.error = Some(e),
                }
            });
    }

    fn start_download(&mut self) {
        let Some(pc) = self.pc.clone() else {
            return;
        };
        let code = std::mem::take(&mut self.typed);
        let cancel = self.new_job();
        let shared = Arc::clone(&self.shared);
        let target = self.paths.resources.clone();
        let private = self.paths.private.clone();
        {
            let mut s = shared.lock().unwrap();
            s.busy = true;
            s.error = None;
            s.progress = None;
        }
        self.step = Step::Download;
        let _ = std::thread::Builder::new()
            .name("phone-link-download".into())
            .spawn(move || {
                let result = fetch(pc.addr, &code, &target, &cancel, &shared)
                    .map(|wisp_server| super::save_wisp_server(&private, wisp_server.as_deref()));
                let mut s = shared.lock().unwrap();
                s.busy = false;
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match result {
                    Ok(()) => {
                        s.status = Some(t("done").into());
                        s.go = Some(Step::Home);
                    }
                    Err((step, e)) => {
                        s.error = Some(e);
                        s.go = Some(step);
                    }
                }
            });
    }
}

/// Checks the PC's offer, then copies it. On failure: the step to go back to and why.
fn fetch(
    addr: SocketAddr,
    code: &str,
    target: &std::path::Path,
    cancel: &AtomicBool,
    shared: &Mutex<Shared>,
) -> Result<Option<String>, (Step, String)> {
    let unreachable = |e: &dyn std::fmt::Display| {
        (
            Step::FindPc,
            tf("unreachable", &[("error", &e.to_string())]),
        )
    };
    let hello = wisp_phone_link::hello(addr).map_err(|e| unreachable(&e))?;
    if hello.version != wisp_core::VANILLA_VERSION {
        return Err((
            Step::Home,
            tf(
                "other_version",
                &[
                    ("theirs", &hello.version),
                    ("ours", wisp_core::VANILLA_VERSION),
                ],
            ),
        ));
    }
    if hello.schema == 0 {
        return Err((Step::Home, t("pc_not_ready").into()));
    }
    if hello.schema != wisp_phone_link::RESOURCES_SCHEMA {
        return Err((Step::Home, t("other_layout").into()));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| unreachable(&e))?;
    }
    wisp_phone_link::download(addr, code, target, cancel, &mut |p| {
        shared.lock().unwrap().progress = Some(p);
    })
    .map_err(|e| {
        use wisp_phone_link::DownloadError as E;
        match e {
            E::WrongCode => (Step::Code, t("wrong_code").into()),
            E::Closed => (Step::FindPc, t("offer_closed").into()),
            E::Busy => (Step::FindPc, t("busy").into()),
            E::Cancelled => (Step::Home, String::new()),
            E::Io(e) => (
                Step::FindPc,
                tf("copy_failed", &[("error", &e.to_string())]),
            ),
        }
    })?;
    Ok(hello.wisp_server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_client::hud::{HudAtlas, HudBatch};

    fn paths(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("wisp-phone-setup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Paths {
            resources: root.join("resources").join(wisp_core::VANILLA_VERSION),
            private: root.join("private"),
        }
    }

    /// Runs `f` over a Ui of a landscape phone (2400 × 1080), the finger at `at` (dp).
    fn on_phone<R>(at: (f32, f32), f: impl FnOnce(&mut Ui) -> R) -> R {
        let atlas = HudAtlas::build(std::path::Path::new("/nonexistent/wisp-test-assets")).unwrap();
        let mut batch = HudBatch::new([2400, 1080]);
        let density = Ui::new(&atlas, &mut batch, (0.0, 0.0)).density;
        let gui = (at.0 * density / batch.scale, at.1 * density / batch.scale);
        let mut ui = Ui::new(&atlas, &mut batch, gui);
        f(&mut ui)
    }

    /// The centre of the button doing `action`, in dp.
    fn button(setup: &mut Setup, action: Action) -> (f32, f32) {
        on_phone((0.0, 0.0), |ui| {
            setup.layout(ui).into_iter().find_map(|item| match item {
                Item::Button {
                    rect, action: a, ..
                } if a == action => Some((rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)),
                _ => None,
            })
        })
        .unwrap_or_else(|| panic!("no {action:?} button"))
    }

    fn tap(setup: &mut Setup, action: Action) -> Outcome {
        let at = button(setup, action);
        on_phone(at, |ui| setup.tap(ui))
    }

    #[test]
    fn play_appears_once_the_files_are_there() {
        let paths = paths("play");
        let mut setup = Setup::new(paths.clone());
        on_phone((0.0, 0.0), |ui| {
            assert!(!setup.layout(ui).iter().any(|i| matches!(
                i,
                Item::Button {
                    action: Action::Play,
                    ..
                }
            )));
        });
        std::fs::create_dir_all(&paths.resources).unwrap();
        std::fs::write(paths.resources.join("wisp-resources.json"), b"{}").unwrap();
        assert_eq!(tap(&mut setup, Action::Play), Outcome::Play);
    }

    #[test]
    fn the_code_is_typed_on_the_keypad() {
        let mut setup = Setup::new(paths("keypad"));
        setup.pc = Some(Found {
            addr: "127.0.0.1:1".parse().unwrap(),
            name: "PC".into(),
        });
        setup.step = Step::Code;
        for digit in ['4', '0', '7', '1', '9', '3', '5'] {
            tap(&mut setup, Action::Key(digit));
        }
        // Six digits at most.
        assert_eq!(setup.typed, "407193");
        tap(&mut setup, Action::Erase);
        assert_eq!(setup.typed, "40719");
        // Every button lies inside the screen.
        on_phone((0.0, 0.0), |ui| {
            for item in setup.layout(ui) {
                if let Item::Button { rect, .. } = item {
                    assert!(rect.x >= 0.0 && rect.x + rect.w <= ui.w, "x {}", rect.x);
                    assert!(rect.y >= 0.0 && rect.y + rect.h <= ui.h, "y {}", rect.y);
                }
            }
        });
    }

    #[test]
    fn a_typed_address_leads_to_the_code() {
        let mut setup = Setup::new(paths("address"));
        setup.act(Action::TypeAddress);
        for c in "192.168.1.20".chars() {
            tap(&mut setup, Action::Key(c));
        }
        tap(&mut setup, Action::Next);
        assert_eq!(setup.step, Step::Code);
        assert_eq!(
            setup.pc.as_ref().unwrap().addr,
            "192.168.1.20:25592".parse().unwrap()
        );
        // Back from the code goes back to the address it came from.
        assert!(setup.back());
        assert_eq!(setup.step, Step::Address);
    }

    #[test]
    fn the_whole_copy_from_a_pc() {
        // A launcher's offer on this machine, and the phone's setup getting it.
        let from = std::env::temp_dir().join(format!("wisp-phone-setup-pc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&from);
        std::fs::create_dir_all(from.join("assets")).unwrap();
        std::fs::write(
            from.join("wisp-resources.json"),
            format!("{{\"schema\":{}}}", wisp_phone_link::RESOURCES_SCHEMA),
        )
        .unwrap();
        std::fs::write(from.join("assets/x.png"), vec![1u8; 50_000]).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _server = wisp_phone_link::Server::with_listener(
            listener,
            wisp_phone_link::Offer {
                root: from,
                version: wisp_core::VANILLA_VERSION.into(),
                name: "PC".into(),
                code: "246810".into(),
                wisp_server: Some("https://wisp.example".into()),
            },
        )
        .unwrap();
        let paths = paths("copy");
        let mut setup = Setup::new(paths.clone());
        setup.pc = Some(Found {
            addr,
            name: "PC".into(),
        });
        setup.step = Step::Code;
        for d in "246810".chars() {
            setup.act(Action::Key(d));
        }
        setup.act(Action::Next);
        assert_eq!(setup.step, Step::Download);
        let started = std::time::Instant::now();
        while setup.animating() {
            assert!(started.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(20));
        }
        setup.settle();
        assert_eq!(setup.step, Step::Home);
        assert!(setup.files_ready());
        assert_eq!(
            super::super::wisp_server(&paths.private).as_deref(),
            Some("https://wisp.example")
        );
        assert_eq!(
            std::fs::read(paths.resources.join("assets/x.png"))
                .unwrap()
                .len(),
            50_000
        );
    }
}
