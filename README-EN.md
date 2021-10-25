# Fourth

> Hey, now we are on level 4!

[![](https://img.shields.io/crates/v/fourth)](https://crates.io/crates/fourth) [![CI](https://img.shields.io/github/workflow/status/kernelerr/fourth/Rust)](https://github.com/KernelErr/fourth/actions/workflows/rust.yml)

Fourth is a layer 4 proxy implemented by Rust to listen on specific ports and transfer data to remote addresses according to configuration.

## Features

- Listen on specific port and proxy to local or remote port
- SNI-based rule without terminating TLS connection

## Installation

To gain best performance on your computer's architecture, please consider build the source code. First, you may need [Rust tool chain](https://rustup.rs/).

```bash
$ cd fourth
$ cargo build --release
```

Binary file will be generated at `target/release/fourth`, or you can use `cargo install --path .` to install.

## Configuration

Fourth will read yaml format configuration file from `/etc/fourth/config.yaml`, here is an example:

```yaml
version: 1
log: info

servers:
  example_server:
    listen:
      - "0.0.0.0:443"
      - "[::]:443"
    tls: true # Enable TLS features like SNI
    sni:
      proxy.example.com: proxy
      www.example.com: nginx
    default: ban
  relay_server:
    listen:
      - "127.0.0.1:8081"
    default: remote

upstream:
  nginx: "127.0.0.1:8080"
  proxy: "127.0.0.1:1024"
  other: "www.remote.example.com:8082" # proxy to remote address
```

Built-in two upstreams: ban(terminate connection immediately), echo

## License

Fourth is available under terms of Apache-2.0.