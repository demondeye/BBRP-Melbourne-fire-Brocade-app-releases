//! Local-only login used until Firebase Auth is added.
//! One account, stored on this PC. This keeps casual visitors out — it is
//! NOT real security. Replace these functions with real auth calls later.

use crate::storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub hash: String,
}

fn path() -> std::path::PathBuf {
    storage::data_dir().join("account.json")
}

fn hash(text: &str) -> String {
    let digest = Sha256::digest(format!("va:{text}").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn get_account() -> Option<Account> {
    storage::load(&path())
}

pub fn create_account(name: &str, password: &str) -> String {
    storage::save(&path(), &Account { name: name.to_string(), hash: hash(password) });
    name.to_string()
}

/// The signed-in user's name, or None if the password is wrong.
/// The user is only kept in memory, so closing the app logs out.
pub fn login(password: &str) -> Option<String> {
    let account = get_account()?;
    (account.hash == hash(password)).then_some(account.name)
}

pub fn reset_account() {
    let _ = std::fs::remove_file(path());
}
