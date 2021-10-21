use log::{debug, warn};
use tls_parser::{
    parse_tls_extensions, parse_tls_raw_record, parse_tls_record_with_header, TlsMessage,
    TlsMessageHandshake,
};

pub fn get_sni(buf: &[u8]) -> Vec<String> {
    let mut snis: Vec<String> = Vec::new();
    match parse_tls_raw_record(buf) {
        Ok((_, ref r)) => match parse_tls_record_with_header(r.data, &r.hdr) {
            Ok((_, ref msg_list)) => {
                for msg in msg_list {
                    if let TlsMessage::Handshake(TlsMessageHandshake::ClientHello(ref content)) =
                        *msg
                    {
                        debug!("TLS ClientHello version: {}", content.version);
                        let ext = parse_tls_extensions(content.ext.unwrap_or(b""));
                        match ext {
                            Ok((_, ref extensions)) => {
                                for ext in extensions {
                                    if let tls_parser::TlsExtension::SNI(ref v) = *ext {
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
                                }
                            }
                            Err(e) => {
                                warn!("TLS extensions error: {}", e);
                            }
                        }
                    }
                }
            }
            Err(err) => {
                warn!("Failed to parse TLS: {}", err);
            }
        },
        Err(err) => {
            warn!("Failed to parse TLS: {}", err);
        }
    }

    snis
}
