//! Preludes for [crate::BauBot]

// Use this within the carate only
pub use crate::broadcaster::types;
#[allow(unused_imports)]
pub(crate) use log::{error, info, log, trace, warn};
pub(crate) use std::ops::Deref;
pub(crate) use std::sync::Arc;
pub(crate) use teloxide::dispatching::UpdateFilterExt;
pub(crate) use teloxide::dispatching::UpdateHandler;
pub(crate) use teloxide::prelude::*;
pub(crate) use teloxide::types::MessageId;
pub(crate) use teloxide::types::ReplyParameters;
pub(crate) use teloxide::types::User;
pub(crate) use teloxide::utils::command::BotCommands;
pub(crate) use tokio::task;

#[macro_export]
/// Message formatter
macro_rules! fmt {
    (pass $string:literal ) => {
        concat!("🥳", " ", $string)
    };
    (fail $string:literal ) => {
        concat!("😞", " ", $string)
    };
    (timeout $string:literal ) => {
        concat!("⌚", " ", $string)
    };
}

#[test]
fn macro_test() {
    let string = fmt!(pass "hello");
    println!("{string}");
    let other_string = concat!("🥳", " ", "hello");
    assert_eq!(other_string, string);
}

/// Instruct the bot to reply to a particular
pub(crate) async fn reply_message(
    bot: &Bot,
    chat_id: i64,
    message_id: i32,
    text: String,
) -> Result<Message, teloxide::RequestError> {
    let message = bot
        .send_message(ChatId(chat_id), text)
        .reply_parameters(ReplyParameters::new(MessageId(message_id)))
        .parse_mode(teloxide::types::ParseMode::Html);
    message.await
}

/// Trait for database that [crate::BauBot] is able to interact with
pub trait BauData
where
    Self: Sync + Send,
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
