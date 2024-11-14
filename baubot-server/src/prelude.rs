//! Prelude definitiions

#[allow(unused_imports)]
pub(crate) use log::{debug, error, info, trace, warn};
pub(crate) use serde::Deserialize;
pub(crate) use serde::Serialize;
pub(crate) use tokio::net;
pub(crate) use tokio::sync;
pub(crate) use tokio::task;

pub mod types;

/// Helper to read from a [net::TcpStream]
pub(crate) async fn read_stream(tcp_stream: &net::TcpStream) -> std::io::Result<String> {
    // Create buffer
    let mut buffer = Vec::with_capacity(1024 * 4);

    // Read into buffer
    loop {
        // Wait for stream to be readable
        tcp_stream.readable().await?;
        trace!("Stream readable");

        // NOTE: Slow clap you moron this should be *try_read_buf* and not *try_read*
        match tcp_stream.try_read_buf(&mut buffer) {
            // O means... stream closed??
            Ok(0) => {
                trace!("Stream closed.");
                break;
            }

            // Reading `n` tells us that `n` bytes were read; we then cycle the loop again to
            // receive more bytes
            Ok(n) => {
                trace!("{n} bytes of data received.");
                continue;
            }

            // Would block error means we have finished reading and please unblock
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                trace!("WouldBlock");
                break;
            }

            // Return IO error up the stack
            Err(e) => Err(e)?,
        }
    }

    trace!("Finished receiving, returning payload");
    Ok(String::from_utf8(buffer).unwrap_or_default())
}

/// Helper to write to a [net::TcpStream]
pub(crate) async fn write_stream(
    tcp_stream: &net::TcpStream,
    payload: &String,
) -> std::io::Result<usize> {
    // Convert string to buffer
    let buf = payload.as_bytes();

    loop {
        // Check if stream is writable
        tcp_stream.writable().await?;
        trace!("Stream writable");

        match tcp_stream.try_write(buf) {
            Ok(n) => {
                trace!("Wrote {n} bytes to stream ");
                break Ok(n);
            }

            // Would block error suggest a false positive on the writable check and we should
            // restart please
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }

            // Other errors should bubble
            Err(e) => break Err(e),
        }
    }
}
