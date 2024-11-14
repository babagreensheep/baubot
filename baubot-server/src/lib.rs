//! [BauServer] is a TCP server protocol for [BauBot]. Because fuck HTTP. Protect the connection
//! yourself, ideally through dockerized containers or fork a TLS wrapper I'll be happy to merge.
//!
//! Transactions shoud follow the following pattern:
//! - [BauClient] opens a connection to [BauServer]
//! - [BauClient] sends a payload that is serializable into a [BauMessage]. [BauClient] itself
//! checks the payload to make sure it can be transmitted.
//!     - [BauClient] rejects the request if it cannot be correctly serialized.
//! - [BauServer] constructs a [BauMessage] and sends that to [BauBot]
//!     - [BauServer] rejects the request if it cannot be correctly de-serialized.
//! - [BauBot] broadcasts the [BauMessage] to the appropriate [BauMessage::recipients]
//! - (only if response requested) [BauBot] polls the [BauMessage::recipients] for a response
//! - (only if response requested) [BauBot] receives the [BauResponse] and pipes it back to the
//! [BauServer]
//! - (only if response requested) [BauServer] notifies the [BauClient] through a
//! [BauServerResponseSender] that we received a response.
//! - (only if response requested) [BauClient] polls the [BauServerResponseReceiver] and obtains
//! the [BauServerResponse].
//! - [net::TcpStream] is closed, signifying the end of the transaction.

use baubot_core::BauBot;
pub use prelude::types::*;
pub(crate) use prelude::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

pub mod prelude;

/// [BauServer] listens for requests on the specified address, ideally following the transaction
/// protocol described in the [crate] documentation.
pub struct BauServer<Db, DbRef>
where
    Db: baubot_core::prelude::BauData + Send + Sync + 'static,
    DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
{
    _baubot: PhantomData<BauBot<Db, DbRef>>,
    listener: task::JoinHandle<()>,
}

/// Abort the listener on drop to avoid hanging processes
impl<Db, DbRef> Drop for BauServer<Db, DbRef>
where
    Db: baubot_core::prelude::BauData + Send + Sync + 'static,
    DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.listener.abort();
    }
}

impl<Db, DbRef> BauServer<Db, DbRef>
where
    Db: baubot_core::prelude::BauData + Send + Sync + 'static,
    DbRef: Deref<Target = Db> + Clone + Send + Sync + 'static,
{
    /// Creates a new [BauServer] object that adheres to the transaction protocol specified in the
    /// [crate] documentation.
    pub fn new(db: DbRef, addr: ::core::net::SocketAddr) -> Self {
        // Create baubot
        let baubot = Arc::new(BauBot::new(db));

        // Create listening thread
        let listener = task::spawn(Self::listen(baubot, addr));

        Self {
            _baubot: PhantomData,
            listener,
        }
    }

    /// Loop that listens for [net::TcpStream] connections and spawns threads to deal with them.
    async fn listen(baubot: Arc<BauBot<Db, DbRef>>, addr: ::core::net::SocketAddr) {
        // Create TCP listener
        // NOTE: Init tasks should unwrap
        let tcp_listener = net::TcpListener::bind(addr).await.unwrap();

        // Debug
        trace!("Server listening on {:?}", tcp_listener.local_addr());

        // Loop
        loop {
            match tcp_listener.accept().await {
                Ok(ok) => {
                    task::spawn(Self::incoming_handler(baubot.clone(), ok));
                }
                Err(err) => error!("Unable to accept connection: {err:?}"),
            }
        }
    }

    async fn incoming_handler(
        baubot: Arc<BauBot<Db, DbRef>>,
        (mut tcp_stream, socket_addr): (net::TcpStream, std::net::SocketAddr),
    ) -> std::io::Result<()> {
        let _ = baubot;
        trace!("Received connection from {socket_addr:?}");

        // Receive stream
        let request = read_stream(&tcp_stream).await?;
        trace!("Received request: {request}");

        // Pass off to baubot notification
        let baubot_response_receivers = Self::notify_baubot(baubot, request);

        // Check the status of the baubot_response_recievers
        match baubot_response_receivers {
            // If baubot managed to assemble a set of receivers:
            Ok(responses) => Self::await_baubot_responses(&tcp_stream, responses).await?,

            // If there was a serialization error, report it
            Err(err) => {
                let err = BauServerResponse::InvalidData(err);

                // NOTE: Safe to unwrap becausee we have checked the serialization pipeline
                let err = serde_json::to_string(&err).unwrap();
                write_stream(&tcp_stream, &err).await?;
            }
        }

        // close the TCP connection
        trace!("Shutting stream down");
        tcp_stream.shutdown().await
    }

    fn notify_baubot(
        baubot: Arc<BauBot<Db, DbRef>>,
        request: String,
    ) -> Result<Vec<(String, BauResponseReceiver)>, SerializeError> {
        let mut bau_message = BauMessage::builder(&request)?();
        let mut baubot_responses = Vec::new();

        // If payload requries a response, create handlers
        if !bau_message.responses.keyboard.is_empty() {}
        for (recipient, baubot_response_sender_field) in bau_message.recipients.iter_mut() {
            let (baubot_response_sender, baubot_response_receiver) = sync::oneshot::channel();
            baubot_responses.push((recipient.clone(), baubot_response_receiver));

            *baubot_response_sender_field = Some(baubot_response_sender);
        }

        // NOTE: Safe to unwrap because if the baubot has died... this entire thing is dogshit
        baubot.send(bau_message).unwrap();

        Ok(baubot_responses)
    }

    async fn await_baubot_responses(
        tcp_stream: &net::TcpStream,
        responses: Vec<(String, BauResponseReceiver)>,
    ) -> std::io::Result<()> {
        // Create iterator over responses that returns a future
        let responses = responses.into_iter().map(|(recipient, receiver)| async {
            let receiver = receiver.await;
            trace!("Recieved response from baubot: {receiver:?}");

            let response = match receiver {
                // This indicates a good response
                Ok(response) => BauServerResponse::Recipient {
                    recipient,
                    response,
                },

                // If the receiver errors out, it means that a timeout has occured
                Err(_) => BauServerResponse::Recipient {
                    recipient,
                    response: Err(BauBotError::Timeout),
                },
            };

            // NOTE: Safe to unwrap because we checked the serialization chain
            let response = serde_json::to_string(&response).unwrap();

            write_stream(&tcp_stream, &response).await
        });

        // Drive responses
        for response in responses {
            response.await?;
        }

        Ok(())
    }
}

/// [BauClient] is a helper to connect to a [BauServer] through the transaction protocol
/// described in the [crate] documentation
pub struct BauClient<const RETRIES: usize> {
    addr: ::core::net::SocketAddr,
}

/// [BauClient] connects to a [BauServer]
impl<const RETRIES: usize> BauClient<RETRIES> {
    /// Create a new [BauClient] to which adheres to the transaction protocol described in the
    /// [crate] documentation.
    pub fn new(addr: ::core::net::SocketAddr) -> Self {
        Self { addr }
    }

    /// Creates a [net::TcpStream]. Opaque to the end user.
    async fn connect(&self) -> std::io::Result<net::TcpStream> {
        // Set an attempt counter
        let mut attempt = RETRIES;

        // Try to connect until attempts run out
        loop {
            // Debug
            trace!(
                "Attempt #{} to connect to {:?}",
                RETRIES - attempt,
                self.addr
            );

            // Actual attempted connection
            match net::TcpStream::connect(self.addr.clone()).await {
                // If successful, return the connection
                Ok(connection) => {
                    trace!("Succesful connection");
                    break Ok(connection);
                }

                // If unsuccessful, decrement the attempt counter or break as necssary
                Err(err) => {
                    // Decrement attempt counter if we have not reached 0
                    if attempt > 0 {
                        attempt -= 1;
                        continue;
                    }
                    // Break
                    else {
                        trace!("Connection err: {err}");
                        break Err(err);
                    }
                }
            }
        }
    }

    /// Sends a [String] payload through the [BauClient] to the [BauServer] and returns a
    /// [BauServerResponseReceiver] that we can poll for responses.
    pub async fn send_string(
        &self,
        request: String,
    ) -> Result<BauServerResponseReceiver, SendError> {
        // Create stream
        let tcp_stream = self.connect().await?;

        // Sanity test the input
        trace!("Checking request for validity");
        let _ = BauMessage::builder(&request)?;

        // Write to the stream
        trace!("Attemping to send request to stream: {request}");
        let _ = write_stream(&tcp_stream, &request).await?;

        // Create senders and receivers and send them away with the tcp_stream
        let (bau_response_sender, bau_response_receiver) = sync::mpsc::unbounded_channel();
        task::spawn(Self::receive_responses(tcp_stream, bau_response_sender));

        Ok(bau_response_receiver)
    }

    /// Sends a [BauMessage] through the [BauClient] to the [BauServer] and returns a
    /// [BauServerResponseReceiver] that we can poll for responses.
    ///
    /// **Note**: Caller should **not** provide any handlers. They will be ignored.
    pub async fn send(
        &self,
        bau_message: BauMessage,
    ) -> Result<BauServerResponseReceiver, SendError> {
        // Sanity test the input
        let bau_message = serde_json::to_string(&bau_message).map_err(|err| {
            SendError::InvalidData(SerializeError::InvalidJson(format! {"{err:?}"}))
        })?;

        self.send_string(bau_message).await
    }

    /// Loop to receive responses from the [BauServer] and send responses to the
    /// [BauServerResponseReceiver]. Opaque to the consumer.
    async fn receive_responses(
        tcp_stream: net::TcpStream,
        bau_response_sender: BauServerResponseSender,
    ) {
        trace!("Waiting for responses from BauServer");

        // If the stream is closed read_stream will return an error (which we can discard since we
        // dont care).
        while let Ok(response) = read_stream(&tcp_stream).await {
            trace!("Received payload: {response}");

            // Break if empty response received because that means the stream closed
            if response.is_empty() {
                break;
            }

            // Check if we are able to construct a response from the server
            if let Ok(response) = serde_json::from_str::<BauServerResponse>(&response) {
                trace!("Sending response to receiver.");

                if let Err(_) = bau_response_sender.send(response) {
                    error!("Reciever went out of scope.");
                    break;
                }
            };
        }

        trace!("Finished receiving responses from BauServer.");
    }
}

#[cfg(test)]
mod tests;
