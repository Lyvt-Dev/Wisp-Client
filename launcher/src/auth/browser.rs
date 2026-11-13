//! Signing in on Microsoft's own page, in a launcher window (`window.rs`): the player signs in
//! (or picks one of the accounts Microsoft remembers there) and the page goes to the app's
//! desktop redirect address with a code the launcher trades for tokens. The authorization-code
//! flow with PKCE, as public desktop apps use it: no secret, and the code is useless without the
//! verifier only this launcher holds.
//!
//! The redirect is `REDIRECT`, registered for the app on the "Mobile and desktop applications"
//! platform; the window catches the navigation to it, so nothing listens on this computer.
use super::*;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// The app's registered desktop redirect address.
pub const REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

fn random(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|_| "No random source.".to_owned())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&value[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The query of an address `https://host/path?a=b&c=d#frag`, decoded.
fn query(url: &str) -> HashMap<String, String> {
    let query = url.split_once('?').map_or("", |(_, q)| q);
    let query = query.split('#').next().unwrap_or_default();
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

/// The code in the redirect address the page went to, if it carries our `state`.
fn code_from(redirected: &str, state: &str) -> Result<String, String> {
    let q = query(redirected);
    if let Some(code) = q
        .get("code")
        .filter(|_| q.get("state").map(String::as_str) == Some(state))
    {
        return Ok(code.clone());
    }
    Err(match q.get("error").map(String::as_str) {
        Some("access_denied") => crate::i18n::tr("Sign-in was cancelled."),
        _ => crate::i18n::tr("Microsoft sign-in failed. Try again."),
    })
}

/// Signs in on Microsoft's page: `show` shows the sign-in address and returns the redirect
/// address the page went to; the code in it is traded for Microsoft tokens.
pub fn browser_sign_in(
    ep: &Endpoints,
    client_id: &str,
    show: &dyn Fn(&str) -> Result<String, String>,
) -> Result<MsTokens, String> {
    let verifier = random(48)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16)?;
    let url = format!(
        "{}/authorize?client_id={}&response_type=code&redirect_uri={}&response_mode=query\
         &scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}\
         &prompt=select_account",
        ep.microsoft,
        encode(client_id),
        encode(REDIRECT),
        encode(SCOPE),
    );
    let code = code_from(&show(&url)?, &state)?;
    let (status, v) = send(
        request_to("POST", &format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", &code),
            ("redirect_uri", REDIRECT),
            ("code_verifier", &verifier),
            ("scope", SCOPE),
        ])),
        "Microsoft",
    )?;
    ms_tokens(status, &v)
}

/// Shows `url` in the sign-in window (this launcher run as `sign-in-window`) until the page
/// redirects, the player closes it, or `cancelled`.
pub fn sign_in_window(
    url: &str,
    data: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, String> {
    use std::io::Read;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(exe)
        .args(["sign-in-window", url, REDIRECT])
        .arg(data)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let status = loop {
        if cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(crate::i18n::tr("Sign-in cancelled."));
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut out);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut err);
    }
    match status.code() {
        Some(0) if !out.trim().is_empty() => Ok(out.trim().to_owned()),
        Some(2) => Err(crate::i18n::tr("Sign-in cancelled.")),
        _ => Err(crate::i18n::trf(
            "The sign-in window could not open ({reason}). Use a code instead.",
            &[("reason", err.trim())],
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    #[test]
    fn the_code_is_read_from_the_redirect() {
        let at = format!("{REDIRECT}?code=M.C5%2Fab_c&state=xyz");
        assert_eq!(code_from(&at, "xyz").unwrap(), "M.C5/ab_c");
        // Another sign-in's answer doesn't count.
        assert!(code_from(&at, "other").is_err());
        assert!(code_from(&format!("{REDIRECT}?error=access_denied&state=xyz"), "xyz").is_err());
    }

    #[test]
    fn addresses_are_encoded_for_a_url() {
        assert_eq!(encode("http://localhost:5"), "http%3A%2F%2Flocalhost%3A5");
        assert_eq!(
            encode("XboxLive.signin offline_access"),
            "XboxLive.signin%20offline_access"
        );
    }

    #[test]
    fn the_whole_flow_against_a_fake_microsoft() {
        // The fake token endpoint checks the verifier matches the challenge sent.
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let challenge = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen = challenge.clone();
        std::thread::spawn(move || {
            for stream in server.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut length = 0;
                loop {
                    let mut l = String::new();
                    reader.read_line(&mut l).unwrap();
                    if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if l == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                std::io::Read::read_exact(&mut reader, &mut body).unwrap();
                let form = query(&format!("/?{}", String::from_utf8(body).unwrap()));
                let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(Sha256::digest(form["code_verifier"].as_bytes()));
                let ok = form["code"] == "the-code"
                    && form["redirect_uri"] == REDIRECT
                    && *seen.lock().unwrap() == expected;
                let answer = if ok {
                    r#"{"access_token":"a","refresh_token":"r"}"#
                } else {
                    r#"{"error":"invalid_grant"}"#
                };
                let mut out = &stream;
                write!(
                    out,
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{answer}",
                    if ok { "200 OK" } else { "400 Bad Request" },
                    answer.len()
                )
                .unwrap();
            }
        });
        let ep = Endpoints {
            microsoft: base,
            ..Endpoints::live()
        };
        // The "window": reads the sign-in address and redirects with a code.
        let show = |url: &str| {
            let q = query(url);
            *challenge.lock().unwrap() = q["code_challenge"].clone();
            assert_eq!(q["redirect_uri"], REDIRECT);
            Ok(format!(
                "{}?code=the-code&state={}",
                q["redirect_uri"], q["state"]
            ))
        };
        let tokens = browser_sign_in(&ep, "client", &show).unwrap();
        assert_eq!(
            (tokens.access.as_str(), tokens.refresh.as_str()),
            ("a", "r")
        );
    }
}
