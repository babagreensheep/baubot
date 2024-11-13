//! Core of [BauBot].
//! [BauBot] is meant to simplify the plumbing around [teloxide](https://github.com/teloxide/teloxide) for use with server applications. Possible applications include:
//! - E-mail server to receive e-mails and rebroadcast them on telegram
//! - OTP login approval

pub(crate) use prelude::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::dispatching::{UpdateFilterExt, UpdateHandler};
use teloxide::types::User;
use teloxide::utils::command::BotCommands;
use tokio::task;

pub(crate) mod prelude;

pub mod broadcaster;

#[tokio::test]
async fn integration_test() {
    // Create baubot
    let bau_bot = BauBot::new(Arc::new(TestDB::seed()));
    let test_user = std::env::var("TEST_USER").unwrap();

    // Send non-responding message without response sender
    info!("Sending notification message");
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), None)],
        message: "Message!".to_string(),
        responses: Vec::new(),
    });

    // Send non-responding message with response sender
    info!("Sending notification message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Message (waiting on response)!".to_string(),
        responses: Vec::new(),
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send response-required message
    info!("Sending response-required message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send two response-required messsages
    info!("Two response-required message");
    let (bau_response_sender, bau_response_receiver00) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let (bau_response_sender, bau_response_receiver01) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let responses = tokio::join!(bau_response_receiver00, bau_response_receiver01);
    println!("{responses:#?}");

    info!("waiting for app exit");
    let _ = tokio::signal::ctrl_c().await;
}

/// # [BauBot]
/// Call [BauBot::new] with a [BauData] database to start the server(s). A new instance of [BauBot]
/// is created that implements [Deref] to a [broadcaster::types::ClientSocket] (for sending
/// a [broadcaster::types::BauMessage]) to [BauBot].
///
/// Under the hood, calling [BauBot::new] orchestrates and wraps a number of tasks. See
/// [BauBot::new] for more information.
pub struct BauBot<
    Db: BauData + Send + Sync + 'static,
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
impl<
        Db: BauData + Send + Sync + 'static,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    > Drop for BauBot<Db, DbRef>
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
        let request_server = Arc::new(broadcaster::Server::new(db.clone(), bot.clone()));

        // Start server
        let clone = request_server.clone();
        let request_server_handle = task::spawn(async move {
            broadcaster::Server::listen(clone, server_socket).await;
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
        let callback = broadcaster::Server::<DbRef, Db>::callback_update();

        // Message handler
        let message = Update::filter_message()
            // Inject user
            .filter_map(|update: Update| update.from().cloned())
            // Inject chatId
            .filter_map(|message: Message| message.chat_id())
            .branch(command)
            .endpoint(Self::catch_all);

        // Overall handler?
        let master = dptree::entry().branch(callback).branch(message);

        master
    }

    /// Parse [Command] received by the Bot
    async fn command_handler(
        bot: Bot,
        chat_id: ChatId,
        user: User,
        command: Command,
        db: DbRef,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let User { first_name, .. } = &user;

        // Send greeting
        bot.send_message(chat_id, format!("Hello {first_name}"))
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        // Run command
        let outcome = match command {
            Command::Start => Self::register_user(db, chat_id, user).await,
            Command::Unregister => Self::delete_user(db, user).await,
            Command::Help => Ok(Command::descriptions().to_string()),
        }
        .unwrap_or_else(|err| format!("ERROR: {err}"));

        // Send result
        bot.send_message(chat_id, outcome)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        Ok(())
    }

    /// Handler to register a user in the DB
    async fn register_user(db: DbRef, chat_id: ChatId, user: User) -> Result<String, String> {
        // Attempt to get username, reject if fail
        let username = user.username.ok_or(format!("No username supplied."))?;

        // Attempt to insert
        trace!("Attempting to register {username}");
        match db.insert_chat_id(&username, chat_id.0).await {
            Ok(id) => Ok(format!(
                "  Registered!{}\n\n{}",
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
                "  Your chat_id <code>{id}</code> has been deleted"
            )),
            Err(err) => Err(err.to_string()),
        }
    }

    /// Catch-all
    async fn catch_all(
        bot: Bot,
        message: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        bot.send_message(
            message.chat.id,
            "  Baubot does not know how to respond to your input. <b>Baubot is a bad elf!</b>",
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;

        Ok(())
    }
}
