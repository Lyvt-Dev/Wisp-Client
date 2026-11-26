use super::*;

/// The account before there could be several: one, encrypted. Read once and moved over.
const SAVED_ACCOUNT: &str = "wisp-account.dat";
/// Every account signed in, and which one plays, encrypted for this user (Windows: DPAPI;
/// Linux: a key in the keyring, `linux`).
const SAVED_ACCOUNTS: &str = "wisp-accounts.dat";

/// The accounts kept: which one plays (`active`), and the others to switch to.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Store {
    active: Option<String>,
    accounts: Vec<SignedIn>,
}

impl Store {
    fn active(&self) -> Option<&SignedIn> {
        let uuid = self.active.as_ref()?;
        self.accounts.iter().find(|a| &a.account.uuid == uuid)
    }

    /// Puts `session` in, replacing the same account's older entry.
    fn put(&mut self, session: &SignedIn) {
        match self
            .accounts
            .iter_mut()
            .find(|a| a.account.uuid == session.account.uuid)
        {
            Some(entry) => *entry = session.clone(),
            None => self.accounts.push(session.clone()),
        }
    }
}

fn with_lock<T>(home: &Path, action: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("wisp-account.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock()
        .map_err(|_| crate::i18n::tr("Could not open the account store."))?;
    let result = action();
    let _ = lock.unlock();
    result
}

/// The account that plays, if any.
pub fn saved(home: &Path) -> Result<Option<SignedIn>, String> {
    with_lock(home, || load_locked(home))
}

/// Keeps a newly signed-in account and makes it the one that plays.
pub fn save(home: &Path, session: &SignedIn) -> Result<(), String> {
    with_lock(home, || {
        let mut store = read_store(home)?;
        store.put(session);
        store.active = Some(session.account.uuid.clone());
        write_store(home, &store)
    })
}

/// Every account kept (no secrets), and the UUID of the one that plays.
pub fn accounts(home: &Path) -> Result<(Vec<Account>, Option<String>), String> {
    with_lock(home, || {
        let store = read_store(home)?;
        Ok((
            store.accounts.iter().map(|a| a.account.clone()).collect(),
            store.active,
        ))
    })
}

/// Makes the kept account `uuid` the one that plays.
pub fn switch(home: &Path, uuid: &str) -> Result<SignedIn, String> {
    with_lock(home, || {
        let mut store = read_store(home)?;
        let session = store
            .accounts
            .iter()
            .find(|a| a.account.uuid == uuid)
            .cloned()
            .ok_or_else(|| crate::i18n::tr("That account is no longer signed in."))?;
        store.active = Some(uuid.to_owned());
        write_store(home, &store)?;
        Ok(session)
    })
}

/// Forgets the account `uuid`; if it was the one that plays, the next kept one plays instead.
pub fn remove(home: &Path, uuid: &str) -> Result<Option<SignedIn>, String> {
    with_lock(home, || {
        let mut store = read_store(home)?;
        store.accounts.retain(|a| a.account.uuid != uuid);
        if store.active.as_deref() == Some(uuid) || store.active().is_none() {
            store.active = store.accounts.first().map(|a| a.account.uuid.clone());
        }
        write_store(home, &store)?;
        Ok(store.active().cloned())
    })
}

/// Signs the account that plays out (forgets it); another kept account plays next, if any.
pub fn sign_out(home: &Path) -> Result<(), String> {
    let active = with_lock(home, || Ok(read_store(home)?.active))?;
    match active {
        Some(uuid) => remove(home, &uuid).map(|_| ()),
        None => Ok(()),
    }
}

pub fn for_game(
    home: &Path,
    cached: Option<SignedIn>,
    progress: &dyn Fn(Phase),
) -> Result<SignedIn, String> {
    let client = client_id().ok_or(NOT_CONFIGURED)?;
    with_lock(home, || {
        let stored = load_locked(home)?;
        #[cfg(windows)]
        let mut session = stored.ok_or(NEEDS_OWNER)?;
        #[cfg(not(windows))]
        let mut session = stored.or(cached).ok_or(NEEDS_OWNER)?;
        #[cfg(windows)]
        let _ = cached;
        session.renew(home, &Endpoints::live(), &client, progress)?;
        Ok(session)
    })
}

fn load_locked(home: &Path) -> Result<Option<SignedIn>, String> {
    Ok(read_store(home)?.active().cloned())
}

/// Keeps `session` (renewed tokens) without changing which account plays.
pub(super) fn save_locked(home: &Path, session: &SignedIn) -> Result<(), String> {
    let mut store = read_store(home)?;
    store.put(session);
    if store.active.is_none() {
        store.active = Some(session.account.uuid.clone());
    }
    write_store(home, &store)
}

/// The store; the single account kept before is moved into it the first time.
fn read_store(home: &Path) -> Result<Store, String> {
    if let Some(bytes) = read_encrypted(home, SAVED_ACCOUNTS)? {
        return serde_json::from_slice(&bytes).map_err(|_| {
            crate::i18n::tr("The saved sign-ins could not be read. Sign out and sign in again.")
        });
    }
    let Some(bytes) = read_encrypted(home, SAVED_ACCOUNT)? else {
        return Ok(Store::default());
    };
    let old: SignedIn = serde_json::from_slice(&bytes).map_err(|_| {
        crate::i18n::tr("The saved sign-in could not be read. Sign out and sign in again.")
    })?;
    let store = Store {
        active: Some(old.account.uuid.clone()),
        accounts: vec![old],
    };
    write_store(home, &store)?;
    let _ = std::fs::remove_file(home.join(SAVED_ACCOUNT));
    Ok(store)
}

fn write_store(home: &Path, store: &Store) -> Result<(), String> {
    let plain =
        serde_json::to_vec(store).map_err(|_| crate::i18n::tr("Could not save the sign-in."))?;
    write_encrypted(home, SAVED_ACCOUNTS, &plain)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn read_encrypted(_: &Path, _: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(None)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn write_encrypted(_: &Path, _: &str, _: &[u8]) -> Result<(), String> {
    // Other platforms keep credentials in memory; never create a plaintext token file.
    Ok(())
}

/// Linux: the sign-ins are kept encrypted (ChaCha20-Poly1305) in the launcher's folder, readable
/// by this user only. The key is a random one kept in the desktop's keyring (the Secret Service:
/// GNOME Keyring, KWallet), or, where there is none, in a file of its own beside it, readable by
/// this user only, as other Linux launchers keep their sign-ins.
#[cfg(target_os = "linux")]
mod linux {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::Path;

    const KEYRING_SERVICE: &str = "Wisp Client";
    const KEY_FILE: &str = "wisp-account.key";
    const MAGIC: &[u8; 4] = b"WSP1";

    fn unreadable() -> String {
        crate::i18n::tr("The saved sign-ins could not be read. Sign out and sign in again.")
    }

    fn not_saved() -> String {
        crate::i18n::tr("Could not save the sign-in.")
    }

    /// This launcher folder's entry in the keyring (a folder of its own for tests and agents).
    fn keyring_entry(home: &Path) -> Option<keyring::Entry> {
        // Tests never touch the player's keyring.
        if cfg!(test) || wisp_compat::var_os("WISP_ACCOUNT_KEYRING").is_some_and(|v| v == "off") {
            return None;
        }
        keyring::Entry::new(KEYRING_SERVICE, &format!("accounts key {}", home.display())).ok()
    }

    /// The key, made when `create` and there is none yet. The key file, once there, is the key:
    /// a keyring that is locked or gone at one start must not orphan what an earlier one saved.
    fn key(home: &Path, create: bool) -> Result<Option<[u8; 32]>, String> {
        let file = home.join(KEY_FILE);
        match std::fs::read(&file) {
            Ok(bytes) => return bytes.try_into().map(Some).map_err(|_| unreadable()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(unreadable()),
        }
        let entry = keyring_entry(home);
        if let Some(entry) = &entry {
            match entry.get_secret() {
                Ok(bytes) => return bytes.try_into().map(Some).map_err(|_| unreadable()),
                Err(keyring::Error::NoEntry) => {}
                // No keyring here: the key file it is.
                Err(_) => {}
            }
        }
        if !create {
            return Ok(None);
        }
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|_| not_saved())?;
        let kept = entry.is_some_and(|entry| {
            entry.set_secret(&key).is_ok() && entry.get_secret().is_ok_and(|b| b == key)
        });
        if kept {
            return Ok(Some(key));
        }
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&file)
            .map_err(|_| not_saved())?;
        out.write_all(&key).map_err(|_| not_saved())?;
        out.sync_all().map_err(|_| not_saved())?;
        Ok(Some(key))
    }

    pub(super) fn read(home: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        let sealed = match std::fs::read(home.join(name)) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => bytes,
            Ok(_) => return Err(unreadable()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(unreadable()),
        };
        if sealed.len() < MAGIC.len() + 12 || &sealed[..4] != MAGIC {
            return Err(unreadable());
        }
        let key = key(home, false)?.ok_or_else(unreadable)?;
        let cipher = chacha20poly1305::ChaCha20Poly1305::new(&key.into());
        let (nonce, body) = sealed[4..].split_at(12);
        cipher
            .decrypt(nonce.into(), body)
            .map(Some)
            .map_err(|_| unreadable())
    }

    pub(super) fn write(home: &Path, name: &str, plain: &[u8]) -> Result<(), String> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        let key = key(home, true)?.ok_or_else(not_saved)?;
        let cipher = chacha20poly1305::ChaCha20Poly1305::new(&key.into());
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| not_saved())?;
        let body = cipher
            .encrypt((&nonce).into(), plain)
            .map_err(|_| not_saved())?;
        let mut sealed = MAGIC.to_vec();
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&body);
        let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let temporary = home.join(format!(".wisp-account-{suffix}.tmp"));
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&sealed)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, home.join(name))?;
            // An older file may have had wider permissions.
            std::fs::set_permissions(home.join(name), std::fs::Permissions::from_mode(0o600))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|_| not_saved())
    }
}

