use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let mut bytes = [0; 12];
        getrandom::fill(&mut bytes).unwrap();
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("wisp-auth-test-{name}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mock(
    replies: Vec<(&'static str, u16, Value)>,
) -> (Endpoints, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut received = Vec::new();
        for (path, status, body) in replies {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let header = String::from_utf8(request.clone()).unwrap();
            assert!(
                header
                    .lines()
                    .next()
                    .unwrap()
                    .contains(&format!(" {path} ")),
                "{header}"
            );
            let length = header
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut payload = vec![0; length];
            socket.read_exact(&mut payload).unwrap();
            received.push(String::from_utf8(payload).unwrap());
            let body = body.to_string();
            write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        received
    });
    (
        Endpoints {
            microsoft: format!("{base}/oauth"),
            xbox_user: format!("{base}/xbl"),
            xsts: format!("{base}/xsts"),
            minecraft: base,
        },
        handle,
    )
}

fn login_replies(owned: bool) -> Vec<(&'static str, u16, Value)> {
    let mut replies = vec![
        ("/xbl", 200, json!({"Token":"fixture-xbl"})),
        (
            "/xsts",
            200,
            json!({"Token":"fixture-xsts","DisplayClaims":{"xui":[{"uhs":"42","xid":"99"}]}}),
        ),
        (
            "/launcher/login",
            200,
            json!({"access_token":"fixture-minecraft-secret-long-value","expires_in":86400}),
        ),
        (
            "/entitlements/mcstore",
            200,
            json!({"items":if owned {vec![json!({"name":"game_minecraft"})]} else {vec![]}}),
        ),
    ];
    if owned {
        replies.push((
            "/minecraft/profile",
            200,
            json!({"id":"00112233445566778899aabbccddeeff","name":"TestPlayer"}),
        ));
    }
    replies
}

fn fixture() -> SignedIn {
    SignedIn::from_tokens(
        Account {
            uuid: "00112233445566778899aabbccddeeff".into(),
            name: "TestPlayer".into(),
            skin: None,
            cape: None,
        },
        Secrets {
            refresh_token: "fixture-refresh-secret-long-value".into(),
            minecraft_token: "fixture-minecraft-secret-long-value".into(),
            minecraft_expires_at: now() + 86400,
            xuid: Some("99".into()),
            sign_in_required: false,
            owns: true,
        },
    )
}

#[test]
fn device_flow_requests_consumer_scope_and_honors_pending_and_slow_down() {
    let (ep, worker) = mock(vec![
        (
            "/oauth/devicecode",
            200,
            json!({"device_code":"private-device","user_code":"ABCD-EFGH","expires_in":900,"interval":5}),
        ),
        (
            "/oauth/token",
            400,
            json!({"error":"authorization_pending"}),
        ),
        ("/oauth/token", 400, json!({"error":"slow_down"})),
        (
            "/oauth/token",
            200,
            json!({"access_token":"fixture-ms","refresh_token":"fixture-refresh"}),
        ),
    ]);
    let code = device_code(&ep, "wisp-test-client").unwrap();
    assert_eq!(code.user_code, "ABCD-EFGH");
    assert_eq!(code.verification_uri, "https://www.microsoft.com/link");
    assert!(!format!("{code:?}").contains("private-device"));
    assert!(matches!(
        poll(&ep, "wisp-test-client", &code).unwrap(),
        Poll::Pending
    ));
    assert!(matches!(
        poll(&ep, "wisp-test-client", &code).unwrap(),
        Poll::SlowDown
    ));
    assert!(matches!(
        poll(&ep, "wisp-test-client", &code).unwrap(),
        Poll::Done(_)
    ));
    let bodies = worker.join().unwrap();
    assert!(bodies[0].contains("XboxLive.signin") && bodies[0].contains("offline_access"));
    assert!(bodies[1].contains("device_code=private-device"));
}

