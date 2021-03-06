use crate::config::Upstream;
use crate::plugins::kcp::{KcpConfig, KcpListener, KcpStream};
use crate::servers::Proxy;
use futures::future::try_join;
use log::{debug, error, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn proxy(config: Arc<Proxy>) -> Result<(), Box<dyn std::error::Error>> {
    let kcp_config = KcpConfig::default();
    let mut listener = KcpListener::bind(kcp_config, config.listen).await?;
    let config = config.clone();

    loop {
        let thread_proxy = config.clone();
        match listener.accept().await {
            Err(err) => {
                error!("Failed to accept connection: {}", err);
                return Err(Box::new(err));
            }
            Ok((stream, peer)) => {
                tokio::spawn(async move {
                    match accept(stream, peer, thread_proxy).await {
                        Ok(_) => {}
                        Err(err) => {
                            error!("Relay thread returned an error: {}", err);
                        }
                    };
                });
            }
        }
    }
}

async fn accept(
    inbound: KcpStream,
    peer: SocketAddr,
    proxy: Arc<Proxy>,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("New connection from {:?}", peer);

    let upstream_name = proxy.default.clone();

    debug!("Upstream: {}", upstream_name);

    let upstream = match proxy.upstream.get(&upstream_name) {
        Some(upstream) => upstream,
        None => {
            warn!(
                "No upstream named {:?} on server {:?}",
                proxy.default, proxy.name
            );
            return process(inbound, proxy.upstream.get(&proxy.default).unwrap()).await;
            // ToDo: Remove unwrap and check default option
        }
    };
    return process(inbound, upstream).await;
}

async fn process(
    mut inbound: KcpStream,
    upstream: &Upstream,
) -> Result<(), Box<dyn std::error::Error>> {
    match upstream {
        Upstream::Ban => {
            let _ = inbound.shutdown();
        }
        Upstream::Echo => {
            let (mut ri, mut wi) = io::split(inbound);
            let inbound_to_inbound = copy(&mut ri, &mut wi);
            let bytes_tx = inbound_to_inbound.await;
            debug!("Bytes read: {:?}", bytes_tx);
        }
        Upstream::Custom(custom) => match custom.protocol.as_ref() {
            "tcp" => {
                let outbound = TcpStream::connect(custom.addr.clone()).await?;

                let (mut ri, mut wi) = io::split(inbound);
                let (mut ro, mut wo) = io::split(outbound);

                let inbound_to_outbound = copy(&mut ri, &mut wo);
                let outbound_to_inbound = copy(&mut ro, &mut wi);

                let (bytes_tx, bytes_rx) =
                    try_join(inbound_to_outbound, outbound_to_inbound).await?;

                debug!("Bytes read: {:?} write: {:?}", bytes_tx, bytes_rx);
            }
            _ => {
                error!("Reached unknown protocol: {:?}", custom.protocol);
            }
        },
    };
    Ok(())
}

async fn copy<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    match io::copy(reader, writer).await {
        Ok(u64) => {
            let _ = writer.shutdown().await;
            Ok(u64)
        }
        Err(_) => Ok(0),
    }
}
