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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sni_extract() {
        const BUF: [u8; 517] = [
            0x16, 0x03, 0x01, 0x02, 0x00, 0x01, 0x00, 0x01, 0xfc, 0x03, 0x03, 0x35, 0x7a, 0xba,
            0x3d, 0x89, 0xd2, 0x5e, 0x7a, 0xa2, 0xd4, 0xe5, 0x6d, 0xd5, 0xa3, 0x98, 0x41, 0xb0,
            0xae, 0x41, 0xfc, 0xe6, 0x64, 0xfd, 0xae, 0x0b, 0x27, 0x6d, 0x90, 0xa8, 0x0a, 0xfa,
            0x90, 0x20, 0x59, 0x6f, 0x13, 0x18, 0x4a, 0xd1, 0x1c, 0xc4, 0x83, 0x8c, 0xfc, 0x93,
            0xac, 0x6b, 0x3b, 0xac, 0x67, 0xd0, 0x36, 0xb0, 0xa2, 0x1b, 0x04, 0xf7, 0xde, 0x02,
            0xfb, 0x96, 0x1e, 0xdc, 0x76, 0xa8, 0x00, 0x20, 0x2a, 0x2a, 0x13, 0x01, 0x13, 0x02,
            0x13, 0x03, 0xc0, 0x2b, 0xc0, 0x2f, 0xc0, 0x2c, 0xc0, 0x30, 0xcc, 0xa9, 0xcc, 0xa8,
            0xc0, 0x13, 0xc0, 0x14, 0x00, 0x9c, 0x00, 0x9d, 0x00, 0x2f, 0x00, 0x35, 0x01, 0x00,
            0x01, 0x93, 0xea, 0xea, 0x00, 0x00, 0x00, 0x00, 0x00, 0x13, 0x00, 0x11, 0x00, 0x00,
            0x0e, 0x77, 0x77, 0x77, 0x2e, 0x6c, 0x69, 0x72, 0x75, 0x69, 0x2e, 0x74, 0x65, 0x63,
            0x68, 0x00, 0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00, 0x00, 0x0a, 0x00, 0x0a,
            0x00, 0x08, 0xba, 0xba, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00, 0x0b, 0x00, 0x02,
            0x01, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x10, 0x00, 0x0e, 0x00, 0x0c, 0x02, 0x68,
            0x32, 0x08, 0x68, 0x74, 0x74, 0x70, 0x2f, 0x31, 0x2e, 0x31, 0x00, 0x05, 0x00, 0x05,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x12, 0x00, 0x10, 0x04, 0x03, 0x08,
            0x04, 0x04, 0x01, 0x05, 0x03, 0x08, 0x05, 0x05, 0x01, 0x08, 0x06, 0x06, 0x01, 0x00,
            0x12, 0x00, 0x00, 0x00, 0x33, 0x00, 0x2b, 0x00, 0x29, 0xba, 0xba, 0x00, 0x01, 0x00,
            0x00, 0x1d, 0x00, 0x20, 0x3b, 0x45, 0xf9, 0xbc, 0x6e, 0x23, 0x86, 0x41, 0xa5, 0xb2,
            0xf5, 0x03, 0xec, 0x67, 0x4a, 0xd7, 0x9a, 0x17, 0x9f, 0x0c, 0x38, 0x6d, 0x36, 0xf3,
            0x4e, 0x5d, 0xa4, 0x7d, 0x15, 0x79, 0xa4, 0x3f, 0x00, 0x2d, 0x00, 0x02, 0x01, 0x01,
            0x00, 0x2b, 0x00, 0x0b, 0x0a, 0xba, 0xba, 0x03, 0x04, 0x03, 0x03, 0x03, 0x02, 0x03,
            0x01, 0x00, 0x1b, 0x00, 0x03, 0x02, 0x00, 0x02, 0x44, 0x69, 0x00, 0x05, 0x00, 0x03,
            0x02, 0x68, 0x32, 0xda, 0xda, 0x00, 0x01, 0x00, 0x00, 0x15, 0x00, 0xc5, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let sni = get_sni(&BUF);
        assert!(sni[0] == "www.lirui.tech".to_string());
    }
}