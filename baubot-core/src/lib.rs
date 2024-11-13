//! # BauBot
//! Inspired by the [baudot code](https://en.wikipedia.org/wiki/Baudot_code).
//! [BauBot] is meant to simplify the plumbing around [teloxide](https://github.com/teloxide/teloxide) for use with server applications. Possible applications include:
//! - E-mail server to receive e-mails and rebroadcast them on telegram
//! - OTP login approval
//!
//! # Goals and non-goals
//! ## Goals
//! - (Asynchronously) receive push messages from a **client** application.
//! - (Asynchronously) receive responses from a **user** and send it back to the **client**
//! application.
//! - (Asynchronously) register users through a database interface.
//!
//! ## Non-goals
//! - Database implementation: users have to implement their own database and their own interface
//! in the form of [BauData].
//! - Standalone application interface (e.g. TCP server listening for updates): other parts of this
//! project may address taht.

pub(crate) use prelude::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::dispatching::{UpdateFilterExt, UpdateHandler};
use teloxide::types::{MessageId, User};
use teloxide::utils::command::BotCommands;
use tokio::task;

pub(crate) mod prelude;

pub mod broadcaster;

/// # [BauBot]
/// Call [BauBot::new] with a [BauData] database to start the server(s). A new instance of [BauBot]
/// is created that implements [Deref] to a [broadcaster::types::ClientSocket] (for sending
/// a [broadcaster::types::BauMessage]) to [BauBot].
///
/// Under the hood, calling [BauBot::new] orchestrates and wraps a number of tasks. See
/// [BauBot::new] for more information.
pub struct BauBot<
    Db: BauData + Send + Sync,
    DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
> {
    db: PhantomData<DbRef>,
    bot_server_handle: task::JoinHandle<()>,
    request_server_handle: task::JoinHandle<()>,
    client_socket: broadcaster::types::ClientSocket,
}

/// Passthrough [BauBot::client_socket] methods
impl<
        Db: BauData + Send + Sync + 'static,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    > Deref for BauBot<Db, DbRef>
{
    type Target = broadcaster::types::ClientSocket;

    fn deref(&self) -> &Self::Target {
        &self.client_socket
    }
}

/// Passthrough [BauBot::client_socket] methods
impl<
        Db: BauData + Send + Sync + 'static,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    > AsRef<broadcaster::types::ClientSocket> for BauBot<Db, DbRef>
{
    fn as_ref(&self) -> &broadcaster::types::ClientSocket {
        &self.client_socket
    }
}

/// Ensures that all threads are stopped on drop
impl<Db: BauData + Send + Sync, DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static> Drop
    for BauBot<Db, DbRef>
{
    fn drop(&mut self) {
        self.bot_server_handle.abort();
        self.request_server_handle.abort();
    }
}

impl<
        Db: BauData + Send + Sync + 'static,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    > BauBot<Db, DbRef>
{
    /// Creates a new [BauBot]. This runs a number of concurrent tasks
    /// - (test mode) Initialise environment variables and logger
    /// - Initialises a request [broadcaster::Server] to listen for requests
    /// - Initialises a [Bot] to interact with telegram
    pub fn new(db: DbRef) -> Self {
        #[cfg(test)]
        prelude::init();

        // Create sockets
        let (client_socket, server_socket) = tokio::sync::mpsc::unbounded_channel();

        // Create bot
        let bot = Bot::from_env();

        // Create server
        let request_server = Arc::new(broadcaster::Server::new());

        // Start server
        let request_server_clone = request_server.clone();
        let db_clone = db.clone();
        let bot_clone = bot.clone();
        let request_server_handle = task::spawn(async move {
            broadcaster::Server::listen(request_server_clone, bot_clone, db_clone, server_socket)
                .await;
        });

        // Create dependancy map
        let mut dependencies = DependencyMap::new();
        dependencies.insert(db);
        dependencies.insert(request_server);

        // Wrap bot server handle
        let bot_server_handle = task::spawn(async move {
            Dispatcher::builder(bot, Self::handler_builder())
                .dependencies(dependencies)
                .build()
                .dispatch()
                .await
        });

        Self {
            db: PhantomData,
            bot_server_handle,
            request_server_handle,
            client_socket,
        }
    }

    /// Build the handler schema
    fn handler_builder() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
        // Command handler
        let command = teloxide::filter_command::<Command, _>().endpoint(Self::command_handler);

        // Callback handler
        let callback = broadcaster::Server::callback_update();

        // Message handler
        let message = Update::filter_message()
            // Inject user
            .filter_map(|update: Update| update.from().cloned())
            // Inject chatId
            .filter_map(|message: Message| message.chat_id())
            // Inject messageId
            .filter_map(|message: Message| Some(message.id))
            .branch(command)
            .endpoint(Self::catch_all);

        // Overall handler?
        let master = dptree::entry().branch(callback).branch(message);

        master
    }

    /// Parse [Command] received by the Bot
    async fn command_handler(
        bot: Bot,
        ChatId(chat_id): ChatId,
        user: User,
        command: Command,
        MessageId(message_id): MessageId,
        db: DbRef,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Run command
        let outcome = match command {
            Command::Start => Self::register_user(db, chat_id, user).await,
            Command::Unregister => Self::delete_user(db, user).await,
            Command::Help => Ok(Command::descriptions().to_string()),
        }
        .unwrap_or_else(|err| format!("ERROR: {err}"));

        // Send result
        reply_message(&bot, chat_id, message_id, outcome).await?;

        Ok(())
    }

    /// Handler to register a user in the DB
    async fn register_user(db: DbRef, chat_id: i64, user: User) -> Result<String, String> {
        // Attempt to get username, reject if fail
        let username = user.username.ok_or(format!("No username supplied."))?;

        // Attempt to insert
        trace!("Attempting to register {username}");
        match db.insert_chat_id(&username, chat_id).await {
            Ok(id) => Ok(format!(
                fmt!(pass "Registered!{}\n\n{}"),
                match id {
                    Some(id) =>
                        format!(" Your old registration of <code>{id}</code> has been updated."),
                    None => "".to_string(),
                },
                "🤗 Welcome to baubot's notification system."
            )),
            Err(err) => Err(err.to_string()),
        }
    }

    /// Handler to delete a user from the DB
    async fn delete_user(db: DbRef, user: User) -> Result<String, String> {
        // Attempt to get username, reject if fail
        let username = user.username.ok_or(format!("No username supplied."))?;

        // Attempt to insert
        match db.delete_chat_id(&username).await {
            Ok(id) => Ok(format!(
                fmt!(fail "Your chat_id <code>{}</code> has been deleted"),
                id
            )),
            Err(err) => Err(err.to_string()),
        }
    }

    /// Catch-all
    async fn catch_all(
        bot: Bot,
        MessageId(message_id): MessageId,
        ChatId(chat_id): ChatId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        reply_message(
            &bot,
            chat_id,
            message_id,
            fmt!(fail "Baubot does not know how to respond to your input. <b>Baubot is a bad elf!</b>" )
                .into(),
        )
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
