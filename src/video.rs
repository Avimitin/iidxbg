//! Bounded CPU video mailbox. No decoder or native graphics object crosses
//! into the renderer. A visit epoch rejects frames left by a previous visit.
use crate::pixels::Prepared;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod mf;
#[cfg(windows)]
pub use mf::live::load_live;
#[cfg(windows)]
pub use mf::load;

const CAPACITY: usize = 3;
struct Frame {
    epoch: u64,
    pts: Duration,
    pixels: Arc<Prepared>,
}

pub struct Video {
    pub first: Arc<Prepared>,
    queue: Mutex<VecDeque<Frame>>,
    serial: AtomicU64,
    requested: AtomicU64,
    worker: OnceLock<thread::Thread>,
    error: Mutex<Option<String>>,
    session: Mutex<Weak<Session>>,
}

impl Video {
    pub(crate) fn lease(self: &Arc<Self>) -> Option<Arc<Session>> {
        let mut shared = self.session.try_lock().ok()?;
        if let Some(session) = shared.upgrade() {
            return Some(session);
        }
        let session = Arc::new(Session {
            video: self.clone(),
            epoch: self.begin(),
            start: Instant::now(),
            current: Mutex::new(self.first.clone()),
        });
        *shared = Arc::downgrade(&session);
        Some(session)
    }
    pub(crate) fn new(first: Arc<Prepared>) -> Self {
        Self {
            first,
            queue: Mutex::new(VecDeque::with_capacity(CAPACITY)),
            serial: AtomicU64::new(0),
            requested: AtomicU64::new(0),
            worker: OnceLock::new(),
            error: Mutex::new(None),
            session: Mutex::new(Weak::new()),
        }
    }

    fn wake(&self) {
        if let Some(worker) = self.worker.get() {
            worker.unpark();
        }
    }

    /// Returns immediately; seeking and decoding happen on the worker.
    pub fn begin(&self) -> u64 {
        let epoch = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        self.requested.store(epoch, Ordering::Release);
        self.wake();
        epoch
    }

    pub fn stop(&self) {
        self.requested.store(0, Ordering::Release);
        self.wake();
    }

    /// Try-lock only. Hold the last displayed frame if the worker is busy or
    /// late. Drain overdue frames, but never display a future or stale frame.
    pub fn frame(&self, epoch: u64, elapsed: Duration) -> Option<Arc<Prepared>> {
        let mut queue = self.queue.try_lock().ok()?;
        let mut ready = None;
        while let Some(front) = queue.front() {
            if front.epoch != epoch {
                queue.pop_front();
            } else if front.pts <= elapsed {
                ready = queue.pop_front().map(|f| f.pixels);
            } else {
                break;
            }
        }
        drop(queue);
        self.wake();
        ready
    }

    /// Read on the provider worker, never do logging/IO on the decoder callback.
    pub fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn failed(&self) -> bool {
        self.error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }
}

pub(crate) struct Session {
    video: Arc<Video>,
    epoch: u64,
    start: Instant,
    current: Mutex<Arc<Prepared>>,
}

impl Session {
    pub(crate) fn frame(&self) -> Option<Arc<Prepared>> {
        let mut current = self.current.try_lock().ok()?;
        if let Some(frame) = self.video.frame(self.epoch, self.start.elapsed()) {
            *current = frame;
        }
        Some(current.clone())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self
            .video
            .requested
            .compare_exchange(self.epoch, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            self.video.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(n: u8) -> Arc<Prepared> {
        Arc::new(Prepared {
            label: n.to_string(),
            width: 1,
            height: 1,
            bgra: vec![n; 4],
        })
    }
    #[test]
    fn discards_old_visits_and_overdue_frames_but_keeps_future() {
        let video = Video::new(image(0));
        let old = video.begin();
        video.stop();
        let epoch = video.begin();
        for (generation, ms, n) in [(old, 0, 1), (epoch, 10, 2), (epoch, 20, 3), (epoch, 30, 4)] {
            video.queue.lock().unwrap().push_back(Frame {
                epoch: generation,
                pts: Duration::from_millis(ms),
                pixels: image(n),
            });
        }
        assert_eq!(
            video.frame(epoch, Duration::from_millis(25)).unwrap().label,
            "3"
        );
        assert!(video.frame(epoch, Duration::from_millis(29)).is_none());
        assert_eq!(
            video.frame(epoch, Duration::from_millis(30)).unwrap().label,
            "4"
        );
    }
    #[test]
    fn busy_mailbox_never_blocks_renderer() {
        let video = Video::new(image(0));
        let _lock = video.queue.lock().unwrap();
        assert!(video.frame(1, Duration::ZERO).is_none());
    }

    #[test]
    fn bound_consumers_share_frames_and_last_lease_stops_decoder() {
        let video = Arc::new(Video::new(image(0)));
        let a = video.lease().unwrap();
        let b = video.lease().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        video.queue.lock().unwrap().push_back(Frame {
            epoch: a.epoch,
            pts: Duration::ZERO,
            pixels: image(9),
        });
        let first = a.frame().unwrap();
        let second = b.frame().unwrap();
        assert_eq!(first.label, "9");
        assert!(Arc::ptr_eq(&first, &second));
        let epoch = a.epoch;
        drop(a);
        assert_eq!(video.requested.load(Ordering::Acquire), epoch);
        drop(b);
        assert_eq!(video.requested.load(Ordering::Acquire), 0);
        let next = video.lease().unwrap();
        assert_ne!(next.epoch, epoch);
        assert_eq!(next.frame().unwrap().label, "0");
    }
}
