use futures::future::try_join;
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

mod tls;
use self::tls::get_sni;
use crate::config::BaseConfig;

#[derive(Debug)]
pub struct Server {
    pub proxies: Vec<Arc<Proxy>>,
    pub config: BaseConfig,
}

#[derive(Debug, Clone)]
pub struct Proxy {
    pub name: String,
    pub listen: SocketAddr,
    pub tls: bool,
    pub sni: Option<HashMap<String, String>>,
    pub default: String,
    pub upstream: HashMap<String, String>,
}

impl Server {
    pub fn new(config: BaseConfig) -> Self {
        let mut new_server = Server {
            proxies: Vec::new(),
            config: config.clone(),
        };

        for (name, proxy) in config.servers.iter() {
            let tls = proxy.tls.unwrap_or(false);
            let sni = proxy.sni.clone();
            let default = proxy.default.clone().unwrap_or_else(|| "ban".to_string());
            let upstream = config.upstream.clone();
            let mut upstream_set: HashSet<String> = HashSet::new();
            for key in upstream.keys() {
                if key.eq("ban") || key.eq("echo") {
                    continue;
                }
                upstream_set.insert(key.clone());
            }
            for listen in proxy.listen.clone() {
                println!("{:?}", listen);
                let listen_addr: SocketAddr = match listen.parse() {
                    Ok(addr) => addr,
                    Err(_) => {
                        error!("Invalid listen address: {}", listen);
                        continue;
                    }
                };
                let proxy = Proxy {
                    name: name.clone(),
                    listen: listen_addr,
                    tls,
                    sni: sni.clone(),
                    default: default.clone(),
                    upstream: upstream.clone(),
                };
                new_server.proxies.push(Arc::new(proxy));
            }
        }

        new_server
    }

    #[tokio::main]
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let proxies = self.proxies.clone();
        let mut handles: Vec<JoinHandle<()>> = Vec::new();

        for config in proxies {
            info!("Starting server {} on {}", config.name, config.listen);
            let handle = tokio::spawn(async move {
                let _ = proxy(config).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }
        Ok(())
    }
}

async fn proxy(config: Arc<Proxy>) -> Result<(), Box<dyn std::error::Error>> {
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
        loop {
            let mut buf = [0u8; 1];
            let b = inbound.read(&mut buf).await?;
            if b == 0 {
                break;
            } else {
                inbound.write(&buf).await?;
            }
        }
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

#[cfg(test)]
mod test {
    use std::thread::{self, sleep};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn test_echo_server() {
        use crate::config::Config;
        let config = Config::new("tests/config.yaml").unwrap();
        let mut server = Server::new(config.base);
        thread::spawn(move || {
            let _ = server.run();
        });
        sleep(Duration::from_secs(1)); // wait for server to start
        let mut conn = TcpStream::connect("127.0.0.1:54956").await.unwrap();
        let mut buf = [0u8; 1];
        for i in 0..=255u8 {
            conn.write(&[i]).await.unwrap();
            conn.read(&mut buf).await.unwrap();
            assert_eq!(&buf, &[i]);
        }
        conn.shutdown().await.unwrap();
    }
}
