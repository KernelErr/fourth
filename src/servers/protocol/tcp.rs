use crate::servers::protocol::tls::get_sni;
use crate::servers::Proxy;
use futures::future::try_join;
use log::{debug, error, warn};
use std::sync::Arc;
use tokio::io;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub async fn proxy(config: Arc<Proxy>) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(config.listen).await?;
    let config = config.clone();

    loop {
        let thread_proxy = config.clone();
        match listener.accept().await {
            Err(err) => {
                error!("Failed to accept connection: {}", err);
                return Err(Box::new(err));
            }
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    match accept(stream, thread_proxy).await {
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

async fn accept(inbound: TcpStream, proxy: Arc<Proxy>) -> Result<(), Box<dyn std::error::Error>> {
    debug!("New connection from {:?}", inbound.peer_addr()?);

    let upstream_name = match proxy.tls {
        false => proxy.default.clone(),
        true => {
            let mut hello_buf = [0u8; 1024];
            inbound.peek(&mut hello_buf).await?;
            let snis = get_sni(&hello_buf);
            if snis.is_empty() {
                proxy.default.clone()
            } else {
                match proxy.sni.clone() {
                    Some(sni_map) => {
                        let mut upstream = proxy.default.clone();
                        for sni in snis {
                            let m = sni_map.get(&sni);
                            if m.is_some() {
                                upstream = m.unwrap().clone();
                                break;
                            }
                        }
                        upstream
                    }
                    None => proxy.default.clone(),
                }
            }
        }
    };

    debug!("Upstream: {}", upstream_name);

    let upstream = match proxy.upstream.get(&upstream_name) {
        Some(upstream) => upstream,
        None => {
            warn!(
                "No upstream named {:?} on server {:?}",
                proxy.default, proxy.name
            );
            return process(inbound, &proxy.default).await;
        }
    };
    return process(inbound, upstream).await;
}

async fn process(mut inbound: TcpStream, upstream: &str) -> Result<(), Box<dyn std::error::Error>> {
    if upstream == "ban" {
        let _ = inbound.shutdown();
        return Ok(());
    } else if upstream == "echo" {
        let (mut ri, mut wi) = io::split(inbound);
        let inbound_to_inbound = copy(&mut ri, &mut wi);
        let bytes_tx = inbound_to_inbound.await;
        debug!("Bytes read: {:?}", bytes_tx);
        return Ok(());
    }

    let outbound = TcpStream::connect(upstream).await?;

    let (mut ri, mut wi) = io::split(inbound);
    let (mut ro, mut wo) = io::split(outbound);

    let inbound_to_outbound = copy(&mut ri, &mut wo);
    let outbound_to_inbound = copy(&mut ro, &mut wi);

    let (bytes_tx, bytes_rx) = try_join(inbound_to_outbound, outbound_to_inbound).await?;

    debug!("Bytes read: {:?} write: {:?}", bytes_tx, bytes_rx);

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
