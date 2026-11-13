use super::*;
use crate::sources::sha1_hex;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let mut bytes = [0u8; 12];
        getrandom::fill(&mut bytes).unwrap();
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let root = std::env::temp_dir().join(format!("wisp-install-{id}"));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn automatic_install_requires_an_owner_before_downloading_or_creating_instances() {
    let scratch = Scratch::new();
    let result = install(&scratch.0, None, &[], &AtomicBool::new(false), &|_| {});
    assert!(matches!(result, Err(error) if error.contains("owns Minecraft")));
    assert!(!scratch.0.join("minecraft").exists());
    assert!(!scratch.0.join("instances").exists());
    assert!(!scratch.0.join("local-profiles.json").exists());
}

fn fixture() -> (Value, HashMap<String, Vec<u8>>) {
    let client = b"synthetic client";
    let server = b"synthetic server";
    let compiler = b"synthetic compiler";
    let index = br#"{"objects":{}}"#;
    let component = "java-runtime-epsilon";
    let tool = if cfg!(windows) { "javac.exe" } else { "javac" };
    let runtime = json!({"files": {
        format!("bin/{tool}"): {"type":"file","executable":true,"downloads":{"raw":{
            "url":"https://piston-data.mojang.com/compiler","size":compiler.len(),"sha1":sha1_hex(compiler)
        }}}
    }}).to_string();
    let java_index = json!({mojang::java_platform().unwrap(): {component: [{
        "manifest":{"url":"https://piston-meta.mojang.com/runtime.json","sha1":sha1_hex(runtime.as_bytes())},
        "version":{"name":"25.0.1"}
    }]}}).to_string();
    let manifest = json!({"id":"26.3","libraries":[],
        "downloads":{
            "client":{"url":"https://piston-data.mojang.com/client","sha1":sha1_hex(client),"size":client.len()},
            "server":{"url":"https://piston-data.mojang.com/server","sha1":sha1_hex(server),"size":server.len()}
        },
        "assetIndex":{"id":"fixture","url":"https://piston-meta.mojang.com/assets.json","sha1":sha1_hex(index),"size":index.len()},
        "javaVersion":{"majorVersion":25,"component":component}
    }).to_string();
    let hash = sha1_hex(manifest.as_bytes());
    let lock = json!({"minecraft":{"version":"26.3","manifest_sha1":hash,"client_sha1":sha1_hex(client),"server_sha1":sha1_hex(server)}});
    let files = HashMap::from([
        (format!("https://piston-meta.mojang.com/v1/packages/{hash}/26.3.json"), manifest.into_bytes()),
        ("https://piston-data.mojang.com/client".into(),client.to_vec()),
        ("https://piston-data.mojang.com/server".into(),server.to_vec()),
        ("https://piston-data.mojang.com/compiler".into(),compiler.to_vec()),
        ("https://piston-meta.mojang.com/assets.json".into(),index.to_vec()),
        ("https://piston-meta.mojang.com/runtime.json".into(),runtime.into_bytes()),
        ("https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json".into(),java_index.into_bytes())
    ]);
    (lock, files)
}

#[test]
fn provisions_game_server_and_compiler_and_repairs_only_damaged_cached_files() {
    let scratch = Scratch::new();
    let (lock, files) = fixture();
    let asked = Mutex::new(Vec::new());
    let fetch = |url: &str| {
        asked.lock().unwrap().push(url.to_owned());
        files
            .get(url)
            .cloned()
            .ok_or_else(|| format!("unexpected URL: {url}"))
    };
    let cancel = AtomicBool::new(false);
    let inputs = provision(&lock, &scratch.0, &fetch, &cancel, &|_| {}).unwrap();
    assert_eq!(std::fs::read(&inputs.server).unwrap(), b"synthetic server");
    let compiler = inputs
        .java
        .join("bin")
        .join(if cfg!(windows) { "javac.exe" } else { "javac" });
    assert_eq!(std::fs::read(&compiler).unwrap(), b"synthetic compiler");
    let client = inputs.minecraft.join("versions/26.3/26.3.jar");
    assert_eq!(std::fs::read(&client).unwrap(), b"synthetic client");
    asked.lock().unwrap().clear();
    std::fs::write(&client, b"partial").unwrap();
    std::fs::write(&compiler, b"damaged").unwrap();
    provision(&lock, &scratch.0, &fetch, &cancel, &|_| {}).unwrap();
    assert_eq!(std::fs::read(&client).unwrap(), b"synthetic client");
    assert_eq!(std::fs::read(&compiler).unwrap(), b"synthetic compiler");
    let requests = asked.lock().unwrap();
    assert!(requests.iter().any(|url| url.ends_with("/client")));
    assert!(requests.iter().any(|url| url.ends_with("/compiler")));
    assert!(!requests.iter().any(|url| url.ends_with("/server")));
}

#[test]
fn a_server_that_does_not_match_the_bundled_game_is_refused_before_download() {
    let scratch = Scratch::new();
    let (mut lock, files) = fixture();
    lock["minecraft"]["server_sha1"] = json!("0000000000000000000000000000000000000000");
    let fetch = |url: &str| {
        assert!(!url.ends_with("/server"));
        files.get(url).cloned().ok_or_else(|| url.to_owned())
    };
    let result = provision(&lock, &scratch.0, &fetch, &AtomicBool::new(false), &|_| {});
    assert!(matches!(result, Err(error) if error.contains("does not match")));
    assert!(!scratch.0.join("minecraft/server-jars/26.3.jar").exists());
}
