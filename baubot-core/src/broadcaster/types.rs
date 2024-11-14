use super::*;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// This is a client, i.e. someone that is capable of calling [mpsc::UnboundedSender::send]
pub type ClientSocket = mpsc::UnboundedSender<BauMessage>;

/// This is a server, i.e. someone that is capable of receivong on [mpsc::UnboundedReceiver::recv]
pub type ServerSocket = mpsc::UnboundedReceiver<BauMessage>;

/// Response from the [crate::BauBot] if responses are required
pub type BauResponse = Result<String, BauBotError>;

/// Sender for a [BauResponse]. This is a a pipeline used on two ends:
/// - Each [BauMessage] sent by a [ClientSocket] may have a [BauResponseSender] attached to a
/// recipient for the purposes of receiving a [BauResponse] to the [BauMessage]
/// - Each [BauMessage] with a [BauMessage::responses] sent by the [ServerSocket] to [crate::BauBot]
/// will trigger a query handler on [crate::BauBot] when the intended recipient sends a response. The
/// callback handler will extract a [BauResponseSender] from the [BauResponseStore] to send an
/// appropriate [BauResponse]
pub type BauResponseSender = oneshot::Sender<BauResponse>;

/// Receiver for a [BauResponse]. This is used by two pipelines:
/// - When the [ServerSocket] is waiting for a [BauResponse] from [crate::BauBot].
/// - When the [ServerSocket] wants to send the [BauResponse] to the [ClientSocket]
pub type BauResponseReceiver = oneshot::Receiver<BauResponse>;

/// [HashMap] store of [BauMessage] which require a response (key is computed based on `chat_id << 64 |
/// message_id`)
pub type BauResponseStore = HashMap<i128, BauResponseSender>;

#[derive(Debug)]
/// Form of message that can be passed between various interfaces (e.g. [ServerSocket],
/// [crate::BauBot] and [ClientSocket]).
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
    pub recipients: Vec<(String, Option<BauResponseSender>)>,

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
    pub responses: RequestedResponses,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RequestedResponses {
    pub timeout: u64,
    pub keyboard: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
/// Errors emitted by [ServerSocket] that are sent to the [ClientSocket].
pub enum BauBotError {
    /// The [BauMessage::recipients] is uncontactable.
    /// This may happen because the recipient cannot be found in [BauData], or because [Bot] was
    /// unable to send the message.
    Uncontactable,

    ///  The pipeline for sending a response between [crate::BauBot] and [ServerSocket] has expired. This
    ///  happens in the following circumstances:
    ///  - The timeout hook was triggered.
    ///  - The request server did not use the provided [BauResponseSender] for some reason (which
    ///  should not be the case, but we will provide for the possibility anyway).
    Timeout,
}

use serde_json::Value;

impl BauMessage {
    /// Helper to build a [BauMessage] from a JSON string. returns a callback with a properly
    /// constructed [BauMessage] albeit that [BauMessage::recipients] will have an empty list of
    /// [BauResponseSender] by default.
    pub fn builder(
        string: &str,
    ) -> std::result::Result<impl FnOnce() -> BauMessage, SerializeError> {
        // Attempt to create json value
        let mut json_value = serde_json::from_str::<Value>(string)?;

        // Extract sender
        let sender = serde_json::from_value(json_value.get_mut("sender").ok_or("sender")?.take())?;

        // Extract recipients
        let recipients = serde_json::from_value::<Vec<String>>(
            json_value.get_mut("recipients").ok_or("recipients")?.take(),
        )?
        .into_iter()
        .map(|recipient| (recipient, None))
        .collect();

        // Extract message
        let message =
            serde_json::from_value(json_value.get_mut("message").ok_or("message")?.take())?;

        // Extract responses
        let responses = match json_value.get_mut("responses") {
            Some(value) => {
                let value = value.take();
                serde_json::from_value(value)?
            }
            None => RequestedResponses::default(),
        };

        // Return callback
        Ok(move || BauMessage {
            sender,
            recipients,
            message,
            responses,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SerializeError {
    InvalidJson(String),
    InvalidField(String),
}

impl From<serde_json::Error> for SerializeError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidJson(format!("{value:?}"))
    }
}

impl From<&'static str> for SerializeError {
    fn from(value: &'static str) -> Self {
        Self::InvalidField(value.into())
    }
}

#[test]
fn successful_conversion() {
    let message = BauMessage::builder(
        r#"{
    "sender": "sender",
    "recipients": [
        "recipient"
    ],
    "message": "hello world",
    "responses": { 
        "timeout": 5000,
        "keyboard": [
            [
                "hi",
                "bye"
            ],
            [ "fuck off" ]
        ] 
    }
}"#,
    )
    .unwrap()();

    println!("{message:#?}");
    assert_eq!(message.sender, "sender");
    assert_eq!(message.responses.keyboard.len(), 2);
}

#[test]
fn missing_responses() {
    let message = BauMessage::builder(
        r#"{
    "sender": "sender",
    "recipients": [
        "recipient"
    ],
    "message": "hello world"
}"#,
    )
    .unwrap()();

    println!("{message:#?}");
    assert_eq!(message.sender, "sender");
    assert_eq!(message.responses.keyboard.len(), 0);
}

#[test]
fn invalid_responses() {
    let message = BauMessage::builder(
        r#"{
    "sender": "sender",
    "recipients": [
        "recipient"
    ],
    "message": "hello world",
    "responses": "boo"
}"#,
    );

    if let Err(err) = &message {
        println!("{err:#?}")
    }
    assert!(message.is_err());
}

#[test]
fn missing_sender() {
    let message = BauMessage::builder(
        r#"{
    "recipients": [
        "recipient"
    ],
    "message": "hello world",
    "responses": { 
        "timeout": 5000,
        "keyboard": [
            [
                "hi",
                "bye"
            ],
            [ "fuck off" ]
        ] 
    }
}"#,
    );

    if let Err(err) = &message {
        println!("{err:#?}")
    }
    assert!(message.is_err());
}

#[test]
fn invalid_recipients() {
    let message = BauMessage::builder(
        r#"{
    "sender": "sender",
    "recipients": "recipient",
    "message": "hello world",
    "responses": { 
        "timeout": 5000,
        "keyboard": [
            [
                "hi",
                "bye"
            ],
            [ "fuck off" ]
        ] 
    }
}"#,
    );

    if let Err(err) = &message {
        println!("{err:#?}")
    }
    assert!(message.is_err());
}
