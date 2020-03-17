/* See LICENSE for license details */
use std::convert::TryInto;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;
extern crate chrono;
use chrono::prelude::*;

mod thread_pool;

pub struct Server {
    threadpool: thread_pool::ThreadPool,
    workers: usize,
    input_thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    pub fn new(num: usize) -> Server {
        assert!(num > 0);
        let mut threadpool = thread_pool::ThreadPool::new(num);
        let input_thread = Option::Some(threadpool.input());
        Server {
            threadpool,
            workers: num,
            input_thread,
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.threadpool.execute(f);
    }

    pub fn is_dead(&self) -> bool {
        return self.threadpool.is_dead();
    }

    pub fn start_at(self, addr: &str) -> thread::JoinHandle<()> {
        let listener = TcpListener::bind(addr).unwrap();
        let is_debug = env::var("debug").is_ok();
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
                    thread::sleep(Duration::from_millis(
                        (self.workers * 200).try_into().unwrap(),
                    ));
                    break;
                }
                match listener.accept() {
                    Ok((stream, addr)) => {
                        self.execute(move || {
                            Parse::handle(stream, is_debug);
                        });
                        let time: DateTime<Local> = Local::now();
                        file.write_all(
                            format!("{:?} at {}\n", addr, time).as_bytes(),
                        )
                        .unwrap();
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        thread::sleep(Duration::from_millis(500));
                    }
                    Err(e) => panic!("Err: {}", e),
                };
            })
            .unwrap();
        return thread;
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(thread) = self.input_thread.take() {
            thread.join().unwrap();
        }
    }
}

struct Parse {}

impl Parse {
    pub fn handle(mut stream: TcpStream, is_debug: bool) {
        let mut buffer = [0; 512];
        stream.read(&mut buffer).unwrap();

        if is_debug {
            println!(
                "\n----------\n\n{}",
                String::from_utf8(buffer.to_vec()).unwrap()
            );
        }

        let get = b"GET";
        let post = b"POST";
        let put = b"PUT";

        if buffer.starts_with(get) {
            let mut file_path = String::from(String::from_utf8(buffer.to_vec())
                .unwrap().split(' ').collect::<Vec<&str>>()[1]);
            let filename = if file_path == "/" {
                "hello.html"
            } else {
                file_path.remove(0);
                file_path.as_str()
            };
            if is_debug {
                println!("file_name: {}", filename);
            }
            let contents = fs::read_to_string(filename).unwrap_or_else(|_|{
                fs::read_to_string("404.html").unwrap()
            });
            let content_type = check_ext(&String::from(filename));
            let status_line = format!(
                "HTTP/1.1 \r\nContent-Type: text/{}\r\n\r\n",
                content_type
            );
            let response = format!("{}{}", status_line, contents);
            if is_debug {
                println!("response: \n{}\n----------\n", response);
            }
            stream.write(response.as_bytes()).unwrap();
        } else if buffer.starts_with(post) {
        } else if buffer.starts_with(put) {
        }
    }
}

fn check_ext(filename: &String) -> String {
    if filename.ends_with(".css") {
        "css".to_string()
    } else if filename.ends_with(".html") {
        "html".to_string()
    } else {
        "html".to_string()
    }
}
