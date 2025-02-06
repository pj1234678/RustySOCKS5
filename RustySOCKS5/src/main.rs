use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    println!("Initializing SOCKS5 Proxy...");
    let listener = TcpListener::bind("0.0.0.0:1080")?;
    println!("SOCKS5 proxy running on 0.0.0.0:1080");

    let listener = Arc::new(listener);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Incoming connection: {:?}", stream.peer_addr());
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Error accepting connection: {}", e),
        }
    }
    Ok(())
}

fn handle_client(mut client: TcpStream) -> io::Result<()> {
    let mut buf = [0u8; 1024];

    // SOCKS5 Greeting
    let n = client.read(&mut buf)?;
    println!("Received greeting: {:?}", &buf[0..n]);
    if n < 3 || buf[0] != 0x05 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 header"));
    }

    let nmethods = buf[1];
    let methods = &buf[2..2 + nmethods as usize];
    if !methods.contains(&0x02) {
        client.write_all(&[0x05, 0xFF])?;
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "No supported authentication methods"));
    }

    // Select username/password auth
    client.write_all(&[0x05, 0x02])?;

    // Authentication
    let n = client.read(&mut buf)?;
    if n < 1 || buf[0] != 0x01 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid authentication version"));
    }

    let ulen = buf[1] as usize;
    if 2 + ulen >= n {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid username length"));
    }
    let username = &buf[2..2 + ulen];

    let plen_index = 2 + ulen;
    if plen_index >= n {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "No password length"));
    }
    let plen = buf[plen_index] as usize;

    let password_start = plen_index + 1;
    let password_end = password_start + plen;
    if password_end > n {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid password length"));
    }
    let password = &buf[password_start..password_end];

    // Validate credentials (hardcoded for example)
    if username != b"admin" || password != b"password" {
        client.write_all(&[0x01, 0x01])?;
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed"));
    }

    // Authentication successful
    client.write_all(&[0x01, 0x00])?;

    // Read client request
    let n = client.read(&mut buf)?;
    println!("Received request: {:?}", &buf[0..n]);
    if n < 7 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete request"));
    }

    match buf[1] {
        0x01 => handle_tcp_connect(&mut client, &buf[0..n]),
        0x03 => handle_udp_associate(&mut client),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported command")),
    }
}

fn handle_tcp_connect(client: &mut TcpStream, request: &[u8]) -> io::Result<()> {
    let target_address = parse_target_address(request)?;
    println!("Connecting to target: {}", target_address);
    
    let mut server = TcpStream::connect(&target_address)?;
    client.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])?;

    let (mut client_reader, mut client_writer) = (client.try_clone()?, client.try_clone()?);
    let (mut server_reader, mut server_writer) = (server.try_clone()?, server.try_clone()?);

    let client_to_server = thread::spawn(move || {
        io::copy(&mut client_reader, &mut server_writer).map(|_| ())
    });

    let server_to_client = thread::spawn(move || {
        io::copy(&mut server_reader, &mut client_writer).map(|_| ())
    });

    client_to_server.join().unwrap()?;
    server_to_client.join().unwrap()?;
    println!("TCP relay terminated.");
    Ok(())
}

fn handle_udp_associate(client: &mut TcpStream) -> io::Result<()> {
    let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
    let local_addr = udp_socket.local_addr()?;
    
    let ip = match local_addr.ip() {
        std::net::IpAddr::V4(v4) => v4.octets(),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "IPv6 not supported")),
    };
    
    client.write_all(&[
        0x05, 0x00, 0x00, 0x01,
        ip[0], ip[1], ip[2], ip[3],
        (local_addr.port() >> 8) as u8, local_addr.port() as u8,
    ])?;

    let mut buf = [0u8; 4096];
    loop {
        let (n, src) = udp_socket.recv_from(&mut buf)?;
        if n < 10 || buf[2] != 0x00 {
            continue; // Skip invalid packets
        }

        let target_addr = match parse_udp_target(&buf[3..n]) {
            Ok(addr) => addr,
            Err(_) => continue,
        };

        // Implement UDP forwarding logic here
        println!("Received {} bytes from {} for {}", n, src, target_addr);
    }
}

fn parse_target_address(request: &[u8]) -> io::Result<String> {
    match request[3] {
        0x01 => Ok(format!("{}.{}.{}.{}:{}",
            request[4], request[5], request[6], request[7],
            u16::from_be_bytes([request[8], request[9]])
        )),
        0x03 => {
            let len = request[4] as usize;
            let domain = std::str::from_utf8(&request[5..5+len])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid domain"))?;
            let port = u16::from_be_bytes([request[5+len], request[6+len]]);
            Ok(format!("{}:{}", domain, port))
        },
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type")),
    }
}

fn parse_udp_target(data: &[u8]) -> io::Result<String> {
    match data[0] {
        0x01 => Ok(format!("{}.{}.{}.{}:{}",
            data[1], data[2], data[3], data[4],
            u16::from_be_bytes([data[5], data[6]])
        )),
        0x03 => {
            let len = data[1] as usize;
            let domain = std::str::from_utf8(&data[2..2+len])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid domain"))?;
            let port = u16::from_be_bytes([data[2+len], data[3+len]]);
            Ok(format!("{}:{}", domain, port))
        },
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported UDP address type")),
    }
}