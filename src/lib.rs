/* See LICENSE for license details */
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

    pub fn start_input(&self) -> std::thread::JoinHandle<()> {
        return self.threadpool.input();
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
}
