//! Reference catalog layouts, backed by actual local instances and package metadata.
use super::reference_views::surface;
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum InstanceFilter {
    #[default]
    All,
    Vanilla,
    Modded,
    Imported,
}
#[derive(Default)]
pub(super) struct CatalogUi {
    pub versions_tab: InstanceFilter,
    pub versions_reverse: bool,
    pub enabled_only: bool,
    pub shader_water: bool,
    pub shader_shadows: bool,
    pub shader_clouds: bool,
    pub local_only: bool,
    pub favorites_only: bool,
    pub activity: bool,
    /// The Mods page's category tab (None: all).
    pub mods_category: Option<usize>,
}
pub(super) fn cat_panel(s: f32) -> Div {
    surface(s).rounded(px(18. * s))
}
pub(super) fn badge(label: impl Into<SharedString>, s: f32, accent: bool) -> Div {
    div()
        .px(px(11. * s))
        .py(px(4. * s))
        .rounded_full()
        .text_size(px(13. * s))
        .bg(rgb(if accent {
            tone(0x404249)
        } else {
            tone(0x2c2d32)
        }))
        .child(label.into())
}
pub(super) fn cat_button(
    id: impl Into<ElementId>,
    label: &str,
    glyph: &'static str,
    bright: bool,
    s: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(46. * s))
        .px(px(18. * s))
        .flex_none()
        .rounded(px(12. * s))
        .border_1()
        .border_color(rgb(if bright { 0xd6d7da } else { tone(RULE) }))
        .when(bright, |d| d.bg(rgb(0xffffff)).text_color(rgb(0x0b0b0d)))
        .when(!bright, |d| d.bg(rgba(0x16171acc)))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(9. * s))
        .text_size(px(16. * s))
        .cursor_pointer()
        .hover(|d| d.opacity(0.86))
        .child(icon(glyph, if bright { 0x0b0b0d } else { INK }).size(px(22. * s)))
        .child(tr(label))
}
impl Launcher {
    pub(super) fn is_catalog(&self) -> bool {
        matches!(self.page, Page::Instances | Page::Mods | Page::Shaders)
    }
    pub(super) fn activity_panel(&self, cx: &mut Context<Self>) -> Div {
        let s = self.ui_scale;
        cat_panel(s)
            .p(px(15. * s))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12. * s))
            .child(icon("info", accent_tone(ACCENT)))
            .child(
                div()
                    .flex_1()
                    .text_size(px(15. * s))
                    .child(self.status.clone()),
            )
            .child(
                cat_button("activity-logs", "Open logs", "terminal", false, s).on_click(
                    cx.listener(|this, _, _, cx| this.show_path(this.home.join("logs"), cx)),
                ),
            )
    }
}
