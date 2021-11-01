# Fourth

> Hey, now we are on level 4!

[![](https://img.shields.io/crates/v/fourth)](https://crates.io/crates/fourth) [![CI](https://img.shields.io/github/workflow/status/kernelerr/fourth/Rust)](https://github.com/KernelErr/fourth/actions/workflows/rust.yml)

**Under heavy development, version 0.1 may update frequently**

Fourth is a layer 4 proxy implemented by Rust to listen on specific ports and transfer TCP/KCP data to remote addresses(only TCP) according to configuration.

## Features

- Listen on specific port and proxy to local or remote port
- SNI-based rule without terminating TLS connection
- Allow KCP inbound(warning: untested)

## Installation

To gain best performance on your computer's architecture, please consider build the source code. First, you may need [Rust tool chain](https://rustup.rs/).

```bash
$ cd fourth
$ cargo build --release
```

Binary file will be generated at `target/release/fourth`, or you can use `cargo install --path .` to install.

Or you can use Cargo to install Fourth:

```bash
$ cargo install fourth
```

Or you can download binary file form the Release page.

## Configuration

Fourth will read yaml format configuration file from `/etc/fourth/config.yaml`, and you can set custom path to environment variable `FOURTH_CONFIG`, here is an minimal viable example:

```yaml
version: 1
log: info

servers:
  proxy_server:
    listen:
      - "127.0.0.1:8081"
    default: remote

upstream:
  remote: "tcp://www.remote.example.com:8082" # proxy to remote address
```

Built-in two upstreams: ban(terminate connection immediately), echo. For detailed configuration, check [this example](./example-config.yaml).

## Performance Benchmark

Tested on 4C2G server:

Use fourth to proxy to Nginx(QPS of direct connection: ~120000): ~70000 req/s (Command: `wrk -t200 -c1000 -d120s --latency http://proxy-server:8081`)

Use fourth to proxy to local iperf3: 8Gbps

## Thanks

- [tokio_kcp](https://github.com/Matrix-Zhang/tokio_kcp)

## License

Fourth is available under terms of Apache-2.0.