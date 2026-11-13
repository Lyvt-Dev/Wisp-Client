//! Focusable native search input, including IME/UTF-16 selection and clipboard actions.
use super::*;
use gpui::{
    canvas, ClipboardItem, ElementInputHandler, EntityInputHandler, EventEmitter, Focusable,
    Pixels, Point, UTF16Selection,
};
use std::ops::Range;

actions!(
    wisp_catalog_search,
    [EndSearch, SelectAll, Paste, Copy, Cut]
);
pub(super) struct Changed;
pub(super) struct SearchInput {
    pub value: String,
    placeholder: String,
    focus: FocusHandle,
    owner: FocusHandle,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
}
impl EventEmitter<Changed> for SearchInput {}
impl Focusable for SearchInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
fn byte_at(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units + ch.len_utf16() > unit {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}
fn replace(text: &mut String, range: Range<usize>, value: &str) -> usize {
    let a = byte_at(text, range.start);
    let b = byte_at(text, range.end.max(range.start));
    let value: String = value
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect();
    let start = text[..a].encode_utf16().count();
    text.replace_range(a..b, &value);
    start + value.encode_utf16().count()
}
impl SearchInput {
    pub fn init(cx: &mut App) {
        cx.bind_keys([
            KeyBinding::new("enter", EndSearch, Some("WispSearch")),
            KeyBinding::new("escape", EndSearch, Some("WispSearch")),
            KeyBinding::new("ctrl-a", SelectAll, Some("WispSearch")),
            KeyBinding::new("cmd-a", SelectAll, Some("WispSearch")),
            KeyBinding::new("ctrl-v", Paste, Some("WispSearch")),
            KeyBinding::new("cmd-v", Paste, Some("WispSearch")),
            KeyBinding::new("ctrl-c", Copy, Some("WispSearch")),
            KeyBinding::new("cmd-c", Copy, Some("WispSearch")),
            KeyBinding::new("ctrl-x", Cut, Some("WispSearch")),
            KeyBinding::new("cmd-x", Cut, Some("WispSearch")),
        ]);
    }
    pub fn new(placeholder: &str, owner: FocusHandle, cx: &mut Context<Self>) -> Self {
        Self {
            value: String::new(),
            placeholder: tr(placeholder),
            focus: cx.focus_handle(),
            owner,
            selection: 0..0,
            marked: None,
        }
    }
    /// Replaces the text (the launcher filled it in, or emptied it after use).
    pub fn set(&mut self, value: &str, cx: &mut Context<Self>) {
        self.value = value
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect();
        let end = self.value.encode_utf16().count();
        self.selection = end..end;
        self.marked = None;
        cx.emit(Changed);
        cx.notify();
    }
    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus);
        window.notify_text_selection_changed(&self.focus);
        cx.emit(Changed);
        cx.notify();
    }
    fn selected(&self) -> String {
        self.value
            [byte_at(&self.value, self.selection.start)..byte_at(&self.value, self.selection.end)]
            .to_string()
    }
    fn key(&mut self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let length = self.value.encode_utf16().count();
        let byte = byte_at(&self.value, self.selection.end);
        let before = self.value[..byte]
            .chars()
            .next_back()
            .map_or(0, |c| c.len_utf16());
        let after = self.value[byte..]
            .chars()
            .next()
            .map_or(0, |c| c.len_utf16());
        match event.keystroke.key.as_str() {
            "backspace" => {
                let range = if self.selection.is_empty() {
                    self.selection.start.saturating_sub(before)..self.selection.end
                } else {
                    self.selection.clone()
                };
                self.replace_text_in_range(Some(range), "", window, cx);
            }
            "delete" => {
                let range = if self.selection.is_empty() {
                    self.selection.start..(self.selection.end + after).min(length)
                } else {
                    self.selection.clone()
                };
                self.replace_text_in_range(Some(range), "", window, cx);
            }
            "left" => {
                let pos = if self.selection.is_empty() {
                    self.selection.end.saturating_sub(before)
                } else {
                    self.selection.start
                };
                self.selection = pos..pos;
                self.changed(window, cx);
            }
            "right" => {
                let pos = if self.selection.is_empty() {
                    (self.selection.end + after).min(length)
                } else {
                    self.selection.end
                };
                self.selection = pos..pos;
                self.changed(window, cx);
            }
            "home" => {
                self.selection = 0..0;
                self.changed(window, cx);
            }
            "end" => {
                self.selection = length..length;
                self.changed(window, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}
impl Render for SearchInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = (f32::from(window.viewport_size().width) / 1672.)
            .min(f32::from(window.viewport_size().height) / 941.)
            .clamp(0.5, 1.4);
        let focused = self.focus.is_focused(window);
        let view = cx.entity();
        let focus = self.focus.clone();
        let a = byte_at(&self.value, self.selection.start);
        let b = byte_at(&self.value, self.selection.end);
        div()
            .id("catalog-search-input")
            .relative()
            .h(px(46. * s))
            .w_full()
            .min_w_0()
            .px(px(14. * s))
            .rounded(px(13. * s))
            .bg(rgb(tone(0x202124)))
            .border_1()
            .border_color(rgb(if focused {
                accent_tone(ACCENT)
            } else {
                tone(RULE)
            }))
            .flex()
            .items_center()
            .gap(px(11. * s))
            .cursor_text()
            .track_focus(&self.focus)
            .key_context("WispSearch")
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    let n = this.value.encode_utf16().count();
                    this.selection = n..n;
                    this.changed(window, cx);
                }),
            )
            .on_key_down(cx.listener(Self::key))
            .on_action(cx.listener(|this, _: &EndSearch, window, cx| {
                window.focus(&this.owner, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.selection = 0..this.value.encode_utf16().count();
                this.changed(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| {
                if let Some(item) = cx.read_from_clipboard() {
                    EntityInputHandler::paste(this, item, window, cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(this.selected()));
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &Cut, window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(this.selected()));
                this.replace_text_in_range(None, "", window, cx);
                cx.stop_propagation();
            }))
            .child(icon("search-01", MUTED).size(px(21. * s)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .text_size(px(15. * s))
                    .when(self.value.is_empty(), |d| {
                        d.child(div().text_color(rgb(MUTED)).child(self.placeholder.clone()))
                    })
                    .when(!self.value.is_empty(), |d| {
                        d.child(div().flex_none().child(self.value[..a].to_string()))
                            .when(a != b, |d| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .bg(rgb(tone(0x555862)))
                                        .child(self.value[a..b].to_string()),
                                )
                            })
                            .when(focused, |d| {
                                d.child(
                                    div()
                                        .w(px(1.))
                                        .h(px(19. * s))
                                        .flex_none()
                                        .bg(rgb(accent_tone(ACCENT))),
                                )
                            })
                            .child(div().flex_none().child(self.value[b..].to_string()))
                    }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx)
                    },
                )
                .absolute()
                .left_0()
                .top_0()
                .size_full(),
            )
    }
}
impl EntityInputHandler for SearchInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let a = byte_at(&self.value, range.start);
        let b = byte_at(&self.value, range.end.max(range.start));
        *adjusted =
            Some(self.value[..a].encode_utf16().count()..self.value[..b].encode_utf16().count());
        Some(self.value[a..b].to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        self.changed(window, cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let caret = replace(&mut self.value, range, text);
        self.selection = caret..caret;
        self.marked = None;
        self.changed(window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .or(self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let start = range.start;
        let end = replace(&mut self.value, range, text);
        self.marked = Some(start..end);
        self.selection = selected.map_or(end..end, |r| {
            (start + r.start).min(end)..(start + r.end).min(end)
        });
        self.changed(window, cx);
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.value.encode_utf16().count())
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let n = self.value.encode_utf16().count();
        self.selection = range.start.min(n)..range.end.min(n);
        self.changed(window, cx);
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.value.encode_utf16().count())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_search_edits_keep_unicode_boundaries() {
        let mut s = "A🔭Über".to_string();
        assert_eq!(byte_at(&s, 2), 1);
        let caret = replace(&mut s, 1..3, "Zoom");
        assert_eq!(s, "AZoomÜber");
        assert_eq!(caret, 5);
        replace(&mut s, 5..6, "");
        assert_eq!(s, "AZoomber");
    }
}
