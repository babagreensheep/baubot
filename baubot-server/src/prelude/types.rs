//! Types which are used in the [crate]

use super::*;
pub(crate) use baubot_core::broadcaster::types::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
/// Types of responses that the [crate::BauServer] could distribute.
pub enum BauServerResponse {
    /// Response from each individual recipient. This variant will only apply if the [BauMessage]
    /// was correctly constructed and sent on to the [crate::BauBot] leaving [crate::BauBot] to
    /// handle the individual payloads and errors on a per-recipient level.
    Recipient {
        recipient: String,
        response: BauResponse,
    },

    /// Data was rejected by the [crate::BauServer]
    InvalidData(SerializeError),
}

/// Handle for **sending** responses from the [crate::BauServer]
pub type BauServerResponseSender = sync::mpsc::UnboundedSender<BauServerResponse>;

/// Handle for **receiving** responses from the [crate::BauServer]
pub type BauServerResponseReceiver = sync::mpsc::UnboundedReceiver<BauServerResponse>;

#[derive(Debug)]
/// Error that arises when the [crate::BauClient] is unable to provide an adequate response.
pub enum SendError {
    /// Wrapper for [std::io::Error]. This is usually a serious error and you're fucked. Or... the
    /// [net::TcpStream] failed. Pick your poison.
    Io(std::io::Error),

    /// Supplied data was invalid and [crate::BauClient] was unable to parse it into a valid
    /// [BauMessage] for the purposes of distribution to the [crate::BauServer]
    InvalidData(SerializeError),
}

impl From<SerializeError> for SendError {
    fn from(value: SerializeError) -> Self {
        Self::InvalidData(value)
    }
}

impl From<std::io::Error> for SendError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
