//! The home screen's wardrobe (`wisp_client::wisp_home::wardrobe`): the skin library from the
//! `skins` folder beside `options.txt`, the skin tried on, wearing it (on the Microsoft account,
//! or offline for this game and the next), Show my cosmetics and the owned items.
use super::super::*;
use std::time::{Duration, Instant};
use wisp_client::wisp_home::{ItemRow, SkinChoice, WardrobeData};
use wisp_cosmetics::share::ShopAsk;

/// How long a skin's "saved" or error line stays.
const STATUS_FOR: Duration = Duration::from_secs(8);

/// A skin file of the library.
struct LibrarySkin {
    name: String,
    path: PathBuf,
    /// Its 64x64 RGBA as the game draws it.
    rgba: Vec<u8>,
    face: [u32; 64],
    /// The atlas sprite it is stored under while tried on.
    sprite: String,
}

/// What the wardrobe keeps between frames.
#[derive(Default)]
pub(in crate::game) struct WardrobeState {
    skins: Vec<LibrarySkin>,
    /// The folder's files and times when read, to read it again only when it changes.
    listing: Vec<(PathBuf, std::time::SystemTime)>,
    read: Option<Instant>,
    selected: Option<usize>,
    slim: bool,
    status: Option<(String, bool, Instant)>,
    upload: Option<std::sync::mpsc::Receiver<std::result::Result<(), String>>>,
    /// The offline skin chosen in an earlier game was put on once.
    restored: bool,
}

