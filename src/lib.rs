use std::{
    sync::{mpsc, Arc, Mutex},
    thread, usize,
};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

// We'll note here that the job is _just_ the function
// that we want to pass to the worker. There's nothing
// super fancy here.
type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug)]
pub struct PoolCreationError;

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        // we use the sender as the "manager" of the workers, because they're
        // on multiple threads. We need a way to communicate with them.
        let (sender, receiver) = mpsc::channel();

        // There's a lengthy explanation about why we chose to use the Arc<Mutex>> pattern
        // here. Fundamentally, it comes down to the Multiple Producer, Single Consumer definition
        // of channels in Rust. We only have one receiver and cannot simply clone it to have
        // it run on multiple threads. Implementing it this way allows us to have a queue where
        // only 1 thread reads from the queue at a time.
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            // create some threads and store them
            workers.push(Worker::new(id, Arc::clone(&receiver)))
        }
        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }

    // pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError> {
    //     if size <= 0 {
    //         return Err(PoolCreationError);
    //     }
    //     let mut threads = Vec::with_capacity(size);
    //
    //     for _ in 0..size {
    //         // create some threads and store them
    //     }
    //     Ok(ThreadPool { threads })
    // }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            // every thread will loop indefinitely and take a job off
            // the receiver whenever there is one.
            // Remember: There is only 1 receiver, so we need to lock
            // the use of the receiver and make sure that we read the
            // job off the queue -- this might lead to non-deterministic
            // behaviour if one thread finishes before we exhaust the threadpool.
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down");
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

//
// +-----------------------+       +-----------------------+
// |                       |       |                       |
// |     Main Thread       |       |      Task Queue       |
// |                       |       |                       |
// +-----------+-----------+       +-----------+-----------+
//             |                               ^
//             |                               |
//             v                               |
// +-----------+-----------+       +-----------+-----------+
// |                       |       |                       |
// |      Thread Pool      |-------+   (Shared Queue)      |
// |                       |       |                       |
// +-----------+-----------+       +-----------+-----------+
//             |                               ^
//             |                               |
//    +--------+--------+                      |
//    |        |        |                      |
//    v        v        v                      |
// +---+    +---+    +---+                     |
// |   |    |   |    |   |                     |
// | W |    | W |    | W |                     |
// | o |    | o |    | o |                     |
// | r |    | r |    | r |                     |
// | k |    | k |    | k |                     |
// | e |    | e |    | e |                     |
// | r |    | r |    | r |                     |
// | 1 |    | 2 |    | N |                     |
// |   |    |   |    |   |                     |
// +---+    +---+    +---+                     |
//             ^        ^                      |
//             |        |                      |
//             +--------+----------------------+
//
