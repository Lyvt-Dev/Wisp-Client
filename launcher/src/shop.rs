//! The Shop (`wisp_shop`: items, wallet, rules), the Wisp server it runs on, and the refusals in
//! the player's language.
pub use wisp_shop::*;

/// A refusal in the player's language.
pub fn refusal(refused: &Refused) -> String {
    match refused {
        Refused::Short(n) => crate::i18n::trf(refused.message(), &[("coins", &n.to_string())]),
        _ => crate::i18n::tr(refused.message()),
    }
}

/// The Wisp cosmetics server players share what they wear through
/// (`crates/wisp-cosmetics-server`). Empty: not deployed yet, and only the player sees their
/// own items. `WISP_COSMETICS_API` overrides it.
pub const SERVICE: &str = "";

/// The address set in the launcher's Settings, which wins over `SERVICE`.
static CHOSEN: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Uses `url` (the launcher's Settings) for the Wisp server; empty or None: `SERVICE`.
pub fn set_service(url: Option<&str>) {
    let url = url
        .map(|s| s.trim().trim_end_matches('/').to_owned())
        .filter(|s| !s.is_empty());
    *CHOSEN
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = url;
}

/// The Wisp server (cosmetics and friends) to use, if any: `WISP_COSMETICS_API`, then the
/// address in the launcher's Settings, then `SERVICE`.
pub fn service() -> Option<String> {
    let chosen = CHOSEN
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    wisp_compat::var("WISP_COSMETICS_API")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or(chosen)
        .or_else(|| Some(SERVICE.to_owned()))
        .map(|s| s.trim().trim_end_matches('/').to_owned())
        .filter(|s| !s.is_empty())
}
