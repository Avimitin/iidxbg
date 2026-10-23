//! Prepared media and graphics-side visit leases. No IO or decoding in a lease.
use crate::{
    pixels::Prepared,
    video::{Session, Video},
};
use std::sync::Arc;

#[derive(Clone)]
pub enum Asset {
    Image(Arc<Prepared>),
    Video(Arc<Video>),
}

impl std::ops::Deref for Asset {
    type Target = Prepared;
    fn deref(&self) -> &Prepared {
        match self {
            Self::Image(p) => p,
            Self::Video(v) => &v.first,
        }
    }
}

impl Asset {
    pub fn first(&self) -> Arc<Prepared> {
        match self {
            Self::Image(p) => p.clone(),
            Self::Video(v) => v.first.clone(),
        }
    }
    pub fn play(&self) -> Playback {
        Playback {
            pixels: self.first(),
            session: match self {
                Self::Video(v) => v.lease(),
                _ => None,
            },
        }
    }
}

pub struct Playback {
    pub pixels: Arc<Prepared>,
    session: Option<Arc<Session>>,
}

impl Playback {
    /// Shared leases deliver the same frame to both players; last drop stops decoding.
    pub fn update(&mut self) -> bool {
        let Some(frame) = self.session.as_ref().and_then(|s| s.frame()) else {
            return false;
        };
        if Arc::ptr_eq(&self.pixels, &frame) {
            return false;
        }
        self.pixels = frame;
        true
    }
}

pub fn is_video(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|s| {
        ["mp4", "m4v", "mov", "wmv", "avi", "mkv", "webm"]
            .iter()
            .any(|ext| s.to_string_lossy().eq_ignore_ascii_case(ext))
    })
}
