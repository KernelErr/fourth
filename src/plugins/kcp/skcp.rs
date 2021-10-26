use std::{
    io::{self, ErrorKind, Write},
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use futures::future;
use kcp::{Error as KcpError, Kcp, KcpResult};
use log::{error, trace};
use tokio::{net::UdpSocket, sync::mpsc};

use crate::plugins::kcp::{utils::now_millis, KcpConfig};

/// Writer for sending packets to the underlying UdpSocket
struct UdpOutput {
    socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
    delay_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl UdpOutput {
    /// Create a new Writer for writing packets to UdpSocket
    pub fn new(socket: Arc<UdpSocket>, target_addr: SocketAddr) -> UdpOutput {
        let (delay_tx, mut delay_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        {
            let socket = socket.clone();
            tokio::spawn(async move {
                while let Some(buf) = delay_rx.recv().await {
                    if let Err(err) = socket.send_to(&buf, target_addr).await {
                        error!("[SEND] UDP delayed send failed, error: {}", err);
                    }
                }
            });
        }

        UdpOutput {
            socket,
            target_addr,
            delay_tx,
        }
    }
}

impl Write for UdpOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.socket.try_send_to(buf, self.target_addr) {
            Ok(n) => Ok(n),
            Err(ref err) if err.kind() == ErrorKind::WouldBlock => {
                // send return EAGAIN
                // ignored as packet was lost in transmission
                trace!(
                    "[SEND] UDP send EAGAIN, packet.size: {} bytes, delayed send",
                    buf.len()
                );

                self.delay_tx
                    .send(buf.to_owned())
                    .expect("channel closed unexpectly");

                Ok(buf.len())
            }
            Err(err) => Err(err),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct KcpSocket {
    kcp: Kcp<UdpOutput>,
    last_update: Instant,
    socket: Arc<UdpSocket>,
    flush_write: bool,
    flush_ack_input: bool,
    sent_first: bool,
    pending_sender: Option<Waker>,
    pending_receiver: Option<Waker>,
    closed: bool,
}

impl KcpSocket {
    pub fn new(
        c: &KcpConfig,
        conv: u32,
        socket: Arc<UdpSocket>,
        target_addr: SocketAddr,
        stream: bool,
    ) -> KcpResult<KcpSocket> {
        let output = UdpOutput::new(socket.clone(), target_addr);
        let mut kcp = if stream {
            Kcp::new_stream(conv, output)
        } else {
            Kcp::new(conv, output)
        };
        c.apply_config(&mut kcp);

        // Ask server to allocate one
        if conv == 0 {
            kcp.input_conv();
        }

        kcp.update(now_millis())?;

        Ok(KcpSocket {
            kcp,
            last_update: Instant::now(),
            socket,
            flush_write: c.flush_write,
            flush_ack_input: c.flush_acks_input,
            sent_first: false,
            pending_sender: None,
            pending_receiver: None,
            closed: false,
        })
    }

    /// Call every time you got data from transmission
    pub fn input(&mut self, buf: &[u8]) -> KcpResult<bool> {
        match self.kcp.input(buf) {
            Ok(..) => {}
            Err(KcpError::ConvInconsistent(expected, actual)) => {
                trace!(
                    "[INPUT] Conv expected={} actual={} ignored",
                    expected,
                    actual
                );
                return Ok(false);
            }
            Err(err) => return Err(err),
        }
        self.last_update = Instant::now();

        if self.flush_ack_input {
            self.kcp.flush_ack()?;
        }

        Ok(self.try_wake_pending_waker())
    }

    /// Call if you want to send some data
    pub fn poll_send(&mut self, cx: &mut Context<'_>, mut buf: &[u8]) -> Poll<KcpResult<usize>> {
        if self.closed {
            return Ok(0).into();
        }

        // If:
        //     1. Have sent the first packet (asking for conv)
        //     2. Too many pending packets
        if self.sent_first
            && (self.kcp.wait_snd() >= self.kcp.snd_wnd() as usize || self.kcp.waiting_conv())
        {
            trace!(
                "[SEND] waitsnd={} sndwnd={} excceeded or waiting conv={}",
                self.kcp.wait_snd(),
                self.kcp.snd_wnd(),
                self.kcp.waiting_conv()
            );
            self.pending_sender = Some(cx.waker().clone());
            return Poll::Pending;
        }

        if !self.sent_first && self.kcp.waiting_conv() && buf.len() > self.kcp.mss() as usize {
            buf = &buf[..self.kcp.mss() as usize];
        }

        let n = self.kcp.send(buf)?;
        self.sent_first = true;
        self.last_update = Instant::now();

        if self.flush_write {
            self.kcp.flush()?;
        }

        Ok(n).into()
    }

    /// Call if you want to send some data
    #[allow(dead_code)]
    pub async fn send(&mut self, buf: &[u8]) -> KcpResult<usize> {
        future::poll_fn(|cx| self.poll_send(cx, buf)).await
    }

    #[allow(dead_code)]
    pub fn try_recv(&mut self, buf: &mut [u8]) -> KcpResult<usize> {
        if self.closed {
            return Ok(0);
        }
        self.kcp.recv(buf)
    }

    pub fn poll_recv(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<KcpResult<usize>> {
        if self.closed {
            return Ok(0).into();
        }

        match self.kcp.recv(buf) {
            Ok(n) => Ok(n).into(),
            Err(KcpError::RecvQueueEmpty) => {
                self.pending_receiver = Some(cx.waker().clone());
                Poll::Pending
            }
            Err(err) => Err(err).into(),
        }
    }

    #[allow(dead_code)]
    pub async fn recv(&mut self, buf: &mut [u8]) -> KcpResult<usize> {
        future::poll_fn(|cx| self.poll_recv(cx, buf)).await
    }

    pub fn flush(&mut self) -> KcpResult<()> {
        self.kcp.flush()?;
        self.last_update = Instant::now();
        Ok(())
    }

    fn try_wake_pending_waker(&mut self) -> bool {
        let mut waked = false;

        if self.pending_sender.is_some()
            && self.kcp.wait_snd() < self.kcp.snd_wnd() as usize
            && !self.kcp.waiting_conv()
        {
            let waker = self.pending_sender.take().unwrap();
            waker.wake();

            waked = true;
        }

        if self.pending_receiver.is_some() {
            if let Ok(peek) = self.kcp.peeksize() {
                if peek > 0 {
                    let waker = self.pending_receiver.take().unwrap();
                    waker.wake();

                    waked = true;
                }
            }
        }

        waked
    }

    pub fn update(&mut self) -> KcpResult<Instant> {
        let now = now_millis();
        self.kcp.update(now)?;
        let next = self.kcp.check(now);

        self.try_wake_pending_waker();

        Ok(Instant::now() + Duration::from_millis(next as u64))
    }

    pub fn close(&mut self) {
        self.closed = true;
        if let Some(w) = self.pending_sender.take() {
            w.wake();
        }
        if let Some(w) = self.pending_receiver.take() {
            w.wake();
        }
    }

    pub fn udp_socket(&self) -> &Arc<UdpSocket> {
        &self.socket
    }

    pub fn can_close(&self) -> bool {
        self.kcp.wait_snd() == 0
    }

    pub fn conv(&self) -> u32 {
        self.kcp.conv()
    }

    pub fn peek_size(&self) -> KcpResult<usize> {
        self.kcp.peeksize()
    }

    pub fn last_update_time(&self) -> Instant {
        self.last_update
    }
}
