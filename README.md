# RUSTYSOCKS5

A lightweight, dependency-free SOCKS5 proxy server written in Rust. 
Built exclusively using Rust's standard library, it runs on any platform and architecture supported by Rust.

## Features

- SOCKS5 protocol implementation (RFC 1928)
- Zero external dependencies (pure Rust stdlib)
- Cross-platform support (Windows/Linux/macOS)
- Basic username/password authentication
- Simple to configure and deploy

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/pj1234678/RustySOCKS5.git
   cd rustysocks5
   ```

2. Build with Cargo:
   ```bash
   cargo build --release
   ```

Binary will be located at `target/release/rustysocks5`

## Usage

Start the proxy server:
```bash
./rustysocks5
```

Default configuration:
- Listen address: `127.0.0.1:1080`
- Authentication: enabled (see configuration below)

Client configuration examples:
```bash
curl --socks5 http://admin:password@127.0.0.1:1080 https://example.com
```


## Configuration

Modify authentication credentials in source code (before compiling):
```rust
// src/main.rs (line XX-XX)
if username != b"admin" || password != b"password" {
    // Authentication logic
}
```

## Security Considerations

⚠️ **Important Security Notes:**
- Default credentials are for demonstration purposes only
- Hardcoded credentials are not production-safe
- For real-world use:
  - Store credentials in environment variables
  - Use proper secret management
  - Consider implementing encryption

## Contributing

Contributions are welcome! Please follow standard GitHub workflow:
1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## License

MIT License.
