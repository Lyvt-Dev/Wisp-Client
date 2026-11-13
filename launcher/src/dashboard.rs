use gpui::{
    actions, div, img, prelude::*, px, relative, rgb, rgba, size, svg, App, Bounds, Context, Div,
    ElementId, FocusHandle, FontWeight, KeyBinding, ObjectFit, PathPromptOptions, SharedString,
    Stateful, Svg, TitlebarOptions, Window, WindowBounds, WindowControlArea, WindowOptions,
};
use std::{
    borrow::Cow,
    path::PathBuf,
    process::Child,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use wisp_launcher::{
    auth, engine,
    i18n::{tr, trf},
    installation, Profile, Settings,
};

mod actions_impl;
mod catalog_views;
mod content;
mod friends_view;
mod locker_view;
mod mods_view;
mod packs_servers;
mod profile_view;
mod reference_views;
mod search_input;
mod settings_view;
mod shaders_view;
mod shop_view;
mod theme;
mod versions_view;
mod views;
use catalog_views::CatalogUi;
use search_input::SearchInput;
use theme::{accent_tone, accent_tone_a, tone, tone_a};

// Dark neutral greys with a green primary, laid out like the owner's launcher reference.
const PAGE: u32 = 0x111215;
const SURFACE: u32 = 0x18191d;
const RAISED: u32 = 0x232429;
const RULE: u32 = 0x2b2c32;
const INK: u32 = 0xf2f3f5;
const MUTED: u32 = 0x9a9ca4;
const ACCENT: u32 = 0x3ccf6a;
/// The window frame: top bar and icon rail, a shade darker than the page.
const FRAME: u32 = 0x0b0b0d;
const GUIDE: &str = "https://github.com/Lyvt-Dev/Wisp-Client/blob/codex/rust-engine-login/launcher/docs/microsoft-sign-in.md";

actions!(wisp_dashboard, [Play, Add, Next, ShowHome, ShowSetup]);

struct Assets;
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "artwork/hero.png" => Some(include_bytes!("../assets/artwork/wisp-hero.png")),
            "artwork/news.png" => Some(include_bytes!("../assets/artwork/wisp-news.png")),
            "artwork/aurora.png" => Some(include_bytes!("../assets/artwork/aurora-preview.png")),
            "artwork/logo.png" => {
                Some(include_bytes!("../../crates/wisp-theme/icons/wisp-256.png"))
            }
            "artwork/mountains" => Some(include_bytes!("../assets/artwork/sidebar-mountains.svg")),
            "artwork/ribbon" => Some(include_bytes!("../assets/artwork/ribbon.svg")),
            "icons/cpu" => Some(include_bytes!("../assets/icons/cpu.svg")),
            "icons/wisp" => Some(include_bytes!("../assets/icons/wisp.svg")),
            "icons/home" => Some(include_bytes!("../assets/icons/home.svg")),
            "icons/rocket" => Some(include_bytes!("../assets/icons/rocket.svg")),
            "icons/gear" => Some(include_bytes!("../assets/icons/gear.svg")),
            "icons/clock" => Some(include_bytes!("../assets/icons/clock.svg")),
            "artwork/zedix.png" => Some(include_bytes!("../assets/shop/zedix_cape_logo.png")),
            "icons/shield-check" => Some(include_bytes!("../assets/icons/shield-check.svg")),
            "icons/user-circle" => Some(include_bytes!("../assets/icons/user-circle.svg")),
            "icons/refresh" => Some(include_bytes!("../assets/icons/refresh.svg")),
            "icons/log-out" => Some(include_bytes!("../assets/icons/log-out.svg")),
            "icons/spark" => Some(include_bytes!("../assets/icons/spark.svg")),
            "icons/server" => Some(include_bytes!("../assets/icons/server.svg")),
            "icons/layers" => Some(include_bytes!("../assets/icons/layers.svg")),
            "icons/sun" => Some(include_bytes!("../assets/icons/sun.svg")),
            "icons/check-circle" => Some(include_bytes!("../assets/icons/check-circle.svg")),
            "icons/star" => Some(include_bytes!("../assets/icons/star.svg")),
            "icons/notification-01" => Some(include_bytes!("../assets/icons/bell.svg")),
            "icons/compass" => Some(include_bytes!("../assets/icons/compass.svg")),
            "icons/zoom" => Some(include_bytes!("../assets/icons/zoom.svg")),
            "icons/cart" => Some(include_bytes!("../assets/icons/cart.svg")),
            "icons/coin" => Some(include_bytes!("../assets/icons/coin.svg")),
            "icons/cape" => Some(include_bytes!("../assets/icons/cape.svg")),
            "icons/badge" => Some(include_bytes!("../assets/icons/badge.svg")),
            "icons/pet" => Some(include_bytes!("../assets/icons/pet.svg")),
            "icons/trail" => Some(include_bytes!("../assets/icons/trail.svg")),
            "icons/aura" => Some(include_bytes!("../assets/icons/aura.svg")),
            "icons/emote" => Some(include_bytes!("../assets/icons/emote.svg")),
            "icons/sword" => Some(include_bytes!("../assets/icons/sword.svg")),
            "icons/wings" => Some(include_bytes!("../assets/icons/wings.svg")),
            "icons/hat" => Some(include_bytes!("../assets/icons/hat.svg")),
            "icons/chevron-left" => Some(include_bytes!("../assets/icons/chevron-left.svg")),
            "icons/chevron-right" => Some(include_bytes!("../assets/icons/chevron-right.svg")),
            "artwork/catalog-sidebar" => Some(include_bytes!("../assets/artwork/sidebar-wave.svg")),
            _ => path
                .strip_prefix("shop/")
                .and_then(wisp_launcher::shop::picture)
                .or_else(|| {
                    path.strip_prefix("icons/")
                        .and_then(wisp_theme::glass_icons::get)
                }),
        };
        Ok(bytes.map(Cow::Borrowed))
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

