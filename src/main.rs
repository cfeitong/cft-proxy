use std::net::{IpAddr, SocketAddr};

use anyhow::{anyhow, bail, Result};
use socks5::{Method, MethodNegotiation, MethodSelectionMessage, SocksRequest};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use crate::socks5::SocksReply;

mod socks5;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let listener = TcpListener::bind(("0.0.0.0", 1080)).await?;
    loop {
        match listener.accept().await {
            Ok((conn, _peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(err) = on_connection(conn).await {
                        error!(error=?err, "fail to process connection");
                    }
                });
            }
            Err(err) => {
                error!(error=?err, "fail to accept connection");
            }
        }
    }
}

async fn on_connection(mut conn: TcpStream) -> Result<()> {
    let method_negotiation = MethodNegotiation::parse(&mut conn).await?;
    if !method_negotiation.methods.contains(&Method::NoAuth) {
        bail!("authentication is not supported");
    }
    let method_selection_message = MethodSelectionMessage {
        method: Method::NoAuth,
    };
    method_selection_message.send(&mut conn).await?;

    while let Ok(request) = SocksRequest::parse(&mut conn).await {
        let addr: SocketAddr = match request.addr_type {
            socks5::AddrType::IPv4 => {
                let ip: [u8; 4] = request
                    .dest_addr
                    .try_into()
                    .map_err(|_| anyhow!("invalid dest_addr"))?;
                let ip: IpAddr = ip.into();
                (ip, request.dest_port).into()
            }
            socks5::AddrType::DomainName => {
                let domain = String::from_utf8(request.dest_addr)
                    .map_err(|_| anyhow!("invalid dest_addr"))?;
                info!(domain=?domain, "domain lookup");
                tokio::net::lookup_host((domain, request.dest_port))
                    .await?
                    .next()
                    .ok_or_else(|| anyhow!("fail to resolve domain"))?
            }
            _ => bail!("unsupported address type"),
        };
        info!(addr=?addr, "dns resolve success");
        match request.command {
            socks5::RequestCommand::Connect => {
                info!(addr=?addr, "trying to connect");
                let remote = TcpStream::connect(addr)
                    .await
                    .map_err(|_| anyhow!("fail to connect to {}:{}", addr, request.dest_port))?;
                let reply = SocksReply::success();
                reply.send(&mut conn).await?;
                establish_connection(remote, &mut conn).await?;
            }
            socks5::RequestCommand::Bind => error!("bind is not supported"),
            socks5::RequestCommand::UdpAssociate => error!("udp associate is not supported"),
        }
    }
    Ok(())
}

async fn establish_connection(mut a: TcpStream, b: &mut TcpStream) -> Result<()> {
    let (mut rx_a, mut tx_a) = a.split();
    let (mut rx_b, mut tx_b) = b.split();
    let f1 = tokio::io::copy(&mut rx_a, &mut tx_b);
    let f2 = tokio::io::copy(&mut rx_b, &mut tx_a);
    tokio::try_join!(f1, f2)?;
    Ok(())
}
