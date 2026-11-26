//! The Wisp Menu in the game (`docs/extras.md`): Right Shift opens it over the world; it is
//! drawn by `wisp_client::wisp_menu`, and here it gets the settings, its item icons, the
//! mouse and keyboard, and saves what changes.
use super::{Game, Screen};
use std::sync::Arc;
use winit::keyboard::{KeyCode, PhysicalKey};
use wisp_client::{
    extras::features::{self, Kind},
    hud::HudBatch,
    items::ItemStack,
    ui_theme::Ui,
    wisp_menu::{Action, Control, FriendsView, StoreClick, StoreField, StorePage},
};
use wisp_cosmetics::share::ShopAsk;

impl Game<'_> {
    /// Right Shift in a world (or the Extras page's button): the menu opens on the page it was
    /// left on.
    pub(super) fn open_wisp_menu(&mut self) {
        if self.title.is_some() || self.player.is_none() {
            return;
        }
        // Not over the terrain loading: the menu would pause single player before it is ready.
        if self
            .remote
            .as_ref()
            .is_some_and(|r| r.level_load.is_active() || r.reconfiguring.is_some())
        {
            return;
        }
        let m = &mut self.extras.menu;
        m.search_focused = false;
        m.friend_focused = false;
        m.confirm_remove = None;
        m.store_focus = None;
        m.store_confirm = None;
        m.waiting_key = None;
        m.dragging = None;
        m.text_edit = None;
        if m.store_page.is_some() {
            self.shop_ask(ShopAsk::Refresh);
        }
        let m = &mut self.extras.menu;
        // Back from the HUD editor it is already open: no slide in again.
        if self.screen != Screen::HudEditor {
            m.opened = Some(std::time::Instant::now());
        }
        self.screen = Screen::WispMenu;
        // The Friends tab shows the list as it is now, not as it was 20 seconds ago.
        if m.friends_page
            && let Some(sharing) = &self.cosmetics.sharing
        {
            sharing.friend(wisp_cosmetics::share::FriendAsk::Refresh);
        }
        self.capture(false);
    }
    /// The Store key: the Wisp Menu, opened on the Store.
    pub(super) fn open_store(&mut self) {
        let m = &mut self.extras.menu;
        m.store_page = Some(StorePage::Shop);
        m.friends_page = false;
        m.open = None;
        m.scroll = 0.0;
        self.open_wisp_menu();
    }
    /// Hands the Shop's work to the Wisp server's thread (nothing without a server).
    pub(super) fn shop_ask(&self, ask: ShopAsk) {
        if let Some(sharing) = &self.cosmetics.sharing {
            sharing.shop(ask);
        }
    }
    pub(super) fn close_wisp_menu(&mut self) {
        self.save_menu_settings();
        let m = &mut self.extras.menu;
        m.search_focused = false;
        m.friend_focused = false;
        m.store_focus = None;
        m.store_confirm = None;
        m.confirm_remove = None;
        m.waiting_key = None;
        m.dragging = None;
        m.text_edit = None;
        self.screen = Screen::Playing;
        self.capture(true);
    }
    fn save_menu_settings(&mut self) {
        self.settings.save(&Self::options_path());
        self.apply_settings();
    }
    pub(super) fn draw_wisp_menu(&mut self, batch: &mut HudBatch) {
        let cursor = self.cursor;
        let state = self.friends_state();
        let server = self.friends_server();
        let shop = self.cosmetics.sharing.as_ref().map(|s| s.shop_state());
        let player = (self.options.player_name.clone())
            .unwrap_or_else(|| self.acting_player_name().to_owned());
        let icons = {
            let atlas = &self.gui.atlas;
            let mut ui = Ui::new(atlas, batch, cursor);
            let key_name = |name: &str| wisp_client::keybinds::display_name(name).plain();
            let friends = FriendsView {
                state: state.as_ref(),
                server: server.as_deref(),
            };
            let icons = self.extras.menu.draw(
                &mut ui,
                &self.settings.extras,
                &key_name,
                friends,
                shop.as_ref(),
                &player,
            );
            ui.finish();
            icons
        };
        for icon in icons {
            let stack = ItemStack {
                item: Arc::from(icon.item),
                count: 1,
                damage: 0,
                extra: None,
            };
            let start = batch.vertices.len();
            self.draw_item_model(batch, &stack, icon.x, icon.y);
            batch.scale_since(start, icon.x, icon.y, icon.size / 16.0, icon.size / 16.0);
            let [cx, cy, cw, ch] = icon.clip;
            batch.clip_since(start, cx, cy, cx + cw, cy + ch);
        }
        if let (Some(area), Some(key)) = (self.extras.menu.preview, self.extras.menu.open) {
            self.draw_widget_preview(batch, key, area);
        }
    }
    /// A left press in the menu.
    pub(super) fn wisp_menu_press(&mut self) {
        let Some(action) = self.extras.menu.click(self.cursor) else {
            return;
        };
        let m = &mut self.extras.menu;
        if action != Action::Search {
            m.search_focused = false;
        }
        if action != Action::FriendField && action != Action::FriendAdd {
            m.friend_focused = false;
        }
        if !matches!(
            action,
            Action::Store(StoreClick::Field(_) | StoreClick::Submit(_))
        ) {
            m.store_focus = None;
        }
        if !matches!(action, Action::Store(StoreClick::Remove(_))) {
            m.store_confirm = None;
        }
        if !matches!(
            action,
            Action::Friend(wisp_client::wisp_menu::FriendClick::Remove, _)
        ) {
            m.confirm_remove = None;
        }
        if !matches!(action, Action::Setting(_, _, Control::Key)) {
            m.waiting_key = None;
        }
        let typing = matches!(action, Action::Setting(_, _, Control::Text | Control::Hex));
        if !typing && self.extras.menu.text_edit.is_some() {
            self.finish_text_edit(true);
        }
        let click = !matches!(action, Action::Close | Action::Nothing);
        if click {
            self.audio.play_button_click();
        }
        let e = &mut self.settings.extras;
        match action {
            Action::Category(c) => {
                let m = &mut self.extras.menu;
                m.category = c;
                m.open = None;
                m.scroll = 0.0;
                m.friends_page = false;
                m.store_page = None;
            }
            Action::Store(click) => self.store_click(click),
            Action::Friends => {
                let m = &mut self.extras.menu;
                m.friends_page = true;
                m.store_page = None;
                m.scroll = 0.0;
                if let Some(sharing) = &self.cosmetics.sharing {
                    sharing.friend(wisp_cosmetics::share::FriendAsk::Refresh);
                }
            }
            Action::FriendField => self.extras.menu.friend_focused = true,
            Action::FriendAdd => self.friends_click(None),
            Action::Friend(click, i) => self.friends_click(Some((click, i))),
            Action::Toggle(key) => {
                let on = e.feature_on(key);
                e.set_feature(key, !on);
                self.save_menu_settings();
            }
            Action::Open(key) => {
                let m = &mut self.extras.menu;
                m.open = Some(key);
                m.scroll = 0.0;
            }
            Action::Back => {
                let m = &mut self.extras.menu;
                m.open = None;
                m.scroll = 0.0;
            }
            Action::Search => self.extras.menu.search_focused = true,
            Action::EditHud => {
                self.save_menu_settings();
                self.extras.editor_from_menu = true;
                self.open_hud_editor();
            }
            Action::Reset(key) => {
                e.reset_feature(key);
                self.save_menu_settings();
            }
            Action::Setting(f, s, control) => {
                match control {
                    Control::Toggle => {
                        let v = !e.opt_bool(f, s);
                        e.set_opt(f, s, v.to_string());
                    }
                    Control::Choice(i) => {
                        if let Some(Kind::Choice(names, _)) = setting_kind(f, s)
                            && let Some(name) = names.get(i)
                        {
                            e.set_opt(f, s, (*name).to_owned());
                        }
                    }
                    Control::Color(c) => e.set_opt(f, s, format!("{c:06x}")),
                    Control::Slider { x0, x1 } => {
                        self.extras.menu.dragging = Some((f, s, x0, x1));
                        self.wisp_menu_drag();
                        return;
                    }
                    Control::Key => {
                        self.extras.menu.waiting_key = Some((f, s));
                        return;
                    }
                    Control::Text | Control::Hex => {
                        if self.extras.menu.text_edit == Some((f, s)) {
                            return;
                        }
                        if self.extras.menu.text_edit.is_some() {
                            self.finish_text_edit(true);
                        }
                        let e = &self.settings.extras;
                        // A hex code is typed fresh (the chip shows the colour until then); a
                        // text goes on from what it says.
                        let now = if control == Control::Hex {
                            String::new()
                        } else {
                            e.opt_text(f, s)
                        };
                        let m = &mut self.extras.menu;
                        m.text_edit = Some((f, s));
                        m.text_input = now;
                        return;
                    }
                    Control::Hue { x0, x1 } => {
                        self.extras.menu.dragging = Some((f, s, x0, x1));
                        self.wisp_menu_drag();
                        return;
                    }
                }
                self.save_menu_settings();
            }
            Action::Close => self.close_wisp_menu(),
            Action::Nothing => {}
        }
    }
    /// A click on the Store, Creator or Admin page. Every purchase is priced and checked by the
    /// Wisp server; the page shows its answer.
    fn store_click(&mut self, click: StoreClick) {
        let shop = self.cosmetics.sharing.as_ref().map(|s| s.shop_state());
        let m = &mut self.extras.menu;
        match click {
            StoreClick::Page(page) => {
                m.store_page = Some(page);
                m.friends_page = false;
                m.open = None;
                m.scroll = 0.0;
                m.store_input.clear();
                self.shop_ask(ShopAsk::Refresh);
            }
            StoreClick::Kind(kind) => {
                m.store_kind = kind;
                m.scroll = 0.0;
            }
            StoreClick::Item(id) => {
                let wallet = shop.and_then(|s| s.wallet).unwrap_or_default();
                let ask = if wallet.equipped.iter().any(|e| e == id) {
                    ShopAsk::Unequip(id.to_owned())
                } else if wallet.owned.iter().any(|o| o == id) {
                    ShopAsk::Equip(id.to_owned())
                } else {
                    ShopAsk::Buy(id.to_owned())
                };
                self.shop_ask(ask);
            }
            StoreClick::Field(field) => {
                if m.store_focus != Some(field) {
                    m.store_input.clear();
                }
                m.store_focus = Some(field);
            }
            StoreClick::Submit(field) => self.store_submit(field),
            StoreClick::ClearSupport => self.shop_ask(ShopAsk::Support(None)),
            StoreClick::Remove(i) => {
                if m.store_confirm == Some(i) {
                    m.store_confirm = None;
                    if let Some(uuid) = m.store_creator(i).map(str::to_owned) {
                        self.shop_ask(ShopAsk::AdminRemove(uuid));
                    }
                } else {
                    m.store_confirm = Some(i);
                }
            }
        }
    }
    /// The field's button (or Enter): what is typed goes to the server.
    fn store_submit(&mut self, field: StoreField) {
        let m = &mut self.extras.menu;
        let text = m.store_input.trim().to_owned();
        if m.store_focus != Some(field) || text.is_empty() {
            m.store_focus = Some(field);
            return;
        }
        m.store_input.clear();
        m.store_focus = None;
        self.shop_ask(match field {
            StoreField::Support => ShopAsk::Support(Some(text)),
            StoreField::Code => ShopAsk::SetCode(text),
            StoreField::AdminName => ShopAsk::AdminAdd(text),
        });
    }
    /// The cursor moved: a held slider follows it.
    pub(super) fn wisp_menu_drag(&mut self) {
        let Some((f, s, x0, x1)) = self.extras.menu.dragging else {
            return;
        };
        let t = ((self.cursor.0 - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
        match setting_kind(f, s) {
            Some(Kind::Slider { min, max, step, .. }) => {
                let v = features::snap(min + t * (max - min), min, max, step);
                self.settings.extras.set_opt(f, s, format!("{v}"));
            }
            // A colour's hue strip.
            Some(Kind::Color(_)) => {
                let c = wisp_client::hud_widgets::hsv(t, 0.75, 1.0);
                self.settings.extras.set_opt(f, s, format!("{c:06x}"));
            }
            _ => {}
        }
    }
    pub(super) fn wisp_menu_release(&mut self) {
        if self.extras.menu.dragging.take().is_some() {
            self.save_menu_settings();
        }
    }
    pub(super) fn wisp_menu_scroll(&mut self, delta: f32) {
        self.extras.menu.wheel(delta);
    }
    /// A key in the menu: binding a key setting, typing a search, or closing.
    pub(super) fn wisp_menu_key(&mut self, event: &winit::event::KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        self.wisp_menu_key_code(code, event.text.as_deref());
    }
    /// A key in the menu by its code and the text it types (a person's keyboard or an agent's
    /// `key`).
    pub(super) fn wisp_menu_key_code(&mut self, code: KeyCode, typed: Option<&str>) {
        if let Some((f, s)) = self.extras.menu.waiting_key.take() {
            if code != KeyCode::Escape
                && let Some(name) = wisp_client::keybinds::key_name(code)
            {
                self.settings.extras.set_opt(f, s, name.to_owned());
                self.save_menu_settings();
            }
            return;
        }
        if self.extras.menu.text_edit.is_some() {
            match code {
                KeyCode::Escape => self.finish_text_edit(false),
                KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => self.finish_text_edit(true),
                KeyCode::Backspace => {
                    self.extras.menu.text_input.pop();
                }
                _ => {
                    if let Some(text) = typed {
                        self.wisp_menu_text(text);
                    }
                }
            }
            return;
        }
        let m = &mut self.extras.menu;
        if let Some(field) = m.store_focus {
            match code {
                KeyCode::Escape => m.store_focus = None,
                KeyCode::Enter | KeyCode::NumpadEnter => self.store_submit(field),
                KeyCode::Backspace => {
                    m.store_input.pop();
                }
                _ => {
                    if let Some(text) = typed {
                        self.wisp_menu_text(text);
                    }
                }
            }
            return;
        }
        if m.friend_focused {
            match code {
                KeyCode::Escape => m.friend_focused = false,
                KeyCode::Enter | KeyCode::NumpadEnter => self.friends_click(None),
                KeyCode::Backspace => {
                    m.friend_input.pop();
                }
                _ => {
                    if let Some(text) = typed {
                        self.wisp_menu_text(text);
                    }
                }
            }
            return;
        }
        if m.search_focused {
            match code {
                KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter => m.search_focused = false,
                KeyCode::Backspace => {
                    m.search.pop();
                    m.scroll = 0.0;
                }
                _ => {
                    if let Some(text) = typed {
                        self.wisp_menu_text(text);
                    }
                }
            }
            return;
        }
        match code {
            KeyCode::Escape | KeyCode::ShiftRight => {
                if self.extras.menu.open.take().is_some()
                    && code == KeyCode::Escape
                    && !self.extras.menu.friends_page
                {
                    self.extras.menu.scroll = 0.0;
                } else {
                    self.close_wisp_menu();
                }
            }
            _ => {
                // Typing anywhere starts a search, as the launcher's lists do.
                if let Some(text) = typed
                    && text.chars().any(|c| c.is_alphanumeric())
                {
                    let m = &mut self.extras.menu;
                    if m.friends_page {
                        // On Friends, typing goes to the add field.
                        m.friend_focused = true;
                        self.wisp_menu_text(text);
                    } else if m.open.is_none() && m.store_page.is_none() {
                        m.search_focused = true;
                        self.wisp_menu_text(text);
                    }
                }
            }
        }
    }
    /// Text typed into the search (keys, the input method or an agent's `type`).
    pub(super) fn wisp_menu_text(&mut self, text: &str) {
        let m = &mut self.extras.menu;
        if let Some((f, s)) = m.text_edit {
            let hex = matches!(setting_kind(f, s), Some(Kind::Color(_)));
            for c in text.chars().filter(|c| !c.is_control()) {
                let len = m.text_input.chars().count();
                if hex {
                    if c.is_ascii_hexdigit() && len < 6 {
                        m.text_input.push(c.to_ascii_uppercase());
                    }
                } else if len < features::TEXT_MAX {
                    m.text_input.push(c);
                }
            }
            return;
        }
        if m.store_focus.is_some() {
            // Creator codes and Minecraft names alike: letters, digits and _, at most 16.
            for c in text
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                if m.store_input.len() < 16 {
                    m.store_input.push(c);
                }
            }
            return;
        }
        if m.friend_focused {
            // Minecraft names: letters, digits and _, at most 16.
            for c in text
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                if m.friend_input.len() < 16 {
                    m.friend_input.push(c);
                }
            }
            return;
        }
        if !m.search_focused {
            return;
        }
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if m.search.chars().count() + clean.chars().count() <= 40 {
            m.search.push_str(&clean);
            m.scroll = 0.0;
        }
    }
}

impl Game<'_> {
    /// Ends typing a text or hex setting: `keep` saves what was typed (a hex code only when it
    /// is one: 3 or 6 digits), else it is dropped.
    fn finish_text_edit(&mut self, keep: bool) {
        let m = &mut self.extras.menu;
        let Some((f, s)) = m.text_edit.take() else {
            return;
        };
        let typed = std::mem::take(&mut m.text_input);
        if !keep {
            return;
        }
        let value = if matches!(setting_kind(f, s), Some(Kind::Color(_))) {
            match parse_hex(&typed) {
                Some(c) => format!("{c:06x}"),
                None => return,
            }
        } else {
            typed.trim().to_owned()
        };
        self.settings.extras.set_opt(f, s, value);
        self.save_menu_settings();
    }
}

/// A colour typed as a hex code: `RRGGBB`, or `RGB` for `RRGGBB` with each digit doubled.
fn parse_hex(text: &str) -> Option<u32> {
    let t = text.trim().trim_start_matches('#');
    match t.len() {
        6 => u32::from_str_radix(t, 16).ok(),
        3 => {
            let v = u32::from_str_radix(t, 16).ok()?;
            let [r, g, b] = [v >> 8 & 0xf, v >> 4 & 0xf, v & 0xf];
            Some((r * 17) << 16 | (g * 17) << 8 | (b * 17))
        }
        _ => None,
    }
}

fn setting_kind(feature: &str, setting: &str) -> Option<Kind> {
    let f = features::feature(feature)?;
    f.settings
        .iter()
        .chain(features::widget_settings().filter(|_| f.widget))
        .find(|s| s.key == setting)
        .map(|s| s.kind)
}

#[cfg(test)]
mod tests {
    use super::parse_hex;

    #[test]
    fn hex_codes_long_and_short() {
        assert_eq!(parse_hex("FF8800"), Some(0xFF8800));
        assert_eq!(parse_hex("#ff8800"), Some(0xFF8800));
        assert_eq!(parse_hex("F80"), Some(0xFF8800));
        assert_eq!(parse_hex("FF88"), None);
        assert_eq!(parse_hex("GG0000"), None);
    }
}
