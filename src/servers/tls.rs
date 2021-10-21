use log::{debug, warn};
use tls_parser::{
    parse_tls_extensions, parse_tls_raw_record, parse_tls_record_with_header, TlsMessage,
    TlsMessageHandshake,
};

pub fn get_sni(buf: &[u8]) -> Vec<String> {
    let mut snis: Vec<String> = Vec::new();
    match parse_tls_raw_record(buf) {
        Ok((_, ref r)) => {
            match parse_tls_record_with_header(r.data, &r.hdr) {
                Ok((_, ref msg_list)) => {
                    for msg in msg_list {
                        match *msg {
                            TlsMessage::Handshake(ref m) => match *m {
                                TlsMessageHandshake::ClientHello(ref content) => {
                                    debug!("TLS ClientHello version: {}", content.version);
                                    let ext = parse_tls_extensions(content.ext.unwrap_or(b""));
                                    match ext {
                                        Ok((_, ref extensions)) => {
                                            for ext in extensions {
                                                match *ext {
                                                    tls_parser::TlsExtension::SNI(ref v) => {
                                                        for &(t, sni) in v {
                                                            match String::from_utf8(sni.to_vec()) {
                                                                Ok(s) => {
                                                                    debug!("TLS SNI: {} {}", t, s);
                                                                    snis.push(s);
                                                                }
                                                                Err(e) => {
                                                                    warn!("Failed to parse SNI: {} {}", t, e);
                                                                }
                                                            }
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("TLS extensions error: {}", e);
                                        }
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
                Err(err) => {
                    warn!("Failed to parse TLS: {}", err);
                }
            }
        }
        Err(err) => {
            warn!("Failed to parse TLS: {}", err);
        }
    }
    
    snis
}