fn text(value: impl Into<SharedString>) -> SharedString {
    value.into()
}
fn icon(name: &'static str, color: u32) -> Svg {
    let name = match name {
        "box" => "package-01",
        "folder" => "folder-01",
        "plus" => "plus-sign",
        "down" => "arrow-down-01",
        "arrow" => "arrow-right-01",
        "terminal" => "computer-terminal-01",
        "settings" => "sliders-horizontal",
        "download" => "download-04",
        "info" => "information-circle",
        "close" => "cancel-01",
        "minus" => "minus-sign",
        "copy" => "copy-01",
        "grid-view" => "dashboard-square-01",
        "checkmark-circle-01" => "tick-02",
        _ => name,
    };
    svg()
        .path(format!("icons/{name}"))
        .size(px(20.))
        .flex_none()
        .text_color(rgb(color))
}
fn panel() -> Div {
    div()
        .rounded_xl()
        .border_1()
        .border_color(rgb(tone(RULE)))
        .bg(rgb(tone(SURFACE)))
}
fn button(id: impl Into<ElementId>, label: &str, glyph: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .px_4()
        .py_3()
        .rounded_lg()
        .bg(rgb(tone(RAISED)))
        .border_1()
        .border_color(rgb(tone(RULE)))
        .text_color(rgb(INK))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x2b2438)))
        .child(icon(glyph, MUTED))
        .child(tr(label))
}
fn quiet(message: &str) -> Div {
    div()
        .text_sm()
        .line_height(px(21.))
        .text_color(rgb(MUTED))
        .child(tr(message))
}
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Instances,
    Packs,
    Shaders,
    Servers,
    Mods,
    Shop,
    Locker,
    Settings,
    Account,
    Setup,
    About,
}

