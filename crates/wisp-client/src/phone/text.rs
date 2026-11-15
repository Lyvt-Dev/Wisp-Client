//! The phone setup's words. Minecraft's language files are among the prepared files the phone
//! doesn't have yet, so the setup carries its own, in the phone's language (German or English).
use std::sync::atomic::{AtomicBool, Ordering};

static GERMAN: AtomicBool = AtomicBool::new(false);

/// The phone's language, as Android names it ("de", "en", "de-DE"...).
pub fn set_language(code: &str) {
    GERMAN.store(
        code.to_ascii_lowercase().starts_with("de"),
        Ordering::Relaxed,
    );
}

/// A message by key, in the phone's language. An unknown key is shown as itself.
pub fn t(key: &str) -> &'static str {
    let german = GERMAN.load(Ordering::Relaxed);
    let found = TEXTS.iter().find(|(k, _, _)| *k == key);
    match found {
        Some((_, en, de)) => {
            if german {
                de
            } else {
                en
            }
        }
        None => "?",
    }
}

/// `t(key)` with `{name}` placeholders filled in.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = t(key).to_owned();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// Key, English, German.
const TEXTS: &[(&str, &str, &str)] = &[
    ("title", "Wisp on your phone", "Wisp auf dem Handy"),
    (
        "intro",
        "Two steps before you play: sign in with the Microsoft account that owns Minecraft: Java Edition, and get the game files from your PC.",
        "Zwei Schritte, dann geht's los: Melde dich mit dem Microsoft-Konto an, dem Minecraft: Java Edition gehört, und hol die Spieldaten von deinem PC.",
    ),
    ("account", "Account", "Konto"),
    (
        "signed_in_as",
        "Signed in as {name}",
        "Angemeldet als {name}",
    ),
    ("not_signed_in", "Not signed in", "Nicht angemeldet"),
    (
        "sign_in",
        "Sign in with Microsoft",
        "Mit Microsoft anmelden",
    ),
    ("sign_out", "Sign out", "Abmelden"),
    ("game_files", "Game files", "Spieldaten"),
    (
        "files_ready",
        "Ready (Minecraft {version})",
        "Bereit (Minecraft {version})",
    ),
    (
        "files_missing",
        "Not on this phone yet",
        "Noch nicht auf dem Handy",
    ),
    ("get_files", "Get them from my PC", "Vom PC holen"),
    ("update_files", "Get them again", "Neu holen"),
    ("play", "Play", "Spielen"),
    ("back", "Back", "Zurück"),
    ("cancel", "Cancel", "Abbrechen"),
    (
        "sign_in_title",
        "Sign in with Microsoft",
        "Mit Microsoft anmelden",
    ),
    (
        "sign_in_steps",
        "Open microsoft.com/link on this phone or any other device and enter this code:",
        "Öffne microsoft.com/link auf diesem Handy oder einem anderen Gerät und gib diesen Code ein:",
    ),
    ("open_browser", "Open the browser", "Browser öffnen"),
    (
        "getting_code",
        "Asking Microsoft for a code…",
        "Code wird bei Microsoft angefragt…",
    ),
    (
        "waiting_browser",
        "Waiting for you to finish in the browser…",
        "Warte darauf, dass du im Browser fertig wirst…",
    ),
    (
        "pc_title",
        "Get the game files from your PC",
        "Spieldaten vom PC holen",
    ),
    (
        "pc_steps",
        "On your PC, open the Wisp launcher (0.15.1 or newer), then Settings › Phone › Connect a phone. Keep the phone on the same Wi-Fi.",
        "Öffne am PC den Wisp Launcher (0.15.1 oder neuer), dann Settings › Phone › Connect a phone. Handy und PC müssen im selben WLAN sein.",
    ),
    ("searching", "Looking for your PC…", "Suche deinen PC…"),
    ("none_found", "No PC found yet.", "Noch kein PC gefunden."),
    ("search_again", "Search again", "Erneut suchen"),
    ("type_address", "Type the address", "Adresse eingeben"),
    ("address_title", "The PC's address", "Adresse des PCs"),
    (
        "address_steps",
        "The launcher shows it under the code, for example 192.168.1.20.",
        "Der Launcher zeigt sie unter dem Code an, zum Beispiel 192.168.1.20.",
    ),
    ("code_title", "Enter the code", "Code eingeben"),
    (
        "code_steps",
        "Type the six-digit code the launcher on {name} shows.",
        "Gib den sechsstelligen Code ein, den der Launcher auf {name} anzeigt.",
    ),
    ("next", "Next", "Weiter"),
    ("your_pc", "your PC", "deinem PC"),
    (
        "downloading",
        "Copying the game files…",
        "Spieldaten werden kopiert…",
    ),
    (
        "download_note",
        "About 800 MB over your Wi-Fi. Keep Wisp open until it's done.",
        "Etwa 800 MB über dein WLAN. Lass Wisp geöffnet, bis es fertig ist.",
    ),
    (
        "done",
        "The game files are on your phone.",
        "Die Spieldaten sind auf deinem Handy.",
    ),
    (
        "wrong_code",
        "That code is wrong. Check the launcher and try again.",
        "Der Code stimmt nicht. Schau im Launcher nach und versuch es nochmal.",
    ),
    (
        "offer_closed",
        "The PC stopped offering the files (too many wrong codes, or the dialog was closed). Open it again on the PC.",
        "Der PC bietet die Daten nicht mehr an (zu viele falsche Codes oder Fenster geschlossen). Öffne es am PC erneut.",
    ),
    (
        "busy",
        "Another device is copying from this PC right now.",
        "Gerade kopiert schon ein anderes Gerät von diesem PC.",
    ),
    (
        "unreachable",
        "Can't reach the PC: {error}",
        "PC nicht erreichbar: {error}",
    ),
    (
        "other_version",
        "This PC has files for Minecraft {theirs}, but this app needs {ours}. Update Wisp on both.",
        "Dieser PC hat Daten für Minecraft {theirs}, die App braucht aber {ours}. Aktualisiere Wisp auf beiden.",
    ),
    (
        "other_layout",
        "This PC's Wisp prepared the files for another Wisp version. Update Wisp on both, then press Play once on the PC.",
        "Der Wisp auf dem PC hat die Daten für eine andere Wisp-Version vorbereitet. Aktualisiere Wisp auf beiden und drück am PC einmal auf Spielen.",
    ),
    (
        "pc_not_ready",
        "Wisp on this PC hasn't prepared the game yet. Press Play once in the launcher first.",
        "Wisp auf diesem PC hat das Spiel noch nicht vorbereitet. Drück im Launcher zuerst einmal auf Spielen.",
    ),
    (
        "copy_failed",
        "Copying stopped: {error}",
        "Kopieren abgebrochen: {error}",
    ),
    (
        "play_offline_note",
        "Without signing in you can play singleplayer and offline servers.",
        "Ohne Anmeldung kannst du Einzelspieler und Offline-Server spielen.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_unique() {
        for (i, (key, _, _)) in TEXTS.iter().enumerate() {
            assert!(
                TEXTS[i + 1..].iter().all(|(k, _, _)| k != key),
                "{key} twice"
            );
        }
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(tf("signed_in_as", &[("name", "Alex")]), "Signed in as Alex");
    }

    #[test]
    fn both_languages_have_the_same_placeholders() {
        for (key, en, de) in TEXTS {
            let names = |s: &str| {
                let mut v: Vec<String> = s
                    .split('{')
                    .skip(1)
                    .filter_map(|p| p.split_once('}').map(|(n, _)| n.to_owned()))
                    .collect();
                v.sort();
                v
            };
            assert_eq!(names(en), names(de), "{key}");
        }
    }
}
