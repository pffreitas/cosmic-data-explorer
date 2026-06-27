use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{CredentialRef, EngineError, Result};

pub trait CredentialStore: Send + Sync {
    fn set_password(&self, credential: &CredentialRef, password: &str) -> Result<()>;
    fn get_password(&self, credential: &CredentialRef) -> Result<Option<String>>;
    fn delete_password(&self, credential: &CredentialRef) -> Result<()>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryCredentialStore {
    passwords: Arc<Mutex<HashMap<CredentialRef, String>>>,
}

impl CredentialStore for InMemoryCredentialStore {
    fn set_password(&self, credential: &CredentialRef, password: &str) -> Result<()> {
        self.passwords
            .lock()
            .map_err(|error| EngineError::Credential(error.to_string()))?
            .insert(credential.clone(), password.to_string());
        Ok(())
    }

    fn get_password(&self, credential: &CredentialRef) -> Result<Option<String>> {
        Ok(self
            .passwords
            .lock()
            .map_err(|error| EngineError::Credential(error.to_string()))?
            .get(credential)
            .cloned())
    }

    fn delete_password(&self, credential: &CredentialRef) -> Result<()> {
        self.passwords
            .lock()
            .map_err(|error| EngineError::Credential(error.to_string()))?
            .remove(credential);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringCredentialStore;

#[cfg(target_os = "macos")]
impl CredentialStore for KeyringCredentialStore {
    fn set_password(&self, credential: &CredentialRef, password: &str) -> Result<()> {
        keyring::Entry::new(&credential.service, &credential.account)?.set_password(password)?;
        Ok(())
    }

    fn get_password(&self, credential: &CredentialRef) -> Result<Option<String>> {
        let entry = keyring::Entry::new(&credential.service, &credential.account)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn delete_password(&self, credential: &CredentialRef) -> Result<()> {
        let entry = keyring::Entry::new(&credential.service, &credential.account)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl CredentialStore for KeyringCredentialStore {
    fn set_password(&self, _credential: &CredentialRef, _password: &str) -> Result<()> {
        Err(EngineError::UnsupportedPlatform(
            "keyring credential storage is currently enabled only on macOS",
        ))
    }

    fn get_password(&self, _credential: &CredentialRef) -> Result<Option<String>> {
        Err(EngineError::UnsupportedPlatform(
            "keyring credential storage is currently enabled only on macOS",
        ))
    }

    fn delete_password(&self, _credential: &CredentialRef) -> Result<()> {
        Err(EngineError::UnsupportedPlatform(
            "keyring credential storage is currently enabled only on macOS",
        ))
    }
}