#[test]
fn authenticated_owner_produces_the_rust_engines_stdin_contract_without_debug_secrets() {
    let (ep, worker) = mock(login_replies(true));
    let (account, secrets) = finish_sign_in(
        &ep,
        MsTokens {
            access: "fixture-ms".into(),
            refresh: "fixture-refresh".into(),
        },
    )
    .unwrap();
    let session = SignedIn::from_tokens(account, secrets);
    assert_eq!(session.account.name, "TestPlayer");
    let credentials = session.credentials();
    assert!(!format!("{credentials:?} {session:?}").contains("fixture-"));
    let mut payload = Vec::new();
    credentials.write_to(&mut payload).unwrap();
    let value: Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(value["name"], "TestPlayer");
    assert_eq!(value["uuid"], "00112233445566778899aabbccddeeff");
    assert_eq!(value["access_token"], "fixture-minecraft-secret-long-value");
    assert_eq!(value["user_type"], "msa");
    assert_eq!(value["xuid"], "99");
    let requests = worker.join().unwrap();
    let xbox: Value = serde_json::from_str(&requests[0]).unwrap();
    assert_eq!(xbox["Properties"]["RpsTicket"], "d=fixture-ms");
    let minecraft: Value = serde_json::from_str(&requests[2]).unwrap();
    assert_eq!(minecraft["xtoken"], "XBL3.0 x=42;fixture-xsts");
}

#[test]
fn account_without_minecraft_is_rejected_and_provider_descriptions_are_not_echoed() {
    let (ep, worker) = mock(login_replies(false));
    let result = finish_sign_in(
        &ep,
        MsTokens {
            access: "fixture-ms".into(),
            refresh: "fixture-refresh".into(),
        },
    );
    assert!(result.unwrap_err().contains("doesn't own"));
    worker.join().unwrap();
    let error = oauth_error(
        &json!({"error":"unknown-provider-error","error_description":"private-token-value"}),
    );
    assert!(!error.contains("private-token-value"));
}

#[test]
fn cancelled_browser_wait_does_not_contact_the_token_endpoint() {
    let code = DeviceCode {
        user_code: "ABCD".into(),
        verification_uri: "https://www.microsoft.com/link".into(),
        interval: 1,
        expires_at: now() + 60,
        device_code: "private".into(),
    };
    assert!(
        wait_for_sign_in(&Endpoints::live(), "test", &code, &|| true)
            .unwrap_err()
            .contains("cancelled")
    );
}

#[test]
fn rotated_refresh_survives_service_failure_and_rejected_refresh_requires_signin() {
    let scratch = Scratch::new();
    let mut session = fixture();
    session.secrets.minecraft_expires_at = 0;
    let (ep, worker) = mock(vec![
        (
            "/oauth/token",
            200,
            json!({"access_token":"fixture-new-ms","refresh_token":"fixture-rotated-refresh"}),
        ),
        ("/xbl", 503, json!({"error":"service_unavailable"})),
    ]);
    assert!(session
        .renew(&scratch.0, &ep, "wisp-test-client", &|_| {})
        .is_err());
    worker.join().unwrap();
    assert_eq!(session.secrets.refresh_token, "fixture-rotated-refresh");
    #[cfg(windows)]
    assert_eq!(
        saved(&scratch.0).unwrap().unwrap().secrets.refresh_token,
        "fixture-rotated-refresh"
    );
    let (ep, worker) = mock(vec![(
        "/oauth/token",
        400,
        json!({"error":"invalid_grant"}),
    )]);
    assert_eq!(
        session
            .renew(&scratch.0, &ep, "wisp-test-client", &|_| {})
            .unwrap_err(),
        SIGN_IN_AGAIN
    );
    let bodies = worker.join().unwrap();
    assert!(bodies[0].contains("fixture-rotated-refresh"));
    assert!(session.secrets.sign_in_required);
    #[cfg(windows)]
    assert!(saved(&scratch.0).unwrap().unwrap().secrets.sign_in_required);
}