#[cfg(target_os = "linux")]
fn read_encrypted(home: &Path, file: &str) -> Result<Option<Vec<u8>>, String> {
    linux::read(home, file)
}

#[cfg(target_os = "linux")]
fn write_encrypted(home: &Path, name: &str, plain: &[u8]) -> Result<(), String> {
    linux::write(home, name, plain)
}

#[cfg(windows)]
fn read_encrypted(home: &Path, file: &str) -> Result<Option<Vec<u8>>, String> {
    let cipher = match std::fs::read(home.join(file)) {
        Ok(bytes) if bytes.len() <= 1024 * 1024 => bytes,
        Ok(_) => {
            return Err(crate::i18n::tr(
                "The saved sign-in could not be read. Sign out and sign in again.",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(crate::i18n::tr("The saved sign-in could not be read.")),
    };
    protect(&cipher, false).map(Some)
}

#[cfg(windows)]
fn write_encrypted(home: &Path, name: &str, plain: &[u8]) -> Result<(), String> {
    {
        let cipher = protect(plain, true)?;
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| crate::i18n::tr("Could not save the sign-in."))?;
        let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let temporary = home.join(format!(".wisp-account-{suffix}.tmp"));
        let result = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&cipher)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, home.join(name))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|_| crate::i18n::tr("Could not save the sign-in."))
    }
}

#[cfg(windows)]
fn protect(input: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let length = u32::try_from(input.len())
        .map_err(|_| crate::i18n::tr("Could not open the account store."))?;
    let data = CRYPT_INTEGER_BLOB {
        cbData: length,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI binds the blob to the current Windows user; returned storage belongs to LocalAlloc.
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &data,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &data,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(crate::i18n::tr(
            "Windows could not protect or unlock this sign-in. Sign in again.",
        ));
    }
    let result = if output.cbData == 0 {
        Vec::new()
    } else {
        // A successful DPAPI call supplies a valid buffer of exactly cbData bytes.
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() }
    };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}
