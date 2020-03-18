/* See LICENSE for license details */
use std::convert::TryInto;
use std::env;
use std::fs::{self, OpenOptions, File};
use std::io::{prelude::*, BufReader};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;
use std::str::FromStr;
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

    pub fn start_at(self, addr: &str, config: &'static str) -> thread::JoinHandle<()> {
        let listener = TcpListener::bind(addr).unwrap();
        let is_debug = env::var("debug").is_ok();
        listener.set_nonblocking(true).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("./ips.txt")
            .unwrap();
        let parser = Parse::new(config);
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
                        let parser = parser.make_copy();
                        self.execute(move || {
                            parser.handle(stream, is_debug);
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
    index: String,
    error_404: String,
    has_index: bool,
    has_error: bool,
}

impl Parse {
    pub fn new(filename: &str) -> Parse {
        let config = File::open(filename).unwrap();
        let mut index = String::new();
        let mut error_404 = String::new();
        let mut has_index = true;
        let mut has_error = true;
        for line in BufReader::new(config).lines() {
            let line = line.unwrap();
            if line.starts_with("index:") {
                index = String::from(line)
                    .split(' ').collect::<Vec<&str>>()[1].to_string();
            } else if line.starts_with("404:") {
                error_404 = String::from(line)
                    .split(' ').collect::<Vec<&str>>()[1].to_string();
            } else if !line.starts_with("#") && line.len() > 0 {
                println!("Garbage in config file: {}", line);
            }
        }

        if index.is_empty() {
            println!("No index file provided. Using dummy file.");
            has_index = false;
        }
        if error_404.is_empty() {
            println!("No 404 file provided. Using dummy file.");
            has_error = false;
        }

        Parse {
            index,
            error_404,
            has_index,
            has_error,
        }
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
            let mut file_path = String::from(String::from_utf8(buffer.to_vec())
                .unwrap().split(' ').collect::<Vec<&str>>()[1]);
            let filename = if file_path == "/" {
                if self.has_index {
                    self.index.as_str()
                } else {
                    "dummy.html"
                }
            }  else {
                file_path.remove(0);
                file_path.as_str()
            };
            if is_debug {
                println!("file_name: {}", filename);
            }
            let contents: String;
            let mut response_type: &str;
            let mut content_type = check_ext(&String::from(filename));
            if !self.has_index && file_path == "/" && !self.has_error {
                contents =
                    "<!DOCTYPE html><html><body>No index file</body></html>"
                    .to_string();
                response_type = "200 OK";
            } else {
                response_type = "200 OK";
                contents = fs::read_to_string(filename).unwrap_or_else(|_|{
                    response_type = "404 NOT FOUND";
                    content_type = "html".to_string();
                    if self.has_error {
                        fs::read_to_string(self.error_404.as_str())
                        .unwrap()
                    } else {
                        "<!DOCTYPE html><html><body>No 404 file</body></html>"
                        .to_string()
                    }
                });
            };
            let status_line = format!(
                "HTTP/1.1 {}\r\nContent-Type: text/{}\r\n\r\n",
                response_type, content_type
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

    pub fn make_copy(&self) -> Parse {
        let index = String::from_str(self.index.as_str()).unwrap();
        let error_404 = String::from_str(self.error_404.as_str()).unwrap();
        let has_index = self.has_index;
        let has_error = self.has_error;

        Parse {
            index,
            error_404,
            has_index,
            has_error,
        }
    }
}

fn check_ext(filename: &String) -> String {
    if filename.ends_with(".css") {
        "css".to_string()
    } else if filename.ends_with(".html") {
        "html".to_string()
    } else {
        "plain".to_string()
    }
}
