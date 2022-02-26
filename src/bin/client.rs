use std::net::SocketAddr;

use anyhow::Result;
use cft_proxy::{ObfucationAsyncReader, ObfucationAsyncWriter};
use clap::Parser;
use tokio::net::{TcpListener, TcpStream};
use tracing::error;

#[derive(Parser)]
struct Args {
    #[clap(long)]
    server: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let arg: Args = Args::parse();
    let listener = TcpListener::bind(("0.0.0.0", 1091)).await?;
    while let Ok((mut conn, _)) = listener.accept().await {
        let server_addr = arg.server.clone();
        let f = async move {
            let mut remote = TcpStream::connect(server_addr).await?;
            let (server_rx, server_tx) = remote.split();
            let mut server_rx = ObfucationAsyncReader::new(server_rx);
            let mut server_tx = ObfucationAsyncWriter::new(server_tx);
            let (mut client_rx, mut client_tx) = conn.split();
            let f1 = tokio::io::copy(&mut server_rx, &mut client_tx);
            let f2 = tokio::io::copy(&mut client_rx, &mut server_tx);
            tokio::try_join!(f1, f2)?;
            Ok(()) as anyhow::Result<_>
        };
        tokio::spawn(async move {
            if let Err(err) = f.await {
                error!(error=?err, "fail to connect to server");
            }
        });
    }
    Ok(())
}
