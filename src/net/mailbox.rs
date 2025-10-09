use crate::types::{MailboxRequest, MailboxResponse};
use futures::prelude::*;
use libp2p::request_response::{self, Codec, ProtocolSupport};
use std::io;

#[derive(Clone, Default)]
pub struct MailboxCodec;

impl MailboxCodec {
    pub const PROTOCOL: &'static str = "/mailbox/1.0.0";
}

#[async_trait::async_trait]
impl Codec for MailboxCodec {
    type Protocol = &'static str;
    type Request = MailboxRequest;
    type Response = MailboxResponse;

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

pub type MailboxBehaviour = request_response::Behaviour<MailboxCodec>;

pub fn create_mailbox_behaviour() -> MailboxBehaviour {
    use std::time::Duration;

    let config = request_response::Config::default().with_request_timeout(Duration::from_secs(2)); // Much faster timeout

    request_response::Behaviour::new([(MailboxCodec::PROTOCOL, ProtocolSupport::Full)], config)
}
