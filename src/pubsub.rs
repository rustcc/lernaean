use crossbeam_channel::{Receiver, Sender};
use futures::Future;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
};

pub fn new_pair() -> (Publisher, Subscriber) {
    let finished = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = crossbeam_channel::unbounded();

    (
        Publisher {
            finished: finished.clone(),
            receiver,
        },
        Subscriber { finished, sender },
    )
}

pub struct Publisher {
    finished: Arc<AtomicBool>,
    receiver: Receiver<Waker>,
}

impl Publisher {
    pub fn finish(self) {
        self.finished.store(true, Ordering::SeqCst);
        while let Ok(waker) = self.receiver.try_recv() {
            waker.wake()
        }
    }
}

#[derive(Clone)]
pub struct Subscriber {
    finished: Arc<AtomicBool>,
    sender: Sender<Waker>,
}

impl Future for Subscriber {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let _ = self.sender.send(cx.waker().clone());

        if self.finished.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
