/* See LICENSE for license details */
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, SystemTime};

mod thread_pool;

pub struct Server {
    threadpool: thread_pool::ThreadPool,
}

impl Server {
    pub fn new(num: usize) -> Server {
        assert!(num > 0);
        let threadpool = thread_pool::ThreadPool::new(num);
        Server { threadpool }
    }

    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.threadpool.execute(f);
    }

    pub fn is_dead(&self) -> bool {
        return self.threadpool.is_dead();
    }

    pub fn start_at(mut self, addr: String) -> thread::JoinHandle<()> {
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("./ips.txt")
            .unwrap();
        let thread = thread::Builder::new()
            .name("server_thread".to_string())
            .spawn(move || loop {
                if self.is_dead() {
                    println!("Quitting!");
                    break;
                }
                match listener.accept() {
                    Ok((stream, addr)) => {
                        self.execute(|| {
                            handle_connection(stream);
                        });
                        let time = SystemTime::now();
                        file.write_all(format!("{:?} at {:?}\n", addr, time).as_bytes())
                            .unwrap();
                    }
                    _ => {}
                };
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();
        return thread;
    }
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
