/* See LICENSE for license details */
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

extern crate chrono;
use chrono::prelude::*;

pub enum ErrorType {
    #[allow(dead_code)]
    NonFatal(String),
    Fatal(String),
    Nothing(String),
}

pub struct ErrorHandler {
    err_sender: mpsc::Sender<ErrorType>,
    err_receiver: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
    num: usize,
    comms_recv: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
    comms_sender: mpsc::Sender<ErrorType>,
    #[allow(dead_code)]
    input_recv: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
    input_sender: mpsc::Sender<ErrorType>,
}

impl ErrorHandler {
    pub fn new(num: usize) -> ErrorHandler {
        let (err_sender, err_receiver) = mpsc::channel();
        let err_receiver = Arc::new(Mutex::new(err_receiver));
        let (comms_sender, comms_recv) = mpsc::channel();
        let comms_recv = Arc::new(Mutex::new(comms_recv));
        let (input_sender, input_recv) = mpsc::channel();
        let input_recv = Arc::new(Mutex::new(input_recv));
        ErrorHandler {
            err_sender,
            err_receiver,
            num,
            comms_recv,
            comms_sender,
            input_recv,
            input_sender,
        }
    }

    pub fn send(&self, err: ErrorType) {
        match err {
            ErrorType::NonFatal(err_non_fatal) => {
                let time: DateTime<Local> = Local::now();
                let err_non_fatal =
                    format!("ERROR::NON_FATAL: {} at {}", err_non_fatal, time);
                println!("{}", err_non_fatal);
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open("./logs/non_fatal.txt")
                    .unwrap();
                let err_non_fatal = format!("{}\n", err_non_fatal);
                file.write_all(err_non_fatal.as_bytes()).unwrap();
            }
            ErrorType::Fatal(err_fatal) => {
                let time: DateTime<Local> = Local::now();
                let err_fatal =
                    format!("ERROR::FATAL: {} at {}", err_fatal, time);
                println!("{}", err_fatal);
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open("./logs/fatal.txt")
                    .unwrap();
                let err_fatal = format!("{}\n", err_fatal);
                file.write_all(err_fatal.as_bytes()).unwrap();
                println!("{}", err_fatal);
                self.err_sender.send(ErrorType::Fatal(err_fatal)).unwrap();
            }
            _ => {}
        }
    }

    pub fn close_checker(&self) -> thread::JoinHandle<()> {
        let sender = mpsc::Sender::clone(&self.err_sender);
        let num = self.num;
        let comms_recv = Arc::clone(&self.comms_recv);
        let err_recv = Arc::clone(&self.err_receiver);
        let input_sender = mpsc::Sender::clone(&self.input_sender);
        let thread = thread::Builder::new()
            .name("error_handler".to_string())
            .spawn(move || loop {
                let msg = comms_recv.lock().unwrap().try_recv().unwrap_or_else(
                    |_| ErrorType::Nothing(String::from("Nothing")),
                );
                if let ErrorType::Fatal(err) = msg {
                    input_sender
                        .send(ErrorType::Fatal(String::from(&err)))
                        .unwrap();
                    for _ in 0..num {
                        sender
                            .send(ErrorType::Fatal(String::from(&err)))
                            .unwrap();
                    }
                    break;
                }
                let err =
                    err_recv.lock().unwrap().try_recv().unwrap_or_else(|_| {
                        ErrorType::Nothing(String::from("Nothing"))
                    });
                if let ErrorType::Fatal(some) = err {
                    input_sender
                        .send(ErrorType::Fatal(String::from(&some)))
                        .unwrap();
                    for _ in 0..num {
                        sender
                            .send(ErrorType::Fatal(String::from(&some)))
                            .unwrap();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();
        return thread;
    }

    pub fn get_err_recv(&self) -> Arc<Mutex<mpsc::Receiver<ErrorType>>> {
        return Arc::clone(&self.err_receiver);
    }

    pub fn get_comms_sender(&self) -> mpsc::Sender<ErrorType> {
        return mpsc::Sender::clone(&self.comms_sender);
    }

    #[allow(dead_code)]
    pub fn get_input_recv(&self) -> Arc<Mutex<mpsc::Receiver<ErrorType>>> {
        return Arc::clone(&self.input_recv);
    }
}
