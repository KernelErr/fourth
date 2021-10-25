# Fourth

> 这一波在第四层。

[![](https://img.shields.io/crates/v/fourth)](https://crates.io/crates/fourth) [![CI](https://img.shields.io/github/workflow/status/kernelerr/fourth/Rust)](https://github.com/KernelErr/fourth/actions/workflows/rust.yml)

[English](/README-EN.md)

Fourth是一个Rust实现的Layer 4代理，用于监听指定端口TCP流量，并根据规则转发到指定目标。

## 功能

- 监听指定端口代理到本地或远端指定端口
- 监听指定端口，通过TLS ClientHello消息中的SNI进行分流

## 安装方法

为了确保获得您架构下的最佳性能，请考虑自行编译，首选需要确保您拥有[Rust工具链](https://rustup.rs/)。

```bash
$ cd fourth
$ cargo build --release
```

将在`target/release/fourth`生成二进制文件，您也可以使用`cargo install --path . `来安装二进制文件。

## 配置

Fourth使用yaml格式的配置文件，默认情况下会读取`/etc/fourth/config.yaml`，如下是一个示例配置。

```yaml
version: 1
log: info

servers:
  example_server:
    listen:
      - "0.0.0.0:443"
      - "[::]:443"
    tls: true # 启动SNI分流，将根据TLS请求中的主机名分流
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
  other: "www.remote.example.com:8082" # 代理到远端地址
```

内置两个的upstream：ban（立即中断连接）、echo（返回读到的数据）。

## io_uring?

尽管经过了很多尝试，我们发现目前一些Rust下面的io_uring实现存在问题，我们使用的io_uring库实现尽管在吞吐量上可以做到单线程20Gbps（相比之下Tokio仅有8Gbps），但在QPS上存在性能损失较大的问题。因此在有成熟的io_uring实现之前，我们仍然选择epoll。之后我们会持续关注相关进展。

可能以后会为Linux高内核版本的用户提供可选的io_uring加速。

## 协议

Fourth以Apache-2.0协议开源。
