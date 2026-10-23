use crate::{Error, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "profiles/iidx33/010-20260819.rs"]
mod iidx33_010_20260819;
#[path = "profiles/iidx34/010-20260916.rs"]
mod iidx34_010_20260916;
#[path = "profiles/iidx34/010-20260917.rs"]
mod iidx34_010_20260917;

#[derive(Serialize)]
pub struct Profile {
    pub label: String,
    pub sha256: String,
    pub image_size: usize,
    pub layout: Layout,
    pub surface_data: BTreeMap<String, usize>,
    pub rvas: BTreeMap<String, usize>,
    pub guards: Vec<Guard>,
    pub tables: Vec<Table>,
}

#[derive(Serialize)]
pub struct Layout {
    pub scene_vtable: usize,
    pub scene_background: usize,
    pub background_archive: usize,
    pub background_movie: usize,
}

#[derive(Serialize)]
pub struct Guard {
    pub rva: usize,
    pub hex: String,
}

impl Guard {
    pub fn check(&self, function: &str, live: &[u8]) -> Result<()> {
        let expected = hex::decode(&self.hex).map_err(|_| Error::Build("embedded guard"))?;
        if live != expected {
            return Err(Error::EntryMismatch {
                function: function.into(),
                rva: self.rva,
                expected: self.hex.clone(),
                actual: hex::encode(live),
            });
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct Table {
    pub rva: usize,
    pub slot: usize,
    pub target: usize,
}

pub fn supported() -> [Profile; 3] {
    [
        iidx33_010_20260819::profile(),
        iidx34_010_20260916::profile(),
        iidx34_010_20260917::profile(),
    ]
}

fn offsets(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|&(name, rva)| (name.into(), rva))
        .collect()
}

// Keep the RVA and its guard together in each build file, so a new native
// function cannot accidentally be added to the address map without a guard.
fn native_entries(entries: &[(&str, usize, &str)]) -> (BTreeMap<String, usize>, Vec<Guard>) {
    let rvas = entries
        .iter()
        .map(|&(name, rva, _)| (name.into(), rva))
        .collect();
    let guards = entries
        .iter()
        .map(|&(_, rva, hex)| Guard {
            rva,
            hex: hex.into(),
        })
        .collect();
    (rvas, guards)
}

fn select_hash(hash: &str) -> Result<Profile> {
    supported()
        .into_iter()
        .find(|p| p.sha256 == hash)
        .ok_or(Error::Build("unsupported DLL SHA-256"))
}

fn slice(data: &[u8], offset: usize, n: usize) -> Result<&[u8]> {
    data.get(offset..offset.checked_add(n).ok_or(Error::Build("PE overflow"))?)
        .ok_or(Error::Build("truncated PE"))
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        slice(data, offset, 2)?.try_into().unwrap(),
    ))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        slice(data, offset, 4)?.try_into().unwrap(),
    ))
}

pub fn image_size(data: &[u8]) -> Result<usize> {
    if slice(data, 0, 2)? != b"MZ" {
        return Err(Error::Build("DOS header"));
    }
    let pe = u32_at(data, 0x3c)? as usize;
    if slice(data, pe, 4)? != b"PE\0\0"
        || u16_at(data, pe + 4)? != 0x8664
        || u16_at(data, pe + 24)? != 0x20b
    {
        return Err(Error::Build("not AMD64 PE32+"));
    }
    Ok(u32_at(data, pe + 24 + 56)? as usize)
}

pub fn verify_file(data: &[u8]) -> Result<Profile> {
    let p = select_hash(&hex::encode(Sha256::digest(data)))?;
    if image_size(data)? != p.image_size {
        return Err(Error::Build("image size"));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_modification_is_reported_separately_from_file_identity() {
        for p in supported() {
            for guard in p.guards {
                let name = p.rvas.iter().find(|(_, rva)| **rva == guard.rva).unwrap().0;
                let mut live = hex::decode(&guard.hex).unwrap();
                guard.check(name, &live).unwrap();
                live[0] ^= 0xff;
                let error = guard.check(name, &live).unwrap_err();
                match &error {
                    Error::EntryMismatch {
                        function,
                        rva,
                        expected,
                        actual,
                    } => {
                        assert_eq!(function, name);
                        assert_eq!(*rva, guard.rva);
                        assert_eq!(expected, &guard.hex);
                        assert_eq!(actual, &hex::encode(live));
                    }
                    _ => panic!("live modification misclassified"),
                }
                assert!(!error.to_string().contains("malformed"));
            }
        }
    }

    #[test]
    fn rejects_other_builds_and_truncated_pe() {
        assert!(verify_file(b"MZ").is_err());
        for data in [vec![], vec![0; 64], [b"MZ".as_slice(), &[0; 62]].concat()] {
            assert!(image_size(&data).is_err());
        }
        assert!(select_hash(&"0".repeat(64)).is_err());
        for p in supported() {
            assert_eq!(select_hash(&p.sha256).unwrap().label, p.label);
            assert_eq!(p.tables.len(), 3);
            for (table, name) in
                p.tables
                    .iter()
                    .zip(["background_update", "result_cleanup", "result_delete"])
            {
                assert_eq!(table.target, p.rvas[name]);
                assert!(table.rva + table.slot * 8 + 8 < p.image_size);
            }
            for rva in p.rvas.values() {
                assert!(p.guards.iter().any(|g| g.rva == *rva));
            }
            for g in p.guards {
                assert_eq!(hex::decode(g.hex).unwrap().len(), 16);
                assert!(g.rva + 16 < p.image_size);
            }
        }
    }
}
