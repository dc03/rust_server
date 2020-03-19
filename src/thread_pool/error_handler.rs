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
    /// Function to create and return a new error handler. This is just a
    /// helper struct to provide the threadpool the means to handle any error
    /// it happens to have
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

    /// Function to send the error handling thread any errors that may occur.
    /// Opens the log file depending on the type of error (fatal / nonfatal).
    /// A fatal error always results in the threadpool being shut off
    pub fn send(&self, err: ErrorType) {
        match err {
            // Useful logging for fatal and non fatal errors, though not
            // really used as much as it should be
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
                // Any fatal error results in the server being shut off,
                // which in this case is only when the user asks the server
                // to shut down
                self.err_sender.send(ErrorType::Fatal(err_fatal)).unwrap();
            }
            _ => {}
        }
    }

    /// The actual error thread that monitors messages from both the input
    /// thread as well as any errors that may be sent to it via the send()
    /// method
    pub fn close_checker(&self) -> thread::JoinHandle<()> {
        let sender = mpsc::Sender::clone(&self.err_sender);
        let num = self.num;
        let comms_recv = Arc::clone(&self.comms_recv);
        let err_recv = Arc::clone(&self.err_receiver);
        let input_sender = mpsc::Sender::clone(&self.input_sender);
        let thread = thread::Builder::new()
            .name("error_handler".to_string())
            .spawn(move || loop {
                // Listen on the input thread to check if the server has to be
                // shut down
                let msg = comms_recv.lock().unwrap().try_recv().unwrap_or_else(
                    |_| ErrorType::Nothing(String::from("Nothing")),
                );
                // First shut off the input thread to prevent the user from
                // doing anything strange with the server then shut off the
                // workers
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
                let err_maybe =
                    err_recv.lock().unwrap().try_recv().unwrap_or_else(|_| {
                        ErrorType::Nothing(String::from("Nothing"))
                    });
                if let ErrorType::Fatal(error) = err_maybe {
                    input_sender
                        .send(ErrorType::Fatal(String::from(&error)))
                        .unwrap();
                    for _ in 0..num {
                        sender
                            .send(ErrorType::Fatal(String::from(&error)))
                            .unwrap();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();
        return thread;
    }

    /// Accessor function for the error receiver
    pub fn get_err_recv(&self) -> Arc<Mutex<mpsc::Receiver<ErrorType>>> {
        return Arc::clone(&self.err_receiver);
    }

    /// Accessor function to get the sender for the input thread
    pub fn get_comms_sender(&self) -> mpsc::Sender<ErrorType> {
        return mpsc::Sender::clone(&self.comms_sender);
    }

    /// I do not know what this is for
    #[allow(dead_code)]
    pub fn get_input_recv(&self) -> Arc<Mutex<mpsc::Receiver<ErrorType>>> {
        return Arc::clone(&self.input_recv);
    }
}
