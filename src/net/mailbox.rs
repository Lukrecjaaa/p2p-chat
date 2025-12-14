//! This module defines the codec for the mailbox protocol, which is used for
//! interacting with mailbox nodes.
use crate::types::{MailboxRequest, MailboxResponse};
use futures::prelude::*;
use libp2p::request_response::{self, Codec, ProtocolSupport};
use std::io;

/// The codec for the mailbox protocol.
///
/// This codec is used by the `libp2p` `request_response` behaviour to encode
/// and decode mailbox requests and responses.
#[derive(Clone, Default)]
pub struct MailboxCodec;

impl MailboxCodec {
    /// The protocol name for the mailbox protocol.
    pub const PROTOCOL: &'static str = "/mailbox/1.0.0";
}

#[async_trait::async_trait]
impl Codec for MailboxCodec {
    type Protocol = &'static str;
    type Request = MailboxRequest;
    type Response = MailboxResponse;

    /// Reads a length-prefixed JSON-encoded request from the given I/O stream.
    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        let mut data = vec![0u8; length];
        io.read_exact(&mut data).await?;

        serde_json::from_slice(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Reads a length-prefixed JSON-encoded response from the given I/O stream.
    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut length_buf = [0u8; 4];
        io.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        let mut data = vec![0u8; length];
        io.read_exact(&mut data).await?;

        serde_json::from_slice(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Writes a length-prefixed JSON-encoded request to the given I/O stream.
    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data =
            serde_json::to_vec(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let length = data.len() as u32;

        io.write_all(&length.to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }

    /// Writes a length-prefixed JSON-encoded response to the given I/O stream.
    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data =
            serde_json::to_vec(&res).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let length = data.len() as u32;

        io.write_all(&length.to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.flush().await?;
        Ok(())
    }
}

/// The `libp2p` `request_response` behaviour for the mailbox protocol.
pub type MailboxBehaviour = request_response::Behaviour<MailboxCodec>;

/// Creates a new `MailboxBehaviour`.
pub fn create_mailbox_behaviour() -> MailboxBehaviour {
    use std::time::Duration;

    let config = request_response::Config::default().with_request_timeout(Duration::from_secs(2));

    request_response::Behaviour::new([(MailboxCodec::PROTOCOL, ProtocolSupport::Full)], config)
}
