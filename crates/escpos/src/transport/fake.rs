use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// How the fake printer misbehaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeMode {
    /// Accepts and reads the whole job.
    Ok,
    /// Accepts the connection, then closes it immediately.
    Refuse,
    /// Accepts the connection and never reads.
    Hang,
    /// Reads part of the job, then drops the connection.
    DieMidJob,
    /// Not listening at all.
    Offline,
}

/// An in-process stand-in for a network thermal printer.
///
/// CI has no printers, and MVP.md section 26's failure paths cannot be
/// exercised any other way.
pub struct FakePrinter {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
    got_job: Arc<Notify>,
}

impl FakePrinter {
    pub async fn start(mode: FakeMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake printer");
        let addr = listener.local_addr().expect("fake printer addr");

        let received = Arc::new(Mutex::new(Vec::new()));
        let got_job = Arc::new(Notify::new());

        if mode == FakeMode::Offline {
            // Drop the listener so the port is closed and connects are refused.
            drop(listener);
            return Self {
                addr,
                received,
                got_job,
            };
        }

        let received_task = Arc::clone(&received);
        let notify_task = Arc::clone(&got_job);

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };

                match mode {
                    FakeMode::Offline => return,
                    FakeMode::Refuse => {
                        drop(stream);
                    }
                    FakeMode::Hang => {
                        // Hold the connection open and never read from it. This
                        // future never resolves, so this task services exactly
                        // one connection and then never calls accept() again --
                        // a second sender pointed at a Hang printer will just
                        // block on connect/accept, it will not get its own hang.
                        std::future::pending::<()>().await;
                    }
                    FakeMode::DieMidJob => {
                        let mut buf = [0u8; 8];
                        let _ = stream.read(&mut buf).await;
                        drop(stream);
                    }
                    FakeMode::Ok => {
                        let mut buf = Vec::new();
                        if stream.read_to_end(&mut buf).await.is_ok() {
                            received_task.lock().expect("fake printer lock").push(buf);
                            notify_task.notify_waiters();
                        }
                    }
                }
            }
        });

        Self {
            addr,
            received,
            got_job,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Snapshot of jobs received so far.
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.received.lock().expect("fake printer lock").clone()
    }

    /// Wait for the next completed job and return its bytes.
    pub async fn wait_for_job(&self) -> Vec<u8> {
        loop {
            // Register interest BEFORE checking, so a job arriving in between
            // still wakes us. notify_waiters() stores no permit, so the reverse
            // order loses that notification permanently.
            let notified = self.got_job.notified();
            if let Some(job) = self.received().into_iter().next() {
                return job;
            }
            notified.await;
        }
    }
}
