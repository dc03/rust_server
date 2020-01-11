/* See LICENSE for license details */
use std::io::{self, Write};
use std::sync::{atomic, atomic::Ordering, mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::thread_pool::error_handler::ErrorType;

pub struct InputParser {
    thread: Option<thread::JoinHandle<()>>,
}

impl InputParser {
    pub fn new() -> InputParser {
        let thread = Option::Some(thread::spawn(|| {}));
        InputParser { thread }
    }

    pub fn start_input(
        &'static mut self,
        err_recv: Arc<Mutex<mpsc::Receiver<ErrorType>>>,
        comms_sender: mpsc::Sender<ErrorType>,
        is_dead: &'static atomic::AtomicBool,
    ) {
        let thread = Option::Some(
            thread::Builder::new()
                .name("input_parser".to_string())
                .spawn(move || loop {
                    let msg = err_recv
                        .lock()
                        .unwrap()
                        .try_recv()
                        .unwrap_or_else(|_| ErrorType::Nothing(String::from("Nothing")));
                    match msg {
                        ErrorType::Fatal(_) => {
                            println!("Server has died. Quitting");
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
                            .send(ErrorType::Fatal(String::from("User asked to quit")))
                            .unwrap();
                        is_dead.store(true, Ordering::Relaxed);
                    }
                    thread::sleep(Duration::from_millis(500));
                })
                .unwrap(),
        );
        self.thread = thread;
    }
}

impl Drop for InputParser {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
