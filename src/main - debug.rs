use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::net::UdpSocket;
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
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
use std::net::ToSocketAddrs;
fn handle_client(mut client: TcpStream) -> io::Result<()> {
    let mut buf = [0u8; 1024];

    // SOCKS5 Greeting
    let n = client.read(&mut buf)?;
    println!("Received greeting: {:?}", &buf[0..n]);
    if n < 3 || buf[0] != 0x05 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid SOCKS5 header"));
    }

    // Respond with no authentication required
    println!("Sending authentication response...");
    client.write_all(&[0x05, 0x00])?;

    // Read the client's request
    let n = client.read(&mut buf)?;
    println!("Received request: {:?}", &buf[0..n]);
    if n < 7 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete request"));
    }

    match buf[1] {
        0x01 => { // TCP Connect
            println!("TCP Connect request received, setting up connection...");

            // Parse the target address
            let target_address = match buf[3] {
                0x01 => { // IPv4
                    let ip = format!("{}.{}.{}.{}", buf[4], buf[5], buf[6], buf[7]);
                    let port = (u16::from(buf[8]) << 8) | u16::from(buf[9]);
                    format!("{}:{}", ip, port)
                }
                0x03 => { // Domain name
                    let len = buf[4] as usize;
                    let domain = std::str::from_utf8(&buf[5..5 + len])
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    let port = (u16::from(buf[5 + len]) << 8) | u16::from(buf[6 + len]);
                    format!("{}:{}", domain, port)
                }
                _ => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported address type"));
                }
            };

            println!("Connecting to target: {}", target_address);

            // Establish the connection to the target address
            let mut server = TcpStream::connect(&target_address)?;

            // Respond with a successful connection
            client.write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])?;
            println!("Connection established with target: {}", target_address);

            // Relay data between client and server
            let mut client_clone = client.try_clone()?;
            let mut server_clone = server.try_clone()?;

            let client_to_server = std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match client_clone.read(&mut buf) {
                        Ok(0) => break, // Client closed connection
                        Ok(n) => {
                            if server_clone.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            let server_to_client = std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match server.read(&mut buf) {
                        Ok(0) => break, // Server closed connection
                        Ok(n) => {
                            if client.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            client_to_server.join().ok();
            server_to_client.join().ok();

            println!("TCP relay terminated.");
            Ok(())
        }
        0x03 => { // UDP Associate
            println!("UDP Associate request received, setting up UDP relay...");

            // Set up a UDP socket for client-to-server communication
            let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
            let local_addr = udp_socket.local_addr()?;
            println!("UDP relay bound to {}", local_addr);

            // Respond with the bound address
            let ip = match local_addr.ip() {
                std::net::IpAddr::V4(v4) => v4.octets(),
                std::net::IpAddr::V6(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "IPv6 not supported")),
            };
            let port = local_addr.port();
            client.write_all(&[
                0x05, 0x00, 0x00, 0x01, ip[0], ip[1], ip[2], ip[3], (port >> 8) as u8, port as u8,
            ])?;

            println!("Sent UDP relay endpoint to client: {}", local_addr);

            // Parse destination address from received UDP packets
            let mut buf = [0u8; 4096];
            loop {
                let (n, src) = udp_socket.recv_from(&mut buf)?;
                if n == 0 {
                    println!("No data received, closing UDP relay...");
                    break;
                }

// Parse the SOCKS5 UDP header to extract the target address
if n < 10 {
    println!("Invalid UDP packet received from client: {:?}", &buf[..n]);
    continue;
}
let frag = buf[2]; // Fragmentation, should be 0
if frag != 0 {
    println!("Unsupported UDP fragmentation, ignoring packet...");
    continue;
}

let target_address = match buf[3] {
    0x01 => { // IPv4
        let ip = format!("{}.{}.{}.{}", buf[4], buf[5], buf[6], buf[7]);
        let port = (u16::from(buf[8]) << 8) | u16::from(buf[9]);
        format!("{}:{}", ip, port)
    }
    0x03 => { // Domain name
        let len = buf[4] as usize;
        let domain = std::str::from_utf8(&buf[5..5 + len])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let port = (u16::from(buf[5 + len]) << 8) | u16::from(buf[6 + len]);
        let resolved = (domain, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "DNS resolution failed"))?;
        format!("{}:{}", resolved.ip(), resolved.port())
    }
    _ => {
        println!("Unsupported address type in UDP packet: {:?}", buf[3]);
        continue;
    }
};

                println!(
                    "Received {} bytes from {}. Target address: {}",
                    n, src, target_address
                );

                // Here, you can forward the packet to the `target_address` if needed
            }

            println!("UDP relay terminated.");
            Ok(())
        }
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported command"));
        }
    }
}




fn relay_traffic(client: TcpStream, server: TcpStream) -> io::Result<()> {
    println!("Setting up traffic relay...");

    let client = Arc::new(Mutex::new(client));
    let server = Arc::new(Mutex::new(server));

    let client_to_server = {
        let client = Arc::clone(&client);
        let server = Arc::clone(&server);

        thread::spawn(move || -> io::Result<()> {
            let mut buf = [0u8; 4096];
            loop {
                let n = client.lock().unwrap().read(&mut buf)?;
                if n == 0 {
                    println!("No data received from client, closing connection...");
                    break;
                }
                println!("Relaying {} bytes from client to server...", n);
                server.lock().unwrap().write_all(&buf[..n])?;
            }
            Ok(())
        })
    };

    let server_to_client = {
        let client = Arc::clone(&client);
        let server = Arc::clone(&server);

        thread::spawn(move || -> io::Result<()> {
            let mut buf = [0u8; 4096];
            loop {
                let n = server.lock().unwrap().read(&mut buf)?;
                if n == 0 {
                    println!("No data received from server, closing connection...");
                    break;
                }
                println!("Relaying {} bytes from server to client...", n);
                client.lock().unwrap().write_all(&buf[..n])?;
            }
            Ok(())
        })
    };

    client_to_server.join().unwrap()?;
    server_to_client.join().unwrap()?;

    println!("Traffic relay completed.");
    Ok(())
}
