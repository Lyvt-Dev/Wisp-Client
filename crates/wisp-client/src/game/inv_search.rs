//! Inventory Search (the `inventory_search` client module, `docs/extras.md`): a search box over
//! the inventory and container screens. While it holds text, every slot whose item doesn't match
//! (by its shown name or its id) is dimmed. Client-only: no click is sent.
use super::{Game, Screen};
use wisp_client::{
    hud::{HudBatch, rgba},
    items::ItemStack,
    ui::EditBox,
};

/// The box's size in GUI pixels, and its gap above the screen's panel.
const WIDTH: f32 = 100.0;
const HEIGHT: f32 = 12.0;
const GAP: f32 = 2.0;
/// The shade over a slot that doesn't match.
const DIM: u32 = 0xC000_0000;

/// Whether `stack` matches `query` (already lowercased): its shown name or its id, without the
/// `minecraft:` namespace, contains it.
pub(super) fn matches(name: &str, id: &str, query: &str) -> bool {
    query.is_empty()
        || name.to_lowercase().contains(query)
        || id
            .trim_start_matches("minecraft:")
            .replace('_', " ")
            .contains(query)
        || id.contains(query)
}

/// Darkens drawn vertices to a quarter of their colour, keeping their alpha.
pub(super) fn darken(vertices: &mut [wisp_client::hud::HudVertex]) {
    for v in vertices {
        let [a, b, g, r] = v.color.to_be_bytes();
        let quarter = |c: u8| c / 4;
        v.color = u32::from_be_bytes([a, quarter(b), quarter(g), quarter(r)]);
    }
}

impl Game<'_> {
    /// The box shows on the inventory and the container screens drawn with slots, not on the
    /// workstation screens that draw themselves.
    fn inv_search_shown(&self) -> bool {
        self.settings.extras.module("inventory_search")
            && self.screen == Screen::Inventory
            && self.station.is_none()
            && !self.mount_active()
    }
    /// Whether the box takes the keyboard.
    pub(super) fn inv_search_focused(&self) -> bool {
        self.inv_search_shown() && self.extras.inv_search.as_ref().is_some_and(|b| b.focused)
    }
    /// The query, lowercased (empty when there is none).
    fn inv_search_query(&self) -> String {
        if !self.inv_search_shown() {
            return String::new();
        }
        self.extras
            .inv_search
            .as_ref()
            .map_or_else(String::new, |b| b.value.trim().to_lowercase())
    }
    /// The query while one is typed (for the slots' items).
    pub(super) fn inv_search_filter(&self) -> Option<String> {
        Some(self.inv_search_query()).filter(|q| !q.is_empty())
    }
    pub(super) fn stack_matches_search(&self, stack: &ItemStack, query: &str) -> bool {
        matches(&self.stack_hover_name(stack), &stack.item, query)
    }
    /// Draws the box above the panel at `(ox, oy)` and dims the slots that don't match.
    pub(super) fn draw_inventory_search(&mut self, batch: &mut HudBatch, ox: f32, oy: f32) {
        if !self.inv_search_shown() {
            return;
        }
        let query = self.inv_search_query();
        if !query.is_empty() {
            for i in self.slots() {
                let shown = self
                    .slot_view(i)
                    .is_some_and(|stack| self.stack_matches_search(&stack, &query));
                if !shown {
                    let (sx, sy) = self.slot_position(i);
                    batch.fill(&self.gui.atlas, ox + sx, oy + sy, 16.0, 16.0, DIM);
                }
            }
        }
        let placeholder = wisp_client::lang::get_or("wisp.inventory_search.hint", "Search...");
        let atlas = &self.gui.atlas;
        let search = self.extras.inv_search.get_or_insert_with(|| {
            let mut b = EditBox::new(0.0, 0.0, WIDTH, HEIGHT);
            b.focused = false;
            b.max_length = 50;
            b
        });
        search.x = ox;
        search.y = oy - HEIGHT - GAP;
        search.message = placeholder.clone();
        search.draw(batch, atlas, None);
        if search.value.is_empty() && !search.focused {
            batch.text(
                atlas,
                &placeholder,
                search.x + 4.0,
                search.y + (HEIGHT - 8.0) / 2.0,
                rgba(112, 112, 112, 255),
                false,
            );
        }
    }
    /// A left press on the box focuses it and places the cursor (true: taken); one elsewhere
    /// lets go of the keyboard and goes on to the slots.
    pub(super) fn inv_search_press(&mut self) -> bool {
        if !self.inv_search_shown() {
            return false;
        }
        let (cx, cy) = self.cursor;
        let shift = self.has_shift_down();
        let atlas = &self.gui.atlas;
        let Some(search) = self.extras.inv_search.as_mut() else {
            return false;
        };
        let inside = cx >= search.x
            && cx < search.x + search.width
            && cy >= search.y
            && cy < search.y + search.height;
        if inside {
            search.focused = true;
            search.reset_blink();
            search.click(atlas, cx, shift, false);
        } else {
            search.focused = false;
        }
        inside
    }
    /// A key while the box has the keyboard: editing keys, shortcuts and typed text. Escape
    /// leaves the box first.
    pub(super) fn inv_search_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if let Some(shortcut) = self.edit_shortcut(event) {
            if let Some(b) = self.extras.inv_search.as_mut() {
                b.shortcut(shortcut, &mut self.clipboard);
            }
            return;
        }
        let (ctrl, shift) = (self.has_control_down(), self.has_shift_down());
        let types_text = self.types_text();
        let Some(search) = self.extras.inv_search.as_mut() else {
            return;
        };
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter) => {
                search.focused = false;
            }
            PhysicalKey::Code(code) if search.key(code, ctrl, shift) => {}
            _ => {
                if types_text && let Some(text) = &event.text {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    search.insert_text(&text);
                }
            }
        }
    }
    /// Text from the input method (or an agent's `type`) while the box has the keyboard.
    pub(super) fn inv_search_text(&mut self, text: &str) {
        if let Some(search) = self.extras.inv_search.as_mut() {
            search.insert_text(text);
        }
    }
    /// The query is forgotten when the screen closes.
    pub(super) fn inv_search_frame(&mut self) {
        if self.screen != Screen::Inventory {
            self.extras.inv_search = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn a_query_matches_the_shown_name_or_the_id() {
        assert!(matches("Diamond Sword", "minecraft:diamond_sword", "sword"));
        assert!(matches(
            "Diamond Sword",
            "minecraft:diamond_sword",
            "diamond s"
        ));
        // A renamed item still matches its id.
        assert!(matches("Excalibur", "minecraft:diamond_sword", "diamond"));
        assert!(matches("Excalibur", "minecraft:diamond_sword", "excal"));
        assert!(!matches("Apple", "minecraft:apple", "sword"));
        assert!(matches("Apple", "minecraft:apple", ""));
    }
}
