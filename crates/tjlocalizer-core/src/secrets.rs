//! Where an API key is kept.
//!
//! Not in project.json, and not anywhere under the project directory. A project is a folder people
//! commit, zip up and send to a translator; a key that lives in it leaks the first time anyone
//! does any of that. It goes in the application's own configuration directory instead, in a file
//! readable only by its owner.
//!
//! This is not a secret store. It is a file with tight permissions, which is what a desktop
//! application can offer without a platform keychain - and it is honest about that rather than
//! implying more.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Keys, one per endpoint.
///
/// Keyed by endpoint rather than by provider family, so pointing two projects at two deployments
/// of the same family keeps two keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keys {
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

impl Keys {
    pub fn load(config_dir: &Path) -> Self {
        std::fs::read_to_string(Self::file(config_dir))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn get(&self, endpoint: &str) -> Option<&str> {
        self.keys.get(endpoint).map(|s| s.as_str())
    }

    pub fn has(&self, endpoint: &str) -> bool {
        self.keys.contains_key(endpoint)
    }

    /// Stores a key, or removes it when the value is blank.
    pub fn set(&mut self, endpoint: &str, key: &str) {
        if key.trim().is_empty() {
            self.keys.remove(endpoint);
        } else {
            self.keys.insert(endpoint.to_string(), key.to_string());
        }
    }

    /// Which endpoints have a key. The keys themselves never leave this type by this route.
    pub fn endpoints(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    pub fn save(&self, config_dir: &Path) -> crate::Result<()> {
        std::fs::create_dir_all(config_dir)?;
        let path = Self::file(config_dir);
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        restrict(&path)?;
        Ok(())
    }

    pub fn file(config_dir: &Path) -> PathBuf {
        config_dir.join("keys.json")
    }
}

/// Makes the file readable only by its owner.
///
/// On Unix this is real. On other platforms it is a no-op, and the documentation says so rather
/// than leaving a reader to assume the file is protected everywhere.
#[cfg(unix)]
fn restrict(path: &Path) -> crate::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> crate::Result<()> {
    Ok(())
}
