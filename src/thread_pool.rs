/* See LICENSE for license details */
use std::env;
use std::io::{self, Write};
use std::sync::{atomic, atomic::Ordering, mpsc, Arc, Mutex};
use std::{thread, time, time::Duration};

mod error_handler;

use error_handler::ErrorType;

pub trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

pub type Job = Box<dyn FnBox + Send + 'static>;

enum Message {
    Terminate,
    NewMessage(Job),
    Nothing(String),
}

struct Worker {
    #[allow(dead_code)]
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
    is_dead: Arc<atomic::AtomicBool>,
    error: error_handler::ErrorHandler,
    err_thread: Option<thread::JoinHandle<()>>,
    _err_recv: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
}

impl ThreadPool {
    pub fn new(num: usize) -> ThreadPool {
        let mut workers = Vec::new();
        let is_dead = Arc::new(atomic::AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let error = error_handler::ErrorHandler::new(num);
        let err_recv = error.get_err_recv();
        let err_thread = Option::Some(error.close_checker());
        for id in 0..num {
            workers.push(Worker::new(
                id,
                Arc::clone(&receiver),
                Arc::clone(&err_recv),
            ));
        }

        ThreadPool {
            workers,
            sender,
            is_dead,
            error,
            err_thread,
            _err_recv: err_recv,
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.is_dead.load(Ordering::Relaxed) {
            println!("Cannot execute");
            return;
        } else {
            let job = Box::new(f);
            self.sender
                .send(Message::NewMessage(job))
                .unwrap_or_else(|err| {
                    self.error.send(ErrorType::Fatal(String::from(format!(
                        "{:?}",
                        err
                    ))));
                    self.is_dead.store(true, Ordering::Relaxed);
                });
        }
    }

    pub fn kill(&mut self) -> usize {
        if self.is_dead.load(Ordering::Relaxed) {
            return 1;
        } else {
            println!("Killing the workers");
            for _ in &mut self.workers {
                self.sender.send(Message::Terminate).ok();
            }
            for worker in &mut self.workers {
                if let Some(thread) = worker.thread.take() {
                    thread.join().unwrap();
                }
            }
            self.is_dead.store(true, Ordering::Relaxed);
            return 0;
        }
    }

    pub fn input(&mut self) -> thread::JoinHandle<()> {
        let err_recv = Arc::clone(&self._err_recv);
        let comms_sender = self.error.get_comms_sender();
        let refer = Arc::clone(&self.is_dead);
        let thread = thread::Builder::new()
            .name("input_parser".to_string())
            .spawn(move || loop {
                if refer.load(Ordering::Relaxed) {
                    println!("Server has died. Closing input thread");
                    break;
                }
                let msg =
                    err_recv.lock().unwrap().try_recv().unwrap_or_else(|_| {
                        ErrorType::Nothing(String::from("Nothing"))
                    });
                match msg {
                    ErrorType::Fatal(_) => {
                        println!("Server has died. Closing input thread");
                        break;
                    }
                    _ => {}
                };
                print!("> ");
                io::stdout().flush().unwrap();
                let mut user_input = String::new();
                io::stdin().read_line(&mut user_input).unwrap();
                if user_input.trim() == "exit" {
                    println!("Server closing");
                    comms_sender
                        .send(ErrorType::Fatal(String::from(
                            "User asked to quit",
                        )))
                        .unwrap();
                    refer.store(true, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();

        return thread;
    }

    pub fn is_dead(&self) -> bool {
        return self.is_dead.load(Ordering::Relaxed);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.kill();
        if let Some(thread) = self.err_thread.take() {
            thread.join().unwrap_or_else(|err| {
                println!("Err: while quitting {:?}", err);
            });
        }
    }
}

impl Worker {
    fn new(
        id: usize,
        recv: Arc<Mutex<mpsc::Receiver<Message>>>,
        err_recv: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
    ) -> Worker {
        let thread = thread::Builder::new()
            .name(String::from(format!("worker_{}", id)))
            .spawn(move || {
                let is_debug = env::var("debug").is_ok();
                loop {
                    let msg =
                        recv.lock().unwrap().try_recv().unwrap_or_else(|_| {
                            Message::Nothing(String::from("Nothing"))
                        });
                    match msg {
                        Message::NewMessage(job) => {
                            if is_debug {
                                println!("Worker {} got a job, executing", id);
                            }
                            job.call_box();
                        }
                        Message::Terminate => {
                            println!("Worker {} told to terminate", id);
                            break;
                        }
                        _ => {}
                    }
                    let err =
                        err_recv.lock().unwrap().try_recv().unwrap_or_else(
                            |_| ErrorType::Nothing(String::from("Nothing")),
                        );
                    if let ErrorType::Fatal(_) = err {
                        println!("Worker {} shutting down", id);
                        break;
                    }
                    thread::sleep(time::Duration::from_millis(500));
                }
            })
            .unwrap();

        Worker {
            id,
            thread: Option::Some(thread),
        }
    }
}
