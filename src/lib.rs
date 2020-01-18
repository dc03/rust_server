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
    parser: Parse,
}

impl Server {
    pub fn new(num: usize, filename: &str) -> Server {
        assert!(num > 0);
        let mut threadpool = thread_pool::ThreadPool::new(num);
        let input_thread = Option::Some(threadpool.input());
        let parser = Parse::new(filename);
        Server {
            threadpool,
            workers: num,
            input_thread,
            parser,
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
                        let refer = self.parser.make_new();
                        self.execute(move || {
                            refer.handle(stream, is_debug);
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

struct Parse {
    configs: Vec<(String, String, String)>,
}

impl Parse {
    pub fn new(filename: &str) -> Parse {
        let file = fs::read_to_string(filename).unwrap();
        let mut configs = Vec::new();
        for line in file.split("\n") {
            let mut token_holder = Vec::new();
            for token in line.split(" ") {
                let mut token = String::from(token);
                if token.contains("\r") | token.contains("\n") {
                    token.pop();
                }
                token_holder.push(token);
            }
            configs.push((
                String::from(&token_holder[0]),
                String::from(&token_holder[1]),
                String::from(&token_holder[2]),
            ));
        }

        Parse { configs }
    }

    pub fn handle(&self, mut stream: TcpStream, is_debug: bool) {
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
            let mut found = false;
            for config in &self.configs {
                if is_debug {
                    println!("config.0: {}, config.1: {}", config.0, config.1);
                }

                if (config.0 == "GET") | (config.0 == "get") {
                    let req = format!("GET {} HTTP/1.1", config.1);

                    if is_debug {
                        println!("req: {}", req);
                    }

                    if buffer.starts_with(req.as_bytes()) {
                        if is_debug {
                            println!("Matches with: {}", req);
                        }

                        let content_type = check_ext(&config.2);
                        let status_line = format!(
                            "HTTP/1.1 \r\nContent-Type: text/{}\r\n\r\n",
                            content_type
                        );
                        let filename = String::from(&config.2);
                        let contents = fs::read_to_string(filename).unwrap();
                        let response = format!("{}{}", status_line, contents);

                        if is_debug {
                            println!("response: {}\n----------\n", response);
                        }
                        stream.write(response.as_bytes()).unwrap();
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                for config in &self.configs {
                    if config.0 == "404" {
                        let file_404 = fs::read_to_string(&config.2).unwrap();
                        let response = format!(
                            "HTTP/1.1 404 NOT FOUND\r\n\r\n{}",
                            file_404
                        );
                        stream.write(response.as_bytes()).unwrap();
                        break;
                    }
                }
            }
        } else if buffer.starts_with(post) {
        } else if buffer.starts_with(put) {
        }
    }

    pub fn make_new(&self) -> Parse {
        let mut configs = Vec::new();
        for config in &self.configs {
            let tuple = (
                String::from(&config.0),
                String::from(&config.1),
                String::from(&config.2),
            );
            configs.push(tuple);
        }
        Parse { configs }
    }
}

fn check_ext(filename: &String) -> String {
    if filename.ends_with(".css") {
        "css".to_string()
    } else if filename.ends_with(".html") {
        "html".to_string()
    } else {
        "none".to_string()
    }
}
