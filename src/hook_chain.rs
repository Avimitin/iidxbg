//! Inspect the x64 MinHook form already supported by bilimani's IIDX adapter.
//! Callers continue calling the game entry, never the discovered detour directly.
use thiserror::Error;

#[derive(Debug, PartialEq)]
pub struct Chain {
    pub relay: usize,
    pub target: usize,
    pub module: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum ChainError {
    #[error("entry is not E9 rel32 with the remaining original bytes intact")]
    Entry,
    #[error("relay address overflow")]
    Overflow,
    #[error("relay is not a separate executable address")]
    RelayAddress,
    #[error("relay is unreadable")]
    Read,
    #[error("relay is not FF 25 00 00 00 00 followed by a 64-bit destination")]
    RelayEncoding,
    #[error("detour destination is self-referential or non-executable")]
    Target,
    #[error("detour destination is not in a loaded external module")]
    Module,
}

pub fn inspect(
    address: usize,
    original: &[u8],
    entry: &[u8],
    read: impl Fn(usize, usize) -> Option<Vec<u8>>,
    executable: impl Fn(usize) -> bool,
    external_module: impl Fn(usize) -> Option<String>,
) -> Result<Option<Chain>, ChainError> {
    if original.len() < 6 || entry.len() != original.len() {
        return Err(ChainError::Entry);
    }
    if entry == original {
        return Ok(None);
    }
    if entry[0] != 0xe9 || entry[5..] != original[5..] {
        return Err(ChainError::Entry);
    }
    let displacement = i32::from_le_bytes(entry[1..5].try_into().unwrap());
    let relay = address
        .checked_add(5)
        .and_then(|next| next.checked_add_signed(displacement as isize))
        .ok_or(ChainError::Overflow)?;
    if relay == address || !executable(relay) {
        return Err(ChainError::RelayAddress);
    }
    let code = read(relay, 14).ok_or(ChainError::Read)?;
    if code.len() != 14 || code[..6] != [0xff, 0x25, 0, 0, 0, 0] {
        return Err(ChainError::RelayEncoding);
    }
    let target = u64::from_le_bytes(code[6..14].try_into().unwrap());
    let target = usize::try_from(target).map_err(|_| ChainError::Target)?;
    if target == address || target == relay || !executable(target) {
        return Err(ChainError::Target);
    }
    let module = external_module(target).ok_or(ChainError::Module)?;
    Ok(Some(Chain {
        relay,
        target,
        module,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const ENTRY: usize = 0x10000;
    const RELAY: usize = 0x8000;
    const TARGET: usize = 0x700000;

    fn patched(original: &[u8]) -> Vec<u8> {
        let mut entry = original.to_vec();
        entry[0] = 0xe9;
        entry[1..5].copy_from_slice(&((RELAY as isize - ENTRY as isize - 5) as i32).to_le_bytes());
        entry
    }

    fn relay(target: usize) -> Vec<u8> {
        let mut code = vec![0xff, 0x25, 0, 0, 0, 0];
        code.extend((target as u64).to_le_bytes());
        code
    }

    #[test]
    fn accepts_original_and_preserved_entry_chains_for_both_profiles() {
        for p in crate::profile::supported() {
            for guard in p.guards {
                let original = hex::decode(guard.hex).unwrap();
                assert_eq!(
                    inspect(
                        ENTRY,
                        &original,
                        &original,
                        |_, _| None,
                        |_| false,
                        |_| None
                    )
                    .unwrap(),
                    None
                );
                assert_eq!(
                    inspect(
                        ENTRY,
                        &original,
                        &patched(&original),
                        |address, size| {
                            assert_eq!((address, size), (RELAY, 14));
                            Some(relay(TARGET))
                        },
                        |_| true,
                        |_| Some("existing-plugin.dll".into())
                    )
                    .unwrap(),
                    Some(Chain {
                        relay: RELAY,
                        target: TARGET,
                        module: "existing-plugin.dll".into()
                    })
                );
            }
        }
    }

    #[test]
    fn rejects_changed_tail_bad_relay_unmapped_targets_and_loops() {
        let original = vec![0x48; 16];
        let entry = patched(&original);
        let check = |bytes: &[u8], code: Option<Vec<u8>>, executable: bool, module: bool| {
            inspect(
                ENTRY,
                &original,
                bytes,
                |_, _| code.clone(),
                |_| executable,
                |_| module.then(|| "existing-plugin.dll".into()),
            )
        };
        let mut tail = entry.clone();
        tail[5] ^= 1;
        assert_eq!(
            check(&tail, Some(relay(TARGET)), true, true),
            Err(ChainError::Entry)
        );
        assert_eq!(check(&entry[..4], None, true, true), Err(ChainError::Entry));
        assert_eq!(check(&entry, None, true, true), Err(ChainError::Read));
        assert_eq!(
            check(&entry, Some(vec![0; 14]), true, true),
            Err(ChainError::RelayEncoding)
        );
        assert_eq!(
            check(&entry, Some(relay(TARGET)[..6].to_vec()), true, true),
            Err(ChainError::RelayEncoding)
        );
        assert_eq!(
            check(&entry, Some(relay(TARGET)), false, true),
            Err(ChainError::RelayAddress)
        );
        assert_eq!(
            check(&entry, Some(relay(TARGET)), true, false),
            Err(ChainError::Module)
        );
        for target in [ENTRY, RELAY] {
            assert_eq!(
                check(&entry, Some(relay(target)), true, true),
                Err(ChainError::Target)
            );
        }
        assert_eq!(
            inspect(
                ENTRY,
                &original,
                &entry,
                |_, _| Some(relay(TARGET)),
                |addr| addr == RELAY,
                |_| Some("existing-plugin.dll".into())
            ),
            Err(ChainError::Target)
        );
    }
}
