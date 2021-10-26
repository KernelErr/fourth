use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use byte_string::ByteStr;
use kcp::{Error as KcpError, KcpResult};
use log::{debug, error, trace};
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    sync::mpsc,
    task::JoinHandle,
    time,
};

use crate::plugins::kcp::{config::KcpConfig, session::KcpSessionManager, stream::KcpStream};

#[allow(unused)]
pub struct KcpListener {
    udp: Arc<UdpSocket>,
    accept_rx: mpsc::Receiver<(KcpStream, SocketAddr)>,
    task_watcher: JoinHandle<()>,
}

impl Drop for KcpListener {
    fn drop(&mut self) {
        self.task_watcher.abort();
    }
}

impl KcpListener {
    pub async fn bind<A: ToSocketAddrs>(config: KcpConfig, addr: A) -> KcpResult<KcpListener> {
        let udp = UdpSocket::bind(addr).await?;
        let udp = Arc::new(udp);
        let server_udp = udp.clone();

        let (accept_tx, accept_rx) = mpsc::channel(1024 /* backlogs */);
        let task_watcher = tokio::spawn(async move {
            let (close_tx, mut close_rx) = mpsc::channel(64);

            let mut sessions = KcpSessionManager::new();
            let mut packet_buffer = [0u8; 65536];
            loop {
                tokio::select! {
                    conv = close_rx.recv() => {
                        let conv = conv.expect("close_tx closed unexpectly");
                        sessions.close_conv(conv);
                        trace!("session conv: {} removed", conv);
                    }

                    recv_res = udp.recv_from(&mut packet_buffer) => {
                        match recv_res {
                            Err(err) => {
                                error!("udp.recv_from failed, error: {}", err);
                                time::sleep(Duration::from_secs(1)).await;
                            }
                            Ok((n, peer_addr)) => {
                                let packet = &mut packet_buffer[..n];

                                log::trace!("received peer: {}, {:?}", peer_addr, ByteStr::new(packet));

                                let mut conv = kcp::get_conv(packet);
                                if conv == 0 {
                                    // Allocate a conv for client.
                                    conv = sessions.alloc_conv();
                                    debug!("allocate {} conv for peer: {}", conv, peer_addr);

                                    kcp::set_conv(packet, conv);
                                }

                                let session = match sessions.get_or_create(&config, conv, &udp, peer_addr, &close_tx) {
                                    Ok((s, created)) => {
                                        if created {
                                            // Created a new session, constructed a new accepted client
                                            let stream = KcpStream::with_session(s.clone());
                                            if let Err(..) = accept_tx.try_send((stream, peer_addr)) {
                                                debug!("failed to create accepted stream due to channel failure");

                                                // remove it from session
                                                sessions.close_conv(conv);
                                                continue;
                                            }
                                        }

                                        s
                                    },
                                    Err(err) => {
                                        error!("failed to create session, error: {}, peer: {}, conv: {}", err, peer_addr, conv);
                                        continue;
                                    }
                                };

                                // let mut kcp = session.kcp_socket().lock().await;
                                // if let Err(err) = kcp.input(packet) {
                                //     error!("kcp.input failed, peer: {}, conv: {}, error: {}, packet: {:?}", peer_addr, conv, err, ByteStr::new(packet));
                                // }
                                session.input(packet).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(KcpListener {
            udp: server_udp,
            accept_rx,
            task_watcher,
        })
    }

    pub async fn accept(&mut self) -> KcpResult<(KcpStream, SocketAddr)> {
        match self.accept_rx.recv().await {
            Some(s) => Ok(s),
            None => Err(KcpError::IoError(io::Error::new(
                ErrorKind::Other,
                "accept channel closed unexpectly",
            ))),
        }
    }

    #[allow(unused)]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.udp.local_addr()
    }
}
