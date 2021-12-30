use log::{error, info};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;

mod protocol;
use crate::config::{ParsedConfig, Upstream};
use protocol::{kcp, tcp};

#[derive(Debug)]
pub struct Server {
    pub proxies: Vec<Arc<Proxy>>,
    pub config: ParsedConfig,
}

#[derive(Debug, Clone)]
pub struct Proxy {
    pub name: String,
    pub listen: SocketAddr,
    pub protocol: String,
    pub tls: bool,
    pub sni: Option<HashMap<String, String>>,
    pub default: String,
    pub upstream: HashMap<String, Upstream>,
}

impl Server {
    pub fn new(config: ParsedConfig) -> Self {
        let mut new_server = Server {
            proxies: Vec::new(),
            config: config.clone(),
        };

        for (name, proxy) in config.servers.iter() {
            let protocol = proxy.protocol.clone().unwrap_or_else(|| "tcp".to_string());
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
                    protocol: protocol.clone(),
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
            info!(
                "Starting {} server {} on {}",
                config.protocol, config.name, config.listen
            );
            let handle = tokio::spawn(async move {
                match config.protocol.as_ref() {
                    "tcp" => {
                        let res = tcp::proxy(config.clone()).await;
                        if res.is_err() {
                            error!("Failed to start {}: {}", config.name, res.err().unwrap());
                        }
                    }
                    "kcp" => {
                        let res = kcp::proxy(config.clone()).await;
                        if res.is_err() {
                            error!("Failed to start {}: {}", config.name, res.err().unwrap());
                        }
                    }
                    _ => {
                        error!("Invalid protocol: {}", config.protocol)
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::plugins::kcp::{KcpConfig, KcpStream};
    use std::net::SocketAddr;
    use std::thread::{self, sleep};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use super::*;

    #[tokio::main]
    async fn tcp_mock_server() {
        let server_addr: SocketAddr = "127.0.0.1:54599".parse().unwrap();
        let listener = TcpListener::bind(server_addr).await.unwrap();
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2];
            let mut n = stream.read(&mut buf).await.unwrap();
            while n > 0 {
                stream.write(b"hello").await.unwrap();
                if buf.eq(b"by") {
                    stream.shutdown().await.unwrap();
                    break;
                }
                n = stream.read(&mut buf).await.unwrap();
            }
            stream.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_proxy() {
        use crate::config::Config;
        let config = Config::new("tests/config.yaml").unwrap();
        let mut server = Server::new(config.base);
        thread::spawn(move || {
            tcp_mock_server();
        });
        sleep(Duration::from_secs(1)); // wait for server to start
        thread::spawn(move || {
            let _ = server.run();
        });
        sleep(Duration::from_secs(1)); // wait for server to start

        // test TCP proxy
        let mut conn = TcpStream::connect("127.0.0.1:54500").await.unwrap();
        let mut buf = [0u8; 5];
        conn.write(b"hi").await.unwrap();
        conn.read(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        conn.shutdown().await.unwrap();

        // test TCP echo
        let mut conn = TcpStream::connect("127.0.0.1:54956").await.unwrap();
        let mut buf = [0u8; 1];
        for i in 0..=10u8 {
            conn.write(&[i]).await.unwrap();
            conn.read(&mut buf).await.unwrap();
            assert_eq!(&buf, &[i]);
        }
        conn.shutdown().await.unwrap();

        // test KCP echo
        let kcp_config = KcpConfig::default();
        let server_addr: SocketAddr = "127.0.0.1:54959".parse().unwrap();
        let mut conn = KcpStream::connect(&kcp_config, server_addr).await.unwrap();
        let mut buf = [0u8; 1];
        for i in 0..=10u8 {
            conn.write(&[i]).await.unwrap();
            conn.read(&mut buf).await.unwrap();
            assert_eq!(&buf, &[i]);
        }
        conn.shutdown().await.unwrap();

        // test KCP proxy and close mock server
        let kcp_config = KcpConfig::default();
        let server_addr: SocketAddr = "127.0.0.1:54958".parse().unwrap();
        let mut conn = KcpStream::connect(&kcp_config, server_addr).await.unwrap();
        let mut buf = [0u8; 5];
        conn.write(b"by").await.unwrap();
        conn.read(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
        conn.shutdown().await.unwrap();
    }
}
