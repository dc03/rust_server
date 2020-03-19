/* See LICENSE for license details */

//! # rust_server
//!
//! `rust_server` is a project of mine to create a simple, functional
//! multithreaded server in rust

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

pub mod thread_pool;

pub struct Server {
    threadpool: thread_pool::ThreadPool,
    workers: usize,
    input_thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    /// Creates a new server to use. Requires that the number of workers be
    /// greater than zero
    ///
    /// # Examples
    ///
    /// ```
    /// let server = server::Server::new(5);
    ///
    /// let thread = server.start_at("127.0.0.1:8080", "config.txt");
    /// // Join the thread later
    /// ```
    ///
    /// # Panics
    ///
    /// If the number of workers is less than zero
    pub fn new(
        num: usize
    ) -> Server {
        assert!(num > 0);
        let mut threadpool = thread_pool::ThreadPool::new(num);
        // Option trick so that we can take the thread later to join it
        let input_thread = Option::Some(threadpool.input());
        Server {
            threadpool,
            workers: num,
            input_thread,
        }
    }

    /// Executes a job passed to it through the workers the thread pool
    /// maintains. It is usually not needed to call this as `start_at()`
    /// handles this by itself
    pub fn execute<F>(
        &self, f: F
    )
    where
        F: FnOnce() + Send + 'static,
    {
        self.threadpool.execute(f);
    }

    /// Returns the current state of the server
    pub fn is_dead(
        &self
    ) -> bool {
        return self.threadpool.is_dead();
    }

    /// Starts the server at a given ip address and with a given config file
    /// Automatically handles any requests and returns the handle to the
    /// main server thread
    ///
    /// # Examples
    ///
    /// ```
    /// let server = server::Server::new(5);
    ///
    /// let thread = server.start_at("127.0.0.1:8080", "config.txt");
    /// // Join the thread later
    /// ```
    ///
    /// # Panics
    ///
    /// - If the TcpListener could not be set to non-blocking
    /// - If the ip logging file could not be opened or written to
    /// - If the thread could not be paused while shutting down (should not
    ///   happen)
    /// - If the thread could not be created 
    pub fn start_at(
        self, addr: &str, 
        config: &'static str
    ) -> thread::JoinHandle<()> {
        let listener = TcpListener::bind(addr).unwrap();
        // The environment variable 'debug' can be set to 1 for useful 
        // debugging purposes
        let is_debug = env::var("debug").is_ok();
        listener.set_nonblocking(true).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open("./ips.txt")
            .unwrap();
        let parser = Parse::new(config);
        // Start the server on another thread to avoid blocking the main
        // thread ever
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
        // If for some reason the server is unexpectedly shut off, make sure
        // that the threadpool has been shut off
        self.threadpool.kill();
        if let Some(thread) = self.input_thread.take() {
            thread.join().unwrap();
        }
    }
}

pub struct Parse {
    index: String,
    error_404: String,
    has_index: bool,
    has_error: bool,
}

impl Parse {
    /// Function to create a new parser for any http requests. Requires the
    /// name of the index file and the 404 error file and uses a dummy one if
    /// none are provided.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = "config.txt";
    ///
    /// let parser = server::Parse::new(config);
    /// // Use the parser to handle any requests
    /// ```
    ///
    /// # Panics
    ///
    /// - If the config file could not be opened
    /// - If the iterator returned by BufReader contains Err value (should not
    ///   happen)
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
            // '#' is for comments
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

    /// Function to handle any http requests the parser gets. Does not check
    /// if the file being given is allowed or not (i.e allows access to any
    /// readable file). Does not keep the connection alive yet
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// let parser = server::Parse::new("config.txt");
    ///
    /// listener.set_nonblocking(true).unwrap();
    ///
    /// match listener.accept() {
    ///     Ok((stream, addr)) => {
    ///         // Use the parser to handle any requests
    ///         // The second argument is for printing debug info
    ///         parser.handle(stream, true);
    ///     },
    ///     _ => {},
    /// };
    /// ```
    ///
    /// # Panics
    ///
    /// - If the TcpStream could not be read from
    /// - If the file to be sent could not be opened
    pub fn handle(&self, mut stream: TcpStream, is_debug: bool) {
        let mut buffer = [0; 512];
        stream.read(&mut buffer).unwrap();

        if is_debug {
            println!(
                "\n----------\n\n{}",
                String::from_utf8(buffer.to_vec()).unwrap()
            );
        }

        // The type of requests possible
        let get = b"GET";
        let post = b"POST";
        let put = b"PUT";

        if buffer.starts_with(get) {
            let mut file_path = String::from(String::from_utf8(buffer.to_vec())
                .unwrap().split(' ').collect::<Vec<&str>>()[1]);
            // If the user provided no index file use our own
            let filename = if file_path == "/" {
                if self.has_index {
                    self.index.as_str()
                } else {
                    "dummy.html"
                }
            // Otherwise just give them the file. There is no checking for
            // what file is being sent right now
            }  else {                
                file_path.remove(0);
                file_path.as_str()
            };
            if is_debug {
                println!("file_name: {}", filename);
            }
            let contents: String;
            let mut response_type: &str;
            let mut content_type = check_content(&String::from(filename));
            // If neither index or 404 files are available use a dummy file
            if !self.has_index && file_path == "/" && !self.has_error {
                contents =
                    "<!DOCTYPE html><html><body>No index file</body></html>"
                    .to_string();
                response_type = "200 OK";
            } else {
                response_type = "200 OK";
                contents = fs::read_to_string(filename).unwrap_or_else(|_|{
                    response_type = "404 NOT FOUND";
                    content_type = "text/html".to_string();
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
                "HTTP/1.1 {}\r\nContent-Type: {}\r\n\r\n",
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

    /// Function to make a copy of a parser. Used in the server to prevent
    /// repeatedly spamming stdout if no index or 404 file is given as a new
    /// parser is made every iteration of the loop
    ///
    /// # Examples
    /// ```
    /// let parser = server::Parse::new("config.txt");
    ///
    /// let other_parser = parser.make_copy();
    /// // Move other_parser into a thread if needed
    /// ```
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

/// Function to check the content of the file based on the extension that the
/// file has. Currently only checks for css and html files otherwise returns
/// `text/plain`
///
/// # Examples
///
/// ```
/// // text/html
/// let content_type = server::check_content(&"file.html".to_string());
///
/// // text/css
/// let content_type = server::check_content(&"file.css".to_string());
///
/// // text/plain
/// let content_type = server::check_content(&"foo.bar".to_string());
/// ```
pub fn check_content(filename: &String) -> String {
    // If we do not know the extension just send it as a plaintext file
    if filename.ends_with(".css") {
        "text/css".to_string()
    } else if filename.ends_with(".html") {
        "text/html".to_string()
    } else {
        "text/plain".to_string()
    }
}
