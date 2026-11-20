use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::Result;

const SERVICE: &str = "dev.zsortmail.app";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Keychain,
    File,
    Memory,
}

pub trait Secrets: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, secret: &str) -> Result<Where>;
    fn delete(&self, account: &str) -> Result<()>;
}

pub struct Keychain;

fn entry(account: &str) -> std::result::Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(SERVICE, account)
}

impl Keychain {
    fn get_inner(account: &str) -> std::result::Result<Option<String>, keyring::Error> {
        match entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Secrets for Keychain {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Self::get_inner(account).map_err(|e| std::io::Error::other(e.to_string()).into())
    }

    fn set(&self, account: &str, secret: &str) -> Result<Where> {
        entry(account)
            .and_then(|e| e.set_password(secret))
            .map(|()| Where::Keychain)
            .map_err(|e| std::io::Error::other(e.to_string()).into())
    }

    fn delete(&self, account: &str) -> Result<()> {
        match entry(account).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(std::io::Error::other(e.to_string()).into()),
        }
    }
}

pub struct FileSecrets {
    dir: PathBuf,
}

impl FileSecrets {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, account: &str) -> PathBuf {
        let safe: String =
            account.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
        let hash =
            account.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
        self.dir.join(format!("{safe}-{hash:016x}.token"))
    }
}

impl Secrets for FileSecrets {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match std::fs::read_to_string(self.path(account)) {
            Ok(text) => Ok(Some(text.trim().to_string()).filter(|t| !t.is_empty())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<Where> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path(account);
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file =
                std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
            file.write_all(secret.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&path, secret)?;
        Ok(Where::File)
    }

    fn delete(&self, account: &str) -> Result<()> {
        match std::fs::remove_file(self.path(account)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

pub struct Layered {
    primary: Box<dyn Secrets>,
    fallback: Box<dyn Secrets>,
}

impl Layered {
    pub fn new(primary: Box<dyn Secrets>, fallback: Box<dyn Secrets>) -> Self {
        Self { primary, fallback }
    }
}

impl Secrets for Layered {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match self.primary.get(account) {
            Ok(Some(secret)) => Ok(Some(secret)),
            Ok(None) | Err(_) => self.fallback.get(account),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<Where> {
        match self.primary.set(account, secret) {
            Ok(place) => {
                let _ = self.fallback.delete(account);
                Ok(place)
            }
            Err(error) => {
                tracing::warn!(%error, "no keychain available; keeping the sign-in in a protected file");
                self.fallback.set(account, secret)
            }
        }
    }

    fn delete(&self, account: &str) -> Result<()> {
        let first = self.primary.delete(account);
        let second = self.fallback.delete(account);
        first.and(second)
    }
}

#[derive(Default)]
pub struct MemorySecrets(Mutex<std::collections::HashMap<String, String>>);

impl Secrets for MemorySecrets {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.0.lock().expect("lock").get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<Where> {
        self.0.lock().expect("lock").insert(account.to_string(), secret.to_string());
        Ok(Where::Memory)
    }

    fn delete(&self, account: &str) -> Result<()> {
        self.0.lock().expect("lock").remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Broken;

    impl Secrets for Broken {
        fn get(&self, _: &str) -> Result<Option<String>> {
            Err(std::io::Error::other("no keychain").into())
        }
        fn set(&self, _: &str, _: &str) -> Result<Where> {
            Err(std::io::Error::other("no keychain").into())
        }
        fn delete(&self, _: &str) -> Result<()> {
            Err(std::io::Error::other("no keychain").into())
        }
    }

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("zsm-secrets-{:x}", rand::random::<u64>()))
    }

    #[test]
    fn a_file_secret_round_trips_and_is_private() {
        let dir = temp_dir();
        let secrets = FileSecrets::new(dir.clone());
        assert_eq!(secrets.get("a@b.example").unwrap(), None);
        assert_eq!(secrets.set("a@b.example", "refresh-1").unwrap(), Where::File);
        assert_eq!(secrets.get("a@b.example").unwrap().as_deref(), Some("refresh-1"));
        secrets.set("a@b.example", "refresh-2").unwrap();
        assert_eq!(secrets.get("a@b.example").unwrap().as_deref(), Some("refresh-2"), "a new token replaces the old");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(secrets.path("a@b.example")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "only the owner may read it");
        }
        secrets.delete("a@b.example").unwrap();
        assert_eq!(secrets.get("a@b.example").unwrap(), None);
        secrets.delete("a@b.example").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn different_accounts_never_share_a_file() {
        let dir = temp_dir();
        let secrets = FileSecrets::new(dir.clone());
        assert_ne!(secrets.path("a.b@x.example"), secrets.path("a_b@x.example"));
        let odd = secrets.path("../../etc/passwd");
        assert_eq!(odd.parent(), Some(dir.as_path()));
        assert!(!odd.file_name().unwrap().to_string_lossy().contains(['/', '\\']));
    }

    #[test]
    fn without_a_keychain_the_protected_file_is_used_and_found_again() {
        let dir = temp_dir();
        let layered = Layered::new(Box::new(Broken), Box::new(FileSecrets::new(dir.clone())));
        assert_eq!(layered.set("me@example.com", "tok").unwrap(), Where::File);
        assert_eq!(layered.get("me@example.com").unwrap().as_deref(), Some("tok"));
        layered.delete("me@example.com").ok();
        assert_eq!(layered.get("me@example.com").unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_working_keychain_leaves_no_file_behind() {
        let dir = temp_dir();
        let files = FileSecrets::new(dir.clone());
        files.set("me@example.com", "old").unwrap();
        let layered = Layered::new(Box::new(MemorySecrets::default()), Box::new(FileSecrets::new(dir.clone())));
        assert_eq!(layered.set("me@example.com", "new").unwrap(), Where::Memory);
        assert_eq!(files.get("me@example.com").unwrap(), None);
        assert_eq!(layered.get("me@example.com").unwrap().as_deref(), Some("new"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
