//! Live network producer and MF decoder. Only owned BGRA enters the existing mailbox.
use super::*;
use crate::bilibili::live::{Chunk, Stream};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::Instant;

fn requested(weak: &std::sync::Weak<Video>) -> Option<u64> {
    weak.upgrade().map(|v| v.requested.load(Ordering::Acquire))
}
fn priority() {
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}
fn fault(weak: &std::sync::Weak<Video>, error: Option<String>) {
    if let Some(v) = weak.upgrade() {
        *v.error.lock().unwrap_or_else(|e| e.into_inner()) = error;
    }
}
fn produce(mut stream: Stream, weak: std::sync::Weak<Video>, tx: SyncSender<(u64, Chunk)>) {
    priority();
    let mut epoch = 0;
    let mut pending = None;
    let mut retry = Instant::now();
    while let Some(current) = requested(&weak) {
        if epoch != current {
            epoch = current;
            pending = None;
            stream.reset();
            retry = Instant::now();
        }
        if current == 0 || Instant::now() < retry {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        if let Some(chunk) = pending.take() {
            match tx.try_send((epoch, chunk)) {
                Ok(()) => {}
                Err(TrySendError::Full((_, chunk))) => {
                    pending = Some(chunk);
                    thread::sleep(Duration::from_millis(50));
                }
                Err(TrySendError::Disconnected(_)) => break,
            }
            continue;
        }
        match stream.next_chunk(|| requested(&weak) != Some(epoch)) {
            Ok(Some(chunk)) => {
                pending = Some(chunk);
            }
            Ok(None) => {
                retry = Instant::now() + Duration::from_millis(350);
            }
            Err(e) => {
                fault(&weak, Some(e.to_string()));
                stream.reset(); // obtains fresh CDN URLs and current account on retry
                retry = Instant::now() + Duration::from_secs(3);
            }
        }
    }
}

fn open(chunk: &Chunk, room: u64, width: u32, height: u32) -> Result<Reader> {
    let mut reader = Reader::open(&chunk.path, width, height)?;
    // Never publish signed URLs or temporary file paths into game/UI logs.
    reader.label = format!("bilibili-live:{room}");
    Ok(reader)
}

struct Timeline {
    origin: Option<i64>,
    base: Duration,
}
impl Timeline {
    fn new() -> Self {
        Self {
            origin: None,
            base: Duration::ZERO,
        }
    }
    fn map(&mut self, pts: i64, now: Duration, discontinuity: bool) -> Duration {
        if self.origin.is_none() || discontinuity {
            self.origin = Some(pts);
            self.base = now + Duration::from_millis(200);
        }
        let delta = pts.saturating_sub(self.origin.unwrap()).max(0) as u64;
        let mapped = self
            .base
            .saturating_add(Duration::from_nanos(delta.saturating_mul(100)));
        // Stream restarts and large timestamp jumps must not freeze the mailbox.
        if mapped > now + Duration::from_secs(15) || mapped + Duration::from_secs(5) < now {
            self.origin = Some(pts);
            self.base = now + Duration::from_millis(200);
            self.base
        } else {
            mapped
        }
    }
}

fn decode(
    weak: std::sync::Weak<Video>,
    rx: Receiver<(u64, Chunk)>,
    room: u64,
    width: u32,
    height: u32,
) {
    let mut epoch = 0;
    let mut start = Instant::now();
    let mut timeline = Timeline::new();
    let mut active: Option<(Reader, Chunk)> = None;
    let mut discontinuity = false;
    loop {
        let Some(video) = weak.upgrade() else { break };
        let current = video.requested.load(Ordering::Acquire);
        if epoch != current {
            active = None;
            epoch = current;
            start = Instant::now();
            timeline = Timeline::new();
            video
                .queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
        }
        let full = video.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= CAPACITY;
        drop(video);
        if current == 0 {
            // Release queued files while hidden. Producer also stops on this epoch.
            while rx.try_recv().is_ok() {}
            thread::park_timeout(Duration::from_millis(100));
            continue;
        }
        if full {
            thread::park_timeout(Duration::from_millis(25));
            continue;
        }
        if active.is_none() {
            match rx.try_recv() {
                Ok((generation, chunk)) if generation == epoch => {
                    match open(&chunk, room, width, height) {
                        Ok(reader) => {
                            discontinuity = chunk.discontinuity;
                            active = Some((reader, chunk));
                            fault(&weak, None);
                        }
                        Err(e) => {
                            fault(&weak, Some(e.to_string()));
                        }
                    }
                }
                Ok(_) => continue,
                Err(mpsc::TryRecvError::Empty) => {
                    thread::park_timeout(Duration::from_millis(25));
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
            continue;
        }
        let result = active.as_mut().expect("active reader").0.read();
        match result {
            Ok(Some((pts, _, pixels))) => {
                let stamp = timeline.map(pts, start.elapsed(), discontinuity);
                discontinuity = false;
                if let Some(v) = weak.upgrade()
                    && v.requested.load(Ordering::Acquire) == epoch
                {
                    v.queue
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_back(Frame {
                            epoch,
                            pts: stamp,
                            pixels,
                        });
                }
            }
            Ok(None) => {
                active = None;
            }
            Err(e) => {
                fault(&weak, Some(e.to_string()));
                active = None;
                timeline = Timeline::new();
            }
        }
    }
}

pub fn load_live(
    room: u64,
    width: u32,
    height: u32,
    account: Option<crate::bilibili::auth::AccountKey>,
) -> Result<Arc<Video>> {
    let (tx, rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("iidxbg-live-decode".into())
        .spawn(move || {
            priority();
            let setup = || -> Result<_> {
                let platform = Platform::start()?;
                let mut stream = Stream::new(room, account)?;
                let deadline = Instant::now() + Duration::from_secs(18);
                let chunk = loop {
                    if Instant::now() >= deadline {
                        return Err(Error::Video("直播首帧缓冲超时".into()));
                    }
                    if let Some(chunk) = stream.next_chunk(|| Instant::now() >= deadline)? {
                        break chunk;
                    }
                    thread::sleep(Duration::from_millis(350));
                };
                let mut reader = open(&chunk, room, width, height)?;
                let (_, _, first) = reader
                    .read()?
                    .ok_or_else(|| Error::Video("直播分片没有视频帧".into()))?;
                // MF handles close before TempPath removes this preparation chunk.
                drop(reader);
                drop(chunk);
                Ok((platform, stream, Arc::new(Video::new(first))))
            };
            let (_platform, stream, video) = match setup() {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let _ = video.worker.set(thread::current());
            let weak = Arc::downgrade(&video);
            let (chunks_tx, chunks_rx) = mpsc::sync_channel(2);
            let producer_weak = weak.clone();
            if let Err(e) = thread::Builder::new()
                .name("iidxbg-live-network".into())
                .spawn(move || produce(stream, producer_weak, chunks_tx))
            {
                let _ = tx.send(Err(Error::Io(e)));
                return;
            }
            if tx.send(Ok(video)).is_err() {
                return;
            }
            decode(weak, chunks_rx, room, width, height);
        })?;
    rx.recv_timeout(Duration::from_secs(25))
        .map_err(|_| Error::Video("直播首帧缓冲超时".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeline_continues_across_chunks_and_rebases_after_restart() {
        let mut t = Timeline::new();
        assert_eq!(
            t.map(90_000_000, Duration::from_secs(1), true),
            Duration::from_millis(1200)
        );
        assert_eq!(
            t.map(100_000_000, Duration::from_secs(2), false),
            Duration::from_millis(2200)
        );
        assert_eq!(
            t.map(0, Duration::from_secs(3), true),
            Duration::from_millis(3200)
        );
        assert_eq!(
            t.map(900_000_000, Duration::from_secs(4), false),
            Duration::from_millis(4200)
        );
    }
}
