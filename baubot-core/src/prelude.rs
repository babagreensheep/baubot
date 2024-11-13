//! Preludes for [crate::BauBot]

// Use this within the carate only
pub(crate) use log::{error, info, log, trace, warn};
use teloxide::macros::*;
pub(crate) use teloxide::prelude::*;

/// Trait for database that [crate::BauBot] is able to interact with
pub trait BauData
where
    Self: Default + Sync + Send,
{
    /// Get user's chat_id from the DB based on the suppled `username`.
    /// Please remember that any [String] output gets parsed by [crate::BauBot] as a Html entity.
    ///
    /// # Safety
    /// The implementation of this trait should make all necessary authentication choices at the
    /// appropriate stages (e.g. verifying that the user is allowed to receive or send requests)
    fn register_user_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Option<i64>> + Send;

    /// Insert user's `username `into the DB together with their `chat_id`.
    /// Please remember that any [String] output gets parsed by [crate::BauBot] as a Html entity.
    ///
    /// # Safety
    /// The implementation of this trait should make all necessary authentication choices at the
    /// appropriate stages (e.g. verifying that the user is allowed to receive or send requests)
    fn insert_chat_id(
        &self,
        username: &str,
        chat_id: i64,
    ) -> impl std::future::Future<Output = Result<Option<i64>, String>> + Send;

    /// Delete user's `username `into the DB together with their `chat_id`.
    /// Please remember that any [String] output gets parsed by [crate::BauBot] as a Html entity.
    ///
    /// # Safety
    /// The implementation of this trait should make all necessary authentication choices at the
    /// appropriate stages (e.g. verifying that the user is allowed to receive or send requests)
    fn delete_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Result<i64, String>> + Send;

    /// Get chat_id for `username`
    ///
    /// # Safety
    /// The implementation of this trait should make all necessary authentication choices at the
    /// appropriate stages (e.g. verifying that the user is allowed to receive or send requests)
    fn get_chat_id(&self, username: &str) -> impl std::future::Future<Output = Option<i64>> + Send;

    /// Check if `username` is an admin.
    ///
    /// # Safety
    /// The implementation of this trait should make all necessary authentication choices at the
    /// appropriate stages (e.g. verifying that the user is allowed to receive or send requests)
    fn is_admin(&self, username: &str) -> impl std::future::Future<Output = bool> + Send;
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase")]
pub(crate) enum Command {
    #[command(description = "Registers you as a user of the dobby service")]
    Start,
    #[command(description = "Unregister you as a user of the dobby service")]
    Unregister,
    #[command(description = "Get list of available commands")]
    Help,
}

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
/// Init script to initialise:
/// - Environment variables
/// - Logger
/// **Used only in test scripts** as this is a library. Any project implementing the library should
/// have its own environment init.
pub(crate) fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let manifest_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let secrets_path = manifest_path.join("tele.env");

        // Initialize env secrets
        dotenvy::from_path(secrets_path).unwrap();

        // initialise logger
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Trace)
            .filter_module("teloxide", log::LevelFilter::Off)
            .filter_module("reqwest", log::LevelFilter::Off)
            .parse_env("LOG_LEVEL")
            .init();
    });
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestDB {
    db: tokio::sync::Mutex<HashMap<String, i64>>,
}

#[cfg(test)]
impl TestDB {
    pub(crate) fn seed() -> Self {
        init();
        let user = std::env::var("TEST_USER").unwrap();
        let chat_id = std::env::var("TEST_CHATID").unwrap().parse().unwrap();
        let mut db = HashMap::new();
        db.insert(user, chat_id);
        let db = tokio::sync::Mutex::new(db);
        Self { db }
    }
}

#[cfg(test)]
impl BauData for TestDB {
    fn register_user_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Option<i64>> + Send {
        async {
            let db = self.db.lock().await;
            let entry = db.get(username)?.to_owned();
            Some(entry)
        }
    }

    fn insert_chat_id(
        &self,
        username: &str,
        chat_id: i64,
    ) -> impl std::future::Future<Output = Result<Option<i64>, String>> + Send {
        async move {
            let mut db = self.db.lock().await;
            Ok(db.insert(username.to_string(), chat_id.clone()))
        }
    }

    fn delete_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Result<i64, String>> + Send {
        async move {
            let mut db = self.db.lock().await;
            match db.remove(username) {
                Some(id) => Ok(id),
                None => Err(format!(
                    "Username <code>{username}</code> was not registered."
                )),
            }
        }
    }

    fn is_admin(&self, username: &str) -> impl std::future::Future<Output = bool> + Send {
        let _ = username;
        async { true }
    }

    fn get_chat_id(&self, username: &str) -> impl std::future::Future<Output = Option<i64>> + Send {
        async move {
            let db = self.db.lock().await;
            match db.get(username) {
                Some(id) => Some(id.clone()),
                None => None,
            }
        }
    }
}