/// The face of a 64x64 skin as the wardrobe draws it: the head's front (8,8) with the hat's
/// front (40,8) over it, in the HUD's colour encoding.
fn face_of(rgba: &[u8]) -> [u32; 64] {
    let at = |x: usize, y: usize| {
        let i = (y * 64 + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    std::array::from_fn(|p| {
        let (x, y) = (p % 8, p / 8);
        let base = at(8 + x, 8 + y);
        let hat = at(40 + x, 8 + y);
        let a = f32::from(hat[3]) / 255.0;
        let mix =
            |c: usize| (f32::from(base[c]) * (1.0 - a) + f32::from(hat[c]) * a).round() as u32;
        0xFF00_0000 | mix(2) << 16 | mix(1) << 8 | mix(0)
    })
}

impl Game<'_> {
    fn skins_dir() -> PathBuf {
        Self::wardrobe_options_dir().join("skins")
    }

    fn wardrobe_options_dir() -> PathBuf {
        Self::options_path()
            .parent()
            .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf)
    }

    /// The library, read again when the folder changes (looked at every two seconds).
    fn refresh_skin_library(&mut self) {
        let w = &mut self.home.wardrobe;
        if w.read.is_some_and(|r| r.elapsed() < Duration::from_secs(2)) {
            return;
        }
        w.read = Some(Instant::now());
        let mut listing: Vec<(PathBuf, std::time::SystemTime)> =
            std::fs::read_dir(Self::skins_dir())
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
                })
                .map(|p| {
                    let time = std::fs::metadata(&p)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    (p, time)
                })
                .collect();
        listing.sort();
        if listing == w.listing {
            return;
        }
        let chosen = w
            .selected
            .and_then(|i| w.skins.get(i))
            .map(|s| s.path.clone());
        w.skins = listing
            .iter()
            .enumerate()
            .filter_map(|(i, (path, time))| {
                let (rgba, _) = wisp_client::skins::read_offline_skin(path, false).ok()?;
                let stamp = time
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                Some(LibrarySkin {
                    name: path.file_stem()?.to_string_lossy().into_owned(),
                    path: path.clone(),
                    face: face_of(&rgba),
                    rgba,
                    sprite: format!("wisp/wardrobe/{i}-{stamp}"),
                })
            })
            .collect();
        w.selected = chosen.and_then(|p| w.skins.iter().position(|s| s.path == p));
        w.listing = listing;
    }

    /// An offline skin picked in an earlier game (`wardrobeSkin` in `wisp.txt`), put on once.
    pub(in crate::game) fn restore_wardrobe_skin(&mut self) {
        if std::mem::replace(&mut self.home.wardrobe.restored, true) || self.auth().is_some() {
            return;
        }
        let dir = Self::wardrobe_options_dir();
        let Some(path) = wisp_client::ui_theme::preference(&dir, "wardrobeSkin") else {
            return;
        };
        let slim =
            wisp_client::ui_theme::preference(&dir, "wardrobeSlim").as_deref() == Some("true");
        if let Ok((rgba, slim)) =
            wisp_client::skins::read_offline_skin(std::path::Path::new(&path), slim)
        {
            wisp_client::skins::set_offline_skin(rgba, slim);
        }
    }

    /// The skin tried on in the wardrobe, stored in the atlas: its sprite and model.
    pub(in crate::game) fn wardrobe_preview(&mut self) -> Option<(String, bool)> {
        if self.home.screen.page != wisp_client::wisp_home::Page::Wardrobe {
            return None;
        }
        let w = &self.home.wardrobe;
        let skin = w.skins.get(w.selected?)?;
        let (sprite, rgba, slim) = (skin.sprite.clone(), skin.rgba.clone(), w.slim);
        if !self.skin_slots.contains(&sprite) || !self.gui.atlas.sprites.contains_key(&sprite) {
            self.store_skin(sprite.clone(), rgba);
        }
        self.gui
            .atlas
            .sprites
            .contains_key(&sprite)
            .then_some((sprite, slim))
    }

    /// What the wardrobe page shows this frame.
    pub(in crate::game) fn wardrobe_data(&mut self) -> WardrobeData {
        self.refresh_skin_library();
        let signed_in = self.auth().is_some();
        // A skin on its way to the account came back.
        let finished = self
            .home
            .wardrobe
            .upload
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());
        if let Some(done) = finished {
            let w = &mut self.home.wardrobe;
            w.upload = None;
            w.status = Some(match done {
                Ok(()) => (
                    wisp_client::lang::get_or("wisp.wardrobe.saved", "Skin saved to your account"),
                    false,
                    Instant::now(),
                ),
                Err(e) => (
                    format!(
                        "{} {e}",
                        wisp_client::lang::get_or(
                            "wisp.wardrobe.failed",
                            "Could not save the skin:"
                        )
                    ),
                    true,
                    Instant::now(),
                ),
            });
            // The account's new skin: asked for again, downloaded, worn by the figure.
            self.home.reset_own_skin();
        }
        let w = &mut self.home.wardrobe;
        if w.status
            .as_ref()
            .is_some_and(|(_, _, at)| at.elapsed() > STATUS_FOR)
        {
            w.status = None;
        }
        let shop = self.cosmetics.sharing.as_ref().map(|s| s.shop_state());
        let wallet = shop.as_ref().and_then(|s| s.wallet.clone());
        let items_problem = match (&shop, &wallet) {
            (None, _) => Some(wisp_client::lang::get_or(
                "wisp.wardrobe.no_server",
                "Your items show once the Wisp server is reachable.",
            )),
            (Some(s), None) => Some(s.problem.clone().unwrap_or_else(|| {
                wisp_client::lang::get_or("wisp.wardrobe.loading", "Loading your items...")
            })),
            _ => None,
        };
        let items = wallet
            .map(|wallet| {
                wallet
                    .owned
                    .iter()
                    .filter_map(|id| wisp_shop::OFFERS.iter().find(|o| o.id == id))
                    .filter(|o| o.kind != wisp_shop::Kind::Emote)
                    .map(|o| ItemRow {
                        id: o.id.to_owned(),
                        name: o.name.to_owned(),
                        kind: wisp_client::wisp_menu::store_kind_name(o.kind),
                        worn: wallet.equipped.iter().any(|e| e == o.id),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let w = &self.home.wardrobe;
        WardrobeData {
            skins: w
                .skins
                .iter()
                .map(|s| SkinChoice {
                    name: s.name.clone(),
                    face: s.face,
                })
                .collect(),
            selected: w.selected,
            slim: w.slim,
            signed_in,
            status: w.status.as_ref().map(|(t, bad, _)| (t.clone(), *bad)),
            busy: w.upload.is_some(),
            cosmetics: wisp_cosmetics::enabled(),
            cosmetics_shown: self.settings.extras.cosmetics_mine,
            items,
            items_problem,
        }
    }

    /// A click on the wardrobe page; true when it was one.
    pub(in crate::game) fn wardrobe_click(
        &mut self,
        click: &wisp_client::wisp_home::Click,
    ) -> bool {
        use wisp_client::wisp_home::{Click, Page};
        match click {
            Click::Wardrobe => {
                self.home.screen.page = Page::Wardrobe;
                self.home.screen.reset();
                self.home.wardrobe.read = None;
            }
            Click::Back => {
                self.home.screen.page = Page::Main;
                self.home.screen.reset();
                self.home.wardrobe.selected = None;
            }
            Click::Tab(tab) => self.home.screen.tab = *tab,
            Click::Skin(i) => self.home.wardrobe.selected = Some(*i),
            Click::Model(slim) => self.home.wardrobe.slim = *slim,
            Click::SkinFolder => {
                let dir = Self::skins_dir();
                let _ = std::fs::create_dir_all(&dir);
                super::super::packs::open_folder(&dir);
            }
            Click::ApplySkin => self.wear_wardrobe_skin(),
            Click::ShowCosmetics => {
                self.settings.extras.cosmetics_mine ^= true;
                self.settings.save(&Self::options_path());
                self.apply_settings();
            }
            Click::Item(id) => {
                let shop = self.cosmetics.sharing.as_ref().map(|s| s.shop_state());
                let worn = shop
                    .and_then(|s| s.wallet)
                    .is_some_and(|w| w.equipped.iter().any(|e| e == id));
                self.shop_ask(if worn {
                    ShopAsk::Unequip(id.clone())
                } else {
                    ShopAsk::Equip(id.clone())
                });
            }
            _ => return false,
        }
        true
    }

    /// Wear the skin tried on: sent to the Microsoft account, or offline worn in this game and
    /// remembered for the next.
    fn wear_wardrobe_skin(&mut self) {
        let w = &self.home.wardrobe;
        let Some(skin) = w.selected.and_then(|i| w.skins.get(i)) else {
            return;
        };
        let (path, rgba, slim) = (skin.path.clone(), skin.rgba.clone(), w.slim);
        match self.auth() {
            Some(auth) => {
                let png = match std::fs::read(&path) {
                    Ok(png) => png,
                    Err(e) => {
                        self.home.wardrobe.status = Some((e.to_string(), true, Instant::now()));
                        return;
                    }
                };
                let (send, recv) = std::sync::mpsc::channel();
                std::thread::Builder::new()
                    .name("Wardrobe skin".into())
                    .spawn(move || {
                        let _ =
                            send.send(wisp_client::auth::upload_skin(auth.as_ref(), &png, slim));
                    })
                    .ok();
                self.home.wardrobe.upload = Some(recv);
                self.home.wardrobe.status = None;
            }
            None => {
                wisp_client::skins::set_offline_skin(rgba, slim);
                let dir = Self::wardrobe_options_dir();
                wisp_client::ui_theme::set_preference(
                    &dir,
                    "wardrobeSkin",
                    Some(&path.to_string_lossy()),
                );
                wisp_client::ui_theme::set_preference(
                    &dir,
                    "wardrobeSlim",
                    Some(if slim { "true" } else { "false" }),
                );
                self.home.wardrobe.status = Some((
                    wisp_client::lang::get_or("wisp.wardrobe.worn", "You wear this skin now"),
                    false,
                    Instant::now(),
                ));
            }
        }
    }
}
