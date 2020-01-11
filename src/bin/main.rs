/* See LICENSE for license details */
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, SystemTime};

use server::Server;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    listener.set_nonblocking(true)?;
    let mut server = Server::new(5);
    let thread = server.start_input();

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open("./ips.txt")?;
    loop {
        if server.is_dead() {
            println!("Quitting!");
            break;
        }
        match listener.accept() {
            Ok((stream, addr)) => {
                server.execute(|| {
                    handle_connection(stream);
                });
                let time = SystemTime::now();
                file.write_all(format!("{:?} at {:?}\n", addr, time).as_bytes())?;
            }
            _ => {}
        };
        thread::sleep(Duration::from_millis(500));
    }
    // for stream in listener.incoming() {
    //     match stream {
    //         Ok(s) => server.execute(|| {
    //             handle_connection(s);
    //         }),
    //         Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
    //             if server.is_dead() {
    //                 println!("Quitting!");
    //                 break;
    //             }
    //             thread::sleep(Duration::from_millis(500));
    //         }
    //         Err(e) => panic!("Error: {}", e),
    //     }
    // }
    thread.join().unwrap();
    Ok(())
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();

    let get = b"GET / HTTP/1.1\r\n";
    let sleep = b"GET /sleep HTTP/1.1\r\n";

    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else if buffer.starts_with(sleep) {
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK\r\n\r\n", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND\r\n\r\n", "404.html")
    };
    let contents = fs::read_to_string(filename).unwrap();

    let response = format!("{}{}", status_line, contents);

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
