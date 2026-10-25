//! Owned login snapshots and a small UI handoff file. No PINs or refids.
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize)]
pub struct Layout {
    pub initialized: usize,
    pub joined: usize,
    pub identity: usize,
    pub ready: usize,
    pub stride: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Login {
    pub joined: [bool; 2],
    pub cards: [Option<String>; 2],
}

/// The sampler copies static globals twice. It never calls native functions or
/// retains native pointers; a torn/failed read yields stock settings.
pub fn sample(layout: &Layout, mut read: impl FnMut(usize, &mut [u8]) -> bool) -> Login {
    fn once(layout: &Layout, read: &mut impl FnMut(usize, &mut [u8]) -> bool) -> Option<Login> {
        let mut initialized = [0; 8];
        let mut joined = [0; 8];
        if !read(layout.initialized, &mut initialized) || !read(layout.joined, &mut joined) {
            return None;
        }
        let mut result = Login::default();
        for side in 0..2 {
            result.joined[side] = joined[side * 4..side * 4 + 4] != [0; 4];
            if !result.joined[side] || initialized[..4] == [0; 4] || initialized[4..] == [0; 4] {
                continue;
            }
            let mut identity = [0; 28];
            let mut ready = [0];
            if !read(layout.identity + layout.stride * side, &mut identity)
                || !read(layout.ready + layout.stride * side, &mut ready)
            {
                return None;
            }
            let card = u64::from_le_bytes(identity[..8].try_into().unwrap());
            if ready == [1] && identity[24..28] == [0; 4] && card != 0 && card != u64::MAX {
                result.cards[side] = Some(format!("{card:016X}"));
            }
        }
        Some(result)
    }
    let first = once(layout, &mut read);
    let second = once(layout, &mut read);
    if first == second {
        first.unwrap_or_default()
    } else {
        Login::default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub process_id: u32,
    pub updated_at: u64,
    pub supported: bool,
    pub login: Login,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn status_path(config: &Path) -> PathBuf {
    config.with_extension("session.json")
}

impl Status {
    pub fn fresh(&self) -> bool {
        now()
            .checked_sub(self.updated_at)
            .is_some_and(|age| age <= 5)
    }
    pub fn read(config: &Path) -> crate::Result<Self> {
        let file = std::fs::File::open(status_path(config))?;
        if file.metadata()?.len() > 4096 {
            return Err(crate::Error::Limit);
        }
        Ok(serde_json::from_reader(file)?)
    }
    pub fn write(&self, config: &Path) -> crate::Result<()> {
        let parent = config
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&serde_json::to_vec(self)?)?;
        file.persist(status_path(config)).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout() -> Layout {
        Layout {
            initialized: 0,
            joined: 8,
            identity: 16,
            ready: 44,
            stride: 32,
        }
    }
    fn memory() -> Vec<u8> {
        let mut m = vec![0; 80];
        m[0] = 1;
        m[4] = 1;
        m[8] = 1;
        m[44] = 1;
        m[16..24].copy_from_slice(&0xE004010000001234u64.to_le_bytes());
        m
    }
    #[test]
    fn card_order_guest_logout_and_both_sides() {
        let mut m = memory();
        let sample_memory = |m: &[u8]| {
            sample(&layout(), |at, out| {
                out.copy_from_slice(&m[at..at + out.len()]);
                true
            })
        };
        assert_eq!(
            sample_memory(&m).cards[0].as_deref(),
            Some("E004010000001234")
        );
        m[40] = 1;
        assert!(sample_memory(&m).cards[0].is_none());
        m[40] = 0;
        m[8] = 0;
        assert_eq!(sample_memory(&m), Login::default());
        m[12] = 1;
        m[76] = 1;
        m[48..56].copy_from_slice(&0x0123456789ABCDEFu64.to_le_bytes());
        assert_eq!(
            sample_memory(&m).cards,
            [None, Some("0123456789ABCDEF".into())]
        );
    }
    #[test]
    fn torn_or_unreadable_snapshot_fails_closed() {
        let m = memory();
        let mut reads = 0;
        assert_eq!(
            sample(&layout(), |at, out| {
                reads += 1;
                out.copy_from_slice(&m[at..at + out.len()]);
                if reads > 4 {
                    out.fill(0);
                }
                true
            }),
            Login::default()
        );
        assert_eq!(sample(&layout(), |_, _| false), Login::default());
    }
}