#[cfg(unix)]
#[test]
fn online_launch_delivers_session_on_stdin_and_keeps_it_out_of_arguments_and_logs() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let binary = scratch.0.join("fixture-game");
    std::fs::write(&binary, "#!/usr/bin/env python3\nimport json,sys\nv=json.load(sys.stdin)\nassert v['access_token']=='fixture-minecraft-secret-long-value'\nassert v['name']=='TestPlayer' and v['user_type']=='msa'\nassert sys.argv.count('--session-stdin')==1\nassert not any('fixture-minecraft-secret-long-value' in arg for arg in sys.argv)\nprint('stdin contract passed')\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let resources = scratch.0.join("resources");
    std::fs::create_dir(&resources).unwrap();
    let mut profile =
        crate::Profile::new("Test".into(), &binary, &resources, &scratch.0.join("game")).unwrap();
    profile.use_account = true;
    let status = profile
        .launch_with_credentials(Some(&fixture().credentials()))
        .unwrap()
        .wait()
        .unwrap();
    let log = std::fs::read_to_string(profile.game_home.join("logs/local-launcher.log")).unwrap();
    assert!(status.success(), "{log}");
    assert_eq!(log.trim(), "stdin contract passed");
    profile.args.push("--session-file=old.json".into());
    assert!(profile
        .launch_with_credentials(Some(&fixture().credentials()))
        .is_err());
}

#[test]
fn account_storage_never_falls_back_to_plaintext_and_signout_preserves_profiles() {
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("local-profiles.json"), "keep").unwrap();
    save(&scratch.0, &fixture()).unwrap();
    #[cfg(windows)]
    {
        let bytes = std::fs::read(scratch.0.join("wisp-accounts.dat")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("fixture-refresh"));
        assert_eq!(
            saved(&scratch.0).unwrap().unwrap().account.name,
            "TestPlayer"
        );
        std::fs::write(scratch.0.join("wisp-accounts.dat"), b"damaged").unwrap();
        assert!(saved(&scratch.0).is_err());
        assert_eq!(
            std::fs::read(scratch.0.join("wisp-accounts.dat")).unwrap(),
            b"damaged"
        );
        std::fs::remove_file(scratch.0.join("wisp-accounts.dat")).unwrap();
    }
    #[cfg(not(windows))]
    assert!(!scratch.0.join("wisp-accounts.dat").exists());
    sign_out(&scratch.0).unwrap();
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("local-profiles.json")).unwrap(),
        "keep"
    );
}

#[cfg(windows)]
#[test]
fn several_accounts_are_kept_and_one_plays() {
    let scratch = Scratch::new();
    let first = fixture();
    let mut second = fixture();
    second.account.uuid = "ffffffffffffffffffffffffffffffff".into();
    second.account.name = "SecondPlayer".into();
    save(&scratch.0, &first).unwrap();
    save(&scratch.0, &second).unwrap();
    // The newest sign-in plays; both are kept.
    assert_eq!(
        saved(&scratch.0).unwrap().unwrap().account.name,
        "SecondPlayer"
    );
    let (list, active) = accounts(&scratch.0).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(active.as_deref(), Some("ffffffffffffffffffffffffffffffff"));
    // Switching back, then signing the one that plays out, leaves the other playing.
    switch(&scratch.0, &first.account.uuid).unwrap();
    assert_eq!(
        saved(&scratch.0).unwrap().unwrap().account.name,
        "TestPlayer"
    );
    sign_out(&scratch.0).unwrap();
    assert_eq!(
        saved(&scratch.0).unwrap().unwrap().account.name,
        "SecondPlayer"
    );
    assert!(switch(&scratch.0, &first.account.uuid).is_err());
    remove(&scratch.0, &second.account.uuid).unwrap();
    assert!(saved(&scratch.0).unwrap().is_none());
}
