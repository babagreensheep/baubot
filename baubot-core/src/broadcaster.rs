//! Module containing that [Server] that listens for [types::BauMessage] and broadcasts it to the
//! correct user.

use crate::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use teloxide::payloads::SendMessageSetters;
use teloxide::types::InlineKeyboardButton;
use teloxide::types::InlineKeyboardMarkup;
use teloxide::types::MaybeInaccessibleMessage;
use teloxide::types::MessageId;
use teloxide::types::UpdateKind;
use tokio::sync::oneshot;
use tokio::sync::Mutex;

pub mod types;

pub(crate) struct Server {
    store: Mutex<types::BauResponseStore>,
}

impl Server {
    /// Start the receiver
    pub(crate) fn new() -> Self {
        // Create callback handlers
        let store = Default::default();

        // Create receiver
        Self { store }
    }

    /// Listening loop
    pub(crate) fn listen<
        Db: BauData + Send + Sync,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    >(
        server: Arc<Self>,
        bot: Bot,
        db: DbRef,
        mut server_socket: types::ServerSocket,
    ) -> impl Future<Output = ()> + Send + 'static {
        async move {
            info!("Starting receiver");

            loop {
                match server_socket.recv().await {
                    // If we receive a payload
                    Some(payload) => {
                        Self::client_request_handler(
                            server.clone(),
                            bot.clone(),
                            db.clone(),
                            payload,
                        )
                        .await
                    }

                    // Sender has gone out of scope; break the loop
                    None => break,
                };
            }

            warn!("Shutting down receiver");
        }
    }

    /// Handler
    fn client_request_handler<
        Db: BauData + Send + Sync,
        DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
    >(
        server: Arc<Self>,
        bot: Bot,
        db: DbRef,
        bau_message: types::BauMessage,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        trace!("Payload received");

        async move {
            // Deconstruct message
            let types::BauMessage {
                sender: _,
                recipients,
                message,
                responses,
            } = bau_message;

            // Convert responses into keyboard
            let responses = responses
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|field| InlineKeyboardButton::callback(field.clone(), field.clone()))
                        .collect()
                })
                .collect::<Vec<Vec<_>>>();

            // Run through each recipient
            for (recipient, client_response_sender) in recipients {
                // Get chat_id
                let chat_id = db.get_chat_id(&recipient).await;

                // Attempt to send the message
                let send_attempt = Self::message_sender(
                    bot.clone(),
                    chat_id.clone(),
                    message.clone(),
                    responses.clone(),
                )
                .await;

                // These next steps apply only if a bau_response_sender was provided and a response
                // is required
                if let (Some(chat_id), Some((client_response_sender, timeout)), false) =
                    (chat_id, client_response_sender, responses.is_empty())
                {
                    tokio::task::spawn(Self::response_handler(
                        server.clone(),
                        bot.clone(),
                        chat_id,
                        send_attempt,
                        client_response_sender,
                        timeout,
                    ));
                }
            }
        }
    }

    /// Sends the actual message
    fn message_sender(
        bot: Bot,
        chat_id: Option<i64>,
        message: String,
        responses: Vec<Vec<InlineKeyboardButton>>,
    ) -> impl std::future::Future<Output = std::result::Result<i32, types::Error>> + Send + 'static
    {
        async move {
            // Chck if chat ID exists
            match chat_id {
                Some(chat_id) => {
                    trace!("Attempting to broadcast to {chat_id}: {message}");

                    // Send message to user
                    let mut message_sender = bot.send_message(ChatId(chat_id), message.clone());

                    // Check if keyboard responses provided
                    if !responses.is_empty() {
                        message_sender =
                            message_sender.reply_markup(InlineKeyboardMarkup::new(responses))
                    }

                    // Poll send message
                    match message_sender.await {
                        // If message succesfully sent, return the response receiver
                        Ok(message) => Some(message.id.0),

                        // Else...
                        Err(_) => None,
                    }
                }
                None => None,
            }
            .ok_or(types::Error::Uncontactable)
        }
    }

    /// Creates a i128 key out of the chat_id and the message_id by bitshifting.
    pub(crate) fn make_key(chat_id: i64, message_id: i32) -> i128 {
        let chat_id = (chat_id as i128) << 64;
        chat_id | (message_id as i128)
    }

    /// Pipe between [Server] and [crate::BauBot]
    fn response_handler(
        server: Arc<Self>,
        bot: Bot,
        chat_id: i64,
        send_attempt: std::result::Result<i32, types::Error>,
        client_response_sender: types::BauResponseSender,
        timeout: u64,
    ) -> impl std::future::Future<Output = ()> + Send {
        async move {
            // Check send_attempt
            let _ = match send_attempt {
                // Message was validly out to recipient: now we wait for a response
                Ok(message_id) => {
                    trace!("Waiting for response on message {message_id} on chat {chat_id}.");

                    // Create senders and receivers to listen for responses from baubot
                    let (bau_response_sender, bau_response_receiver) = oneshot::channel();

                    // Create key
                    let key = Self::make_key(chat_id, message_id);
                    trace!("Key for bau_response_sender: {key}.");

                    // Add message to map
                    {
                        // WARN: OBTAINING MUTEX
                        let mut guard = server.store.lock().await;
                        guard.insert(key, bau_response_sender);
                        // WARN: DROPPING MUTEX
                    }

                    // Spawn removal hook. The deletion / dropping of the receiver will cause the
                    // next poll on bau_response_receiver to fail
                    tokio::task::spawn(async move {
                        // Run a timeout
                        tokio::time::sleep(std::time::Duration::from_millis(timeout)).await;

                        // Remove response options
                        let _ = Self::remove_markup(&bot, chat_id, message_id).await;

                        // WARN: OBTAINING MUTEX
                        let mut guard = server.store.lock().await;
                        if let Some(_) = guard.remove(&key) {
                            trace!("Timeout ({timeout}ms) for {key}");

                            // Notify user of timeout
                            let message =
                                format!(crate::fmt!(timeout "Timeout ({}ms) exceeded"), timeout);
                            let _ = reply_message(&bot, chat_id, message_id, message).await;
                        };
                        // WARN: DROPPING MUTEX
                        // WARN: DROPPING RECEIVER; transaction ends here.
                    });

                    // Wait for responses from baubot
                    match bau_response_receiver.await {
                        // Respond okay if baubot sent us a respones on bau_response_receiver
                        Ok(ok) => client_response_sender.send(ok),

                        // See documentation for timeout
                        Err(_) => client_response_sender.send(Err(types::Error::Timeout)),
                    }
                }

                // Message was not validly sent out to recipient
                Err(err) => client_response_sender.send(Err(err)),
            };
        }
    }

    /// Handles [CallbackQuery]
    pub(crate) fn callback_handler(
        bot: Bot,
        server: Arc<Self>,
        (data, chat_id, message_id): (String, i64, i32),
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send
    {
        // Get key
        let key = Self::make_key(chat_id, message_id);

        // Debug
        trace!("Received response to callback for message {message_id}: {data} [key: {key}].",);

        async move {
            // Obtain sender
            let bau_response_sender = {
                // WARN: OBTAINING MUTEX
                let mut guard = server.store.lock().await;
                guard.remove(&key)
                // WARN: DROPPING MUTEX
            };

            // Check if bau_response_sender valid and prepare an appropriate response for user
            let message = match bau_response_sender {
                // Valid bau_response_sender
                Some(sender) => {
                    // Send the response
                    let _ = sender.send(Ok(data.clone()));

                    // Return text
                    format!(crate::fmt!(pass "<code>{}</code>"), data)
                }

                // Invalid bau_response_sender, most likely removed due to a timeout.
                None => {
                    format!(crate::fmt!(timeout "The recipient probably timed out 😭"))
                }
            };

            // Remove response options
            Self::remove_markup(&bot, chat_id, message_id).await?;

            // Send response to user
            reply_message(&bot, chat_id, message_id, message).await?;

            Ok(())
        }
    }

    /// Create a [crate::UpdateHandler] for the [Bot]
    pub(crate) fn callback_update() -> crate::UpdateHandler<Box<dyn std::error::Error + Send + Sync>>
    {
        Update::filter_callback_query()
            .filter_map(|update: Update| {
                let callback_query = if let UpdateKind::CallbackQuery(callback_query) = update.kind
                {
                    Some(callback_query)
                } else {
                    None
                }?;

                let data = callback_query.data?;
                let (chat_id, message_id) =
                    if let MaybeInaccessibleMessage::Regular(message) = callback_query.message? {
                        Some((message.chat.id.0, message.id.0))
                    } else {
                        None
                    }?;

                Some((data, chat_id, message_id))
            })
            .endpoint(Self::callback_handler)
    }

    /// Instruct the bot to remove markup
    async fn remove_markup(
        bot: &Bot,
        chat_id: i64,
        message_id: i32,
    ) -> Result<Message, teloxide::RequestError> {
        let mut message_edit =
            bot.edit_message_reply_markup(ChatId(chat_id), MessageId(message_id));
        message_edit.reply_markup = None;
        message_edit.await
    }
}