enum Event {
    Loaded(Result<Option<auth::SignedIn>, String>),
    Code(auth::DeviceCode),
    Status(String),
    SignedIn(Result<auth::SignedIn, String>),
    Launched(Result<(Child, Option<auth::SignedIn>, String), String>),
    Added(Result<Profile, String>),
    Prepared(Result<Profile, String>),
    Installed(Result<installation::Installed, String>),
    SetupProgress(engine::Progress),
    Avatar(String, Option<crate::account_art::Art>),
    Storage(Vec<settings_view::Folder>),
    Ping(String, Result<wisp_launcher::server_ping::Status, String>),
    Friends(friends_view::FriendsUpdate),
    FriendHead(String, Option<Vec<[u8; 4]>>),
}

struct Launcher {
    catalog: CatalogUi,
    content: content::Content,
    locker: locker_view::LockerUi,
    /// Commands for the friends worker.
    friends_tx: mpsc::Sender<friends_view::FriendCmd>,
    /// The friends list as the server last sent it.
    friends: Option<wisp_launcher::friends::Snapshot>,
    /// Why friends don't work right now, if they don't.
    friends_state: Option<String>,
    /// What the last friends action did; `true` when it failed.
    friends_notice: Option<(String, bool)>,
    friend_heads: std::collections::HashMap<String, Option<Arc<gpui::RenderImage>>>,
    friend_input: gpui::Entity<SearchInput>,
    service_input: gpui::Entity<SearchInput>,
    /// The server the game was started on, for the friends list.
    last_server: Option<String>,
    /// The account's skin as a 64 x 64 texture and whether its arms are slim.
    skin: Option<(Vec<u8>, bool)>,
    /// Status of each saved server by address; None while its ping runs.
    pings: std::collections::HashMap<
        String,
        Option<Result<wisp_launcher::server_ping::Status, String>>,
    >,
    /// The addresses last pinged and when, so the Servers page pings again after a while.
    pinged: Option<(Vec<String>, std::time::Instant)>,
    mod_search: gpui::Entity<SearchInput>,
    version_search: gpui::Entity<SearchInput>,
    shader_search: gpui::Entity<SearchInput>,
    shop_search: gpui::Entity<SearchInput>,
    shop: shop_view::ShopUi,
    wallet: wisp_launcher::shop::Wallet,
    home: PathBuf,
    settings: Settings,
    prefs: wisp_launcher::prefs::Prefs,
    settings_section: settings_view::Section,
    storage: Option<Vec<settings_view::Folder>>,
    /// Minimize the window at the next frame (after the game started).
    minimize_pending: bool,
    /// Ticks of the 200 ms timer since the Home banner last turned.
    hero_ticks: u32,
    status: String,
    failed: bool,
    child: Option<Child>,
    focus: FocusHandle,
    page: Page,
    slide: usize,
    ui_scale: f32,
    instance_menu: bool,
    avatar_key: Option<String>,
    avatar: Option<Arc<gpui::RenderImage>>,
    /// The player's whole front view, for the profile page.
    body: Option<Arc<gpui::RenderImage>>,
    picking: bool,
    auth_busy: bool,
    launching: bool,
    setup_busy: bool,
    install_after_sign_in: bool,
    account: Option<auth::SignedIn>,
    /// Every account kept, to switch between (the one that plays among them).
    accounts: Vec<auth::Account>,
    /// The browser sign-in is open, waiting for the player.
    browser_waiting: bool,
    device: Option<auth::DeviceCode>,
    setup_fraction: Option<f32>,
    auth_cancel: Arc<AtomicBool>,
    setup_cancel: Arc<AtomicBool>,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl Drop for Launcher {
    fn drop(&mut self) {
        self.auth_cancel.store(true, Ordering::Relaxed);
        self.setup_cancel.store(true, Ordering::Relaxed);
    }
}

impl Launcher {
    fn new(home: PathBuf, settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mod_search = cx.new(|cx| SearchInput::new("Search mods…", focus.clone(), cx));
        let version_search = cx.new(|cx| SearchInput::new("Search instances…", focus.clone(), cx));
        let shader_search = cx.new(|cx| SearchInput::new("Search shaders…", focus.clone(), cx));
        let shop_search = cx.new(|cx| SearchInput::new("Search the shop…", focus.clone(), cx));
        for input in [&mod_search, &version_search, &shader_search, &shop_search] {
            cx.subscribe(input, |_, _, _: &search_input::Changed, cx| cx.notify())
                .detach();
        }
        let (sender, receiver) = mpsc::channel();
        let prefs = wisp_launcher::prefs::Prefs::load(&home);
        wisp_launcher::shop::set_service(Some(&prefs.service_url));
        let (friends_tx, friends_rx) = mpsc::channel();
        {
            let (home, events) = (home.clone(), sender.clone());
            std::thread::spawn(move || friends_view::worker(home, events, friends_rx));
        }
        let friend_input = cx.new(|cx| SearchInput::new("Minecraft name…", focus.clone(), cx));
        cx.subscribe(&friend_input, |_, _, _: &search_input::Changed, cx| {
            cx.notify()
        })
        .detach();
        let service_input = cx.new(|cx| {
            let mut input = SearchInput::new("https://your-wisp-server…", focus.clone(), cx);
            input.value = prefs.service_url.clone();
            input
        });
        cx.subscribe(
            &service_input,
            |this, input, _: &search_input::Changed, cx| {
                let url = input.read(cx).value.trim().to_owned();
                if url != this.prefs.service_url {
                    this.set_prefs(|p| p.service_url = url.clone(), cx);
                    wisp_launcher::shop::set_service(Some(&url));
                    this.friends = None;
                    this.friends_state = None;
                    this.friends_send(friends_view::FriendCmd::Reset);
                }
                cx.notify();
            },
        )
        .detach();
        let startup = sender.clone();
        let store = home.clone();
        std::thread::spawn(move || {
            let _ = startup.send(Event::Loaded(auth::saved(&store)));
        });
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(200)).await;
            if this.update(cx, |this, cx| {
                let events = this.receiver.try_iter().collect::<Vec<_>>();
                let mut changed = !events.is_empty();
                for event in events { this.event(event, cx); }
                if let Some(result) = this.child.as_mut().map(Child::try_wait) {
                    match result {
                        Ok(Some(status)) => {
                            this.child = None;
                            this.friends_send(friends_view::FriendCmd::Presence("launcher", None));
                            this.failed = !status.success();
                            this.status = tr(if status.success() { "Minecraft closed. Ready when you are." }
                                else { "Minecraft stopped with an error. Open the game log for details." });
                            changed = true;
                        }
                        Err(error) => { this.status = error.to_string(); this.failed = true; changed = true; }
                        Ok(None) => {}
                    }
                }
                if this.page == Page::Shop {
                    this.shop.slide_ticks += 1;
                    if this.shop.slide_ticks >= 40 {
                        this.shop.slide_ticks = 0;
                        this.shop.slide = (this.shop.slide + 1) % shop_view::SLIDES.len();
                        changed = true;
                    }
                }
                if this.page == Page::Home && this.prefs.hero_autoplay {
                    this.hero_ticks += 1;
                    if this.hero_ticks >= 40 {
                        this.hero_ticks = 0;
                        this.slide = (this.slide + 1) % 3;
                        changed = true;
                    }
                }
                if changed { cx.notify(); }
            }).is_err() { break; }
        }).detach();
        let mut wallet = wisp_launcher::shop::Wallet::load(&home);
        let earned = wallet.collect(wisp_launcher::shop::today());
        if earned > 0 {
            let _ = wallet.save(&home);
        }
        let accounts = auth::accounts(&home).map(|(a, _)| a).unwrap_or_default();
        let start = match prefs.start_page {
            wisp_launcher::prefs::StartPage::Home => Page::Home,
            wisp_launcher::prefs::StartPage::Versions => Page::Instances,
            wisp_launcher::prefs::StartPage::Shop => Page::Shop,
        };
        let launcher = Self {
            catalog: CatalogUi::default(),
            content: Default::default(),
            locker: Default::default(),
            friends_tx,
            friends: None,
            friends_state: None,
            friends_notice: None,
            friend_heads: Default::default(),
            friend_input,
            service_input,
            last_server: None,
            skin: None,
            pings: Default::default(),
            pinged: None,
            mod_search,
            version_search,
            shader_search,
            shop_search,
            shop: Default::default(),
            wallet,
            home,
            settings,
            prefs,
            settings_section: settings_view::Section::from_env(),
            storage: None,
            minimize_pending: false,
            hero_ticks: 0,
            status: tr("Your worlds stay on this computer."),
            failed: false,
            child: None,
            focus,
            // Development builds only: open a page to look at it (or film it).
            page: match wisp_compat::var("WISP_LAUNCHER_PAGE")
                .ok()
                .filter(|_| cfg!(debug_assertions))
                .as_deref()
            {
                Some("shop") => Page::Shop,
                Some("versions") => Page::Instances,
                Some("packs") => Page::Packs,
                Some("shaders") => Page::Shaders,
                Some("servers") => Page::Servers,
                Some("mods") => Page::Mods,
                Some("settings") => Page::Settings,
                Some("account" | "profile") => Page::Account,
                Some("locker") => Page::Locker,
                _ => start,
            },
            slide: 0,
            ui_scale: 0.8,
            instance_menu: false,
            avatar_key: None,
            avatar: None,
            body: None,
            picking: false,
            auth_busy: true,
            launching: false,
            setup_busy: false,
            install_after_sign_in: false,
            account: None,
            accounts,
            browser_waiting: false,
            device: None,
            setup_fraction: None,
            auth_cancel: Arc::new(AtomicBool::new(false)),
            setup_cancel: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        };
        launcher.write_cosmetic_prefs();
        launcher
    }

    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Loaded(result) => {
                self.auth_busy = false;
                match result {
                    Ok(account) => {
                        self.account = account;
                        self.load_avatar();
                        self.status = tr(if self.settings.current().is_none() {
                            "Create an instance to install Minecraft."
                        } else if self.settings.current().is_some_and(|p| p.use_account)
                            && self.account.is_none()
                        {
                            "Sign in to play the selected instance."
                        } else {
                            "Ready to play."
                        });
                    }
                    Err(error) => self.error(error),
                }
            }
            Event::Code(code) => {
                self.device = Some(code);
                self.status = tr("Finish sign-in at microsoft.com/link.");
            }
            Event::Status(status) => self.status = status,
            Event::SignedIn(result) => {
                self.auth_busy = false;
                self.browser_waiting = false;
                self.device = None;
                self.refresh_accounts();
                match result {
                    Ok(account) => {
                        self.status =
                            trf("Signed in as {name}.", &[("name", &account.account.name)]);
                        self.failed = false;
                        self.account = Some(account);
                        self.load_avatar();
                        if self.install_after_sign_in {
                            self.install_after_sign_in = false;
                            self.install(cx);
                        }
                    }
                    Err(error) => {
                        self.install_after_sign_in = false;
                        self.error(error);
                    }
                }
            }
            Event::Launched(result) => {
                self.launching = false;
                match result {
                    Ok((child, account, name)) => {
                        self.friends_send(friends_view::FriendCmd::Presence(
                            "playing",
                            self.last_server.clone(),
                        ));
                        if let Err(error) =
                            wisp_launcher::catalog::Preferences::launched(&self.home, &name)
                        {
                            eprintln!("Wisp launch history: {error}");
                        }
                        self.child = Some(child);
                        if account.is_some() {
                            self.account = account;
                            self.load_avatar();
                        }
                        self.failed = false;
                        self.status = tr("Minecraft is running. Closing Wisp keeps the game open.");
                        match self.prefs.after_launch {
                            wisp_launcher::prefs::AfterLaunch::KeepOpen => {}
                            wisp_launcher::prefs::AfterLaunch::Minimize => {
                                self.minimize_pending = true;
                            }
                            wisp_launcher::prefs::AfterLaunch::Close => cx.quit(),
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
            Event::Added(result) | Event::Prepared(result) => {
                self.picking = false;
                self.setup_busy = false;
                self.setup_fraction = None;
                match result {
                    Ok(profile) => {
                        self.save_profile(profile);
                    }
                    Err(error) => self.error(error),
                }
            }
            Event::Installed(result) => {
                self.setup_busy = false;
                self.setup_fraction = None;
                match result {
                    Ok(_) if self.setup_cancel.load(Ordering::Relaxed) => {
                        self.failed = false;
                        self.status =
                            tr("Installation cancelled. Choose Install & play to resume.");
                    }
                    Ok(installed) => {
                        self.account = Some(installed.account);
                        if self.save_profile(installed.profile) {
                            self.play(cx);
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
            Event::SetupProgress(progress) => {
                self.status = progress.message;
                if progress.fraction.is_some() {
                    self.setup_fraction = progress.fraction;
                }
            }
            Event::Storage(folders) => self.storage = Some(folders),
            Event::Friends(update) => match update {
                friends_view::FriendsUpdate::List(list) => {
                    let uuids: Vec<String> = list
                        .friends
                        .iter()
                        .map(|f| f.uuid.clone())
                        .chain(list.incoming.iter().map(|p| p.uuid.clone()))
                        .chain(list.outgoing.iter().map(|p| p.uuid.clone()))
                        .collect();
                    for uuid in uuids {
                        self.friend_head(&uuid);
                    }
                    self.friends = Some(list);
                    self.friends_state = None;
                }
                friends_view::FriendsUpdate::Unavailable(why) => self.friends_state = Some(why),
                friends_view::FriendsUpdate::Notice(text, bad) => {
                    self.friends_notice = Some((text, bad))
                }
            },
            Event::FriendHead(uuid, face) => {
                self.friend_heads
                    .insert(uuid, face.map(|f| crate::art::bake(&f, 8, 8, 8., 1.)));
            }
            Event::Ping(address, result) => {
                self.pings.insert(address, Some(result));
            }
            Event::Avatar(key, art) => {
                if self.avatar_key.as_ref() == Some(&key) {
                    use crate::account_art::{BODY_H, BODY_W};
                    self.avatar = art
                        .as_ref()
                        .map(|a| crate::art::bake(&a.face, 8, 8, 8., 1.));
                    self.body = art
                        .as_ref()
                        .map(|a| crate::art::bake(&a.body, BODY_W, BODY_H, 8., 1.));
                    self.skin = art.map(|a| (a.skin, key.ends_with("#slim")));
                }
            }
        }
    }

    fn load_avatar(&mut self) {
        let skin = self
            .account
            .as_ref()
            .and_then(|a| a.account.skin.as_ref())
            .cloned();
        let key = skin.as_ref().map(|(url, model)| format!("{url}#{model}"));
        if self.avatar_key == key {
            return;
        }
        self.avatar_key = key.clone();
        self.avatar = None;
        self.body = None;
        self.skin = None;
        if let (Some(key), Some((url, model))) = (key, skin) {
            let sender = self.sender.clone();
            std::thread::spawn(move || {
                let art = crate::account_art::fetch(&url, model == "slim");
                let _ = sender.send(Event::Avatar(key, art));
            });
        }
    }

    fn error(&mut self, message: String) {
        self.failed = true;
        self.status = tr(&message);
    }

    fn save_profile(&mut self, profile: Profile) -> bool {
        let mut settings = self.settings.clone();
        match settings
            .add(profile)
            .and_then(|_| settings.save(&self.home))
        {
            Ok(()) => {
                self.settings = settings;
                self.page = Page::Home;
                self.failed = false;
                self.status = tr("Instance ready. Press Play to start Minecraft.");
                true
            }
            Err(error) => {
                self.error(error.to_string());
                false
            }
        }
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        self.ui_scale = ((f32::from(viewport.width) / 1672.)
            .min(f32::from(viewport.height) / 941.)
            * self.prefs.ui_size)
            .clamp(0.45, 1.6);
        if std::mem::take(&mut self.minimize_pending) {
            window.minimize_window();
        }
        theme::apply(&self.prefs);
        self.content.refresh(
            &self.home,
            self.settings.current().map(|p| p.game_home.as_path()),
            self.page as u8,
        );
        if matches!(self.page, Page::Servers | Page::Home) {
            self.refresh_pings(false);
        }
        let s = self.ui_scale;
        div()
            .id("wisp-dashboard")
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(rgb(tone(FRAME)))
            .text_color(rgb(INK))
            .font_family("Geist")
            .text_size(px(18. * s))
            .track_focus(&self.focus)
            .key_context("Wisp")
            .on_action(cx.listener(|this, _: &Play, _, cx| this.play(cx)))
            .on_action(cx.listener(|this, _: &Add, _, cx| this.new_instance(cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| this.next(cx)))
            .on_action(cx.listener(|this, _: &ShowHome, _, cx| {
                this.page = Page::Home;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowSetup, _, cx| {
                this.page = Page::Setup;
                cx.notify();
            }))
            .flex_col()
            .child(self.titlebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .rounded_tl(px(14. * s))
                            .border_t_1()
                            .border_l_1()
                            .border_color(rgb(tone(RULE)))
                            .bg(rgb(tone(PAGE)))
                            .px(px(22. * s))
                            .pt(px(if self.page == Page::Home { 4. } else { 18. } * s))
                            .pb(px(if self.is_catalog() { 18. } else { 0. } * s))
                            .child(
                                div()
                                    .id("page-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .when(self.is_catalog(), |d| d.overflow_hidden())
                                    .when(!self.is_catalog(), |d| d.overflow_y_scroll())
                                    .child(self.page_content(cx)),
                            )
                            .when(self.failed && self.page != Page::Home, |s| {
                                s.child(
                                    panel()
                                        .id("error-message")
                                        .flex_none()
                                        .max_h(px(100.))
                                        .overflow_y_scroll()
                                        .my_2()
                                        .p_3()
                                        .border_color(rgb(0x6a2f3a))
                                        .child(quiet(&self.status)),
                                )
                            }),
                    )
                    .when(
                        self.prefs.side_panel && f32::from(viewport.width) >= 1100.,
                        |d| d.child(self.friends_panel(cx)),
                    ),
            )
    }
}

pub fn run(home: PathBuf, settings: Settings) {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(wisp_theme::fonts::GEIST_REGULAR),
                    Cow::Borrowed(wisp_theme::fonts::GEIST_SEMIBOLD),
                    Cow::Borrowed(wisp_theme::fonts::GEIST_MONO),
                ])
                .expect("bundled fonts");
            cx.bind_keys([
                KeyBinding::new("enter", Play, Some("Wisp")),
                KeyBinding::new("ctrl-o", Add, Some("Wisp")),
                KeyBinding::new("tab", Next, Some("Wisp")),
                KeyBinding::new("ctrl-1", ShowHome, Some("Wisp")),
                KeyBinding::new("ctrl-2", ShowSetup, Some("Wisp")),
            ]);
            SearchInput::init(cx);
            let mut viewport = size(px(1340.), px(754.));
            if let Some(display) = cx.primary_display() {
                let available = display.bounds().size;
                if available.width < viewport.width + px(24.) {
                    viewport.width = available.width - px(24.);
                }
                if available.height < viewport.height + px(40.) {
                    viewport.height = available.height - px(40.);
                }
            }
            let width = f32::from(viewport.width).min(f32::from(viewport.height) * 1672. / 941.);
            viewport = size(px(width), px(width * 941. / 1672.));
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None, viewport, cx,
                    ))),
                    window_min_size: Some(size(px(940.), px(530.))),
                    app_id: Some("wisp-launcher".into()),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Wisp Client".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Launcher::new(home, settings, window, cx)),
            )
            .expect("Wisp window");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
