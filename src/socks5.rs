use std::ops::Add;

use anyhow::{anyhow, Result};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(IntoPrimitive, TryFromPrimitive, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum Method {
    NoAuth = 0x00,
    Gssapi = 0x01,
    UserPass = 0x02,
    NoAcceptable = 0xFF,
}

#[derive(Debug)]
pub struct MethodNegotiation {
    pub methods: Vec<Method>,
}

impl MethodNegotiation {
    pub async fn parse(mut stream: impl AsyncRead + Unpin) -> Result<Self> {
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await?;
        let version = buf[0];
        if version != 0x05 {
            return Err(anyhow!("unsupported version: {}", version));
        }
        let n_methods = buf[1] as usize;
        let mut methods = vec![0u8; n_methods];
        stream.read_exact(&mut methods).await?;
        let methods = methods
            .into_iter()
            .map(|method| {
                Method::try_from(method).map_err(|_| anyhow!("invalid method: {:X}", method))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { methods })
    }
}

pub struct MethodSelectionMessage {
    pub method: Method,
}

impl MethodSelectionMessage {
    pub async fn send(&self, mut stream: impl AsyncWrite + Unpin) -> Result<()> {
        let mut buf = [0u8; 2];
        buf[0] = 0x05;
        buf[1] = self.method.into();
        stream.write_all(&buf).await?;
        Ok(())
    }
}

pub struct SocksRequest {
    pub command: RequestCommand,
    pub addr_type: AddrType,
    pub dest_addr: Vec<u8>,
    pub dest_port: u16,
}

impl SocksRequest {
    pub async fn parse(mut stream: impl AsyncRead + Unpin) -> Result<Self> {
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await?;
        let version = buf[0];
        if version != 0x05 {
            return Err(anyhow!("unsupported version: {}", version));
        }
        let command = RequestCommand::try_from(buf[1])
            .map_err(|_| anyhow!("invalid command: {:X}", buf[1]))?;
        let addr_type = AddrType::try_from(buf[3])?;
        let dest_addr = match addr_type {
            AddrType::IPv4 => {
                let mut buf = [0u8; 4];
                stream.read_exact(&mut buf).await?;
                buf.to_vec()
            }
            AddrType::DomainName => {
                // Domain name
                let mut buf = [0u8; 1];
                stream.read_exact(&mut buf).await?;
                let len = buf[0] as usize;
                let mut buf = vec![0u8; len];
                stream.read_exact(&mut buf).await?;
                buf
            }
            _ => {
                return Err(anyhow!("unsupported address type: {:?}", addr_type));
            }
        };
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await?;
        let dest_port = u16::from_be_bytes(buf);
        Ok(Self {
            command,
            addr_type,
            dest_addr,
            dest_port,
        })
    }
}

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum RequestCommand {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

#[derive(IntoPrimitive, TryFromPrimitive, Debug, Clone, Copy)]
#[repr(u8)]
pub enum AddrType {
    IPv4 = 0x01,
    DomainName = 0x03,
    IPv6 = 0x04,
}

pub struct SocksReply {
    pub reply: Reply,
    pub addr_type: AddrType,
    pub bind_addr: Vec<u8>,
    pub bind_port: u16,
}

impl SocksReply {
    pub async fn send(&self, mut stream: impl AsyncWrite + Unpin) -> Result<()> {
        let mut buf = [0u8; 4];
        buf[0] = 0x05;
        buf[1] = self.reply.into();
        buf[3] = self.addr_type.into();
        stream.write_all(&buf).await?;
        stream.write_all(&self.bind_addr).await?;
        let mut buf = [0u8; 2];
        buf.copy_from_slice(&self.bind_port.to_be_bytes());
        stream.write_all(&buf).await?;
        Ok(())
    }

    pub fn success() -> Self {
        SocksReply {
            reply: Reply::Succeeded,
            addr_type: AddrType::IPv4,
            bind_addr: vec![0, 0, 0, 0],
            bind_port: 0,
        }
    }
}

#[derive(IntoPrimitive, TryFromPrimitive, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Reply {
    Succeeded = 0x00,
    GeneralFailure = 0x01,
    ConnectionNotAllowed = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}
