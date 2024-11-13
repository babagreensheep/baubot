use super::*;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// This is a client, i.e. someone that is capable of calling [mpsc::UnboundedSender::send]
pub type ClientSocket = mpsc::UnboundedSender<BauMessage>;

/// This is a server, i.e. someone that is capable of receivong on [mpsc::UnboundedReceiver::recv]
pub type ServerSocket = mpsc::UnboundedReceiver<BauMessage>;

/// Response from the [crate::BauBot] if responses are required
pub type BauResponse = Result<String, Error>;

/// Sender for a [BauResponse]. This is a a pipeline used on two ends:
/// - Each [BauMessage] sent by a [ClientSocket] may have a [BauResponseSender] attached to a
/// recipient for the purposes of receiving a [BauResponse] to the [BauMessage]
/// - Each [BauMessage] with a [BauMessage::responses] broadcast by the [Server] will trigger a
/// [Server::callback_handler] when the recipient sends a response. the [Server::callback_handler]
/// will extract the [BauResponseSender] from the [Server::store] to furnish a [BauResponse].
pub type BauResponseSender = oneshot::Sender<BauResponse>;

/// Receiver for a [BauResponse]. This is a pipeline used on two ends:
/// - [ClientSocket] to listen for [BauMessage] coming from the [Server]
/// - [Server] to receive a [BauMessage] from the [Server::callback_handler]
pub type BauResponseReceiver = oneshot::Receiver<BauResponse>;

/// [HashMap] store of [BauMessage] which require a response (key is computed based on `chat_id << 64 |
/// message_id`)
pub type BauResponseStore = HashMap<i128, BauResponseSender>;

#[derive(Debug)]
/// Form of message to be sent to the [Server]
pub struct BauMessage {
    /// Tele username.
    ///
    /// Clients should use the [crate::BauData] trait / database to obtain the appropriate telegram
    /// username.
    pub sender: String,

    /// List of recipients and handlers for that client.
    ///
    /// Clients should use the [crate::BauData] trait / database to obtain the appropriate telegram
    /// username.
    pub recipients: Vec<(String, Option<(BauResponseSender, u64)>)>,

    /// Message to be sent.
    ///
    /// # Safety
    /// [crate::BauBot] will attempt to send the message with a Html parser. Only certain types of
    /// HTML entities are recognized so the user has to check.
    pub message: String,

    /// Expected responses, as a grid of responses. Send an empty [Vec] to indicate that no
    /// responses required.
    ///
    /// ```ignore
    /// [
    ///     ["accept", "reject"],
    ///     ["ignore"],
    /// ]
    ///
    /// ```
    pub responses: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "@error")]
/// Errors emitted by [Server] that are sent to the [ClientSocket].
pub enum Error {
    /// The [BauMessage::recipients] is uncontactable.
    /// This may happen because the recipient cannot be found in [BauData], or because [Bot] was
    /// unable to send the message.
    Uncontactable,

    ///  The pipeline for sending a response between [crate::BauBot] and [Server] has expired. This
    ///  happens in the following circumstances:
    ///  - The timeout hook was triggered.
    ///  - The [Server::callback_handler] did not use the provided [BauResponseSender] for some
    ///  reason (which should not be the case, but we will provide for the possibility anyway).
    Timeout,
}
