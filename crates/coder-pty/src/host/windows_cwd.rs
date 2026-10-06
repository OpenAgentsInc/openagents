//! Bounded reads of the native x64 Windows process-parameter directory.
//! These offsets describe the native x64 PEB and RTL_USER_PROCESS_PARAMETERS.
//! Unsupported layouts, inaccessible memory, and changing records refuse.

const PATH_BYTES: usize = 8192;

fn address(base: u64, offset: u64, size: usize) -> Option<u64> {
    let start = base.checked_add(offset)?;
    let end = start.checked_add(size as u64)?;
    (start != 0 && end <= isize::MAX as u64).then_some(start)
}

fn snapshot(
    peb: u64,
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
) -> Option<(u64, Vec<u8>, Vec<u8>)> {
    let pointer = read(address(peb, 0x20, 8)?, 8)?;
    let parameters = u64::from_le_bytes(pointer.as_slice().try_into().ok()?);
    if parameters == 0 || parameters % 8 != 0 {
        return None;
    }
    let header = read(address(parameters, 0, 80)?, 80)?;
    if header.len() != 80 {
        return None;
    }
    let word = |offset| u32::from_le_bytes(header[offset..offset + 4].try_into().unwrap());
    let maximum = word(0);
    let length = word(4);
    if length < 80 || maximum < length || maximum > 1024 * 1024 || word(8) & 1 == 0 {
        return None;
    }
    let length = u16::from_le_bytes(header[56..58].try_into().unwrap()) as usize;
    let maximum = u16::from_le_bytes(header[58..60].try_into().unwrap()) as usize;
    let buffer = u64::from_le_bytes(header[64..72].try_into().unwrap());
    if length == 0 || length % 2 != 0 || length > PATH_BYTES || maximum < length || buffer % 2 != 0
    {
        return None;
    }
    let bytes = read(address(buffer, 0, length)?, length)?;
    if bytes.len() != length {
        return None;
    }
    Some((parameters, header, bytes))
}

pub(super) fn inspect(
    peb: u64,
    mut read: impl FnMut(u64, usize) -> Option<Vec<u8>>,
) -> Option<String> {
    if peb == 0 || peb % 8 != 0 {
        return None;
    }
    let first = snapshot(peb, &mut read)?;
    if snapshot(peb, &mut read)? != first {
        return None;
    }
    let units: Vec<u16> = first
        .2
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let path = String::from_utf16(&units).ok()?;
    let bytes = path.as_bytes();
    if path.len() > PATH_BYTES
        || path.chars().any(char::is_control)
        || bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
    {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn memory(path: &[u16]) -> BTreeMap<u64, Vec<u8>> {
        let mut header = vec![0; 80];
        header[0..4].copy_from_slice(&80u32.to_le_bytes());
        header[4..8].copy_from_slice(&80u32.to_le_bytes());
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        let length = (path.len() * 2) as u16;
        header[56..58].copy_from_slice(&length.to_le_bytes());
        header[58..60].copy_from_slice(&length.to_le_bytes());
        header[64..72].copy_from_slice(&0x3000u64.to_le_bytes());
        BTreeMap::from([
            (0x1020, 0x2000u64.to_le_bytes().to_vec()),
            (0x2000, header),
            (0x3000, path.iter().flat_map(|u| u.to_le_bytes()).collect()),
        ])
    }
    fn check(memory: &BTreeMap<u64, Vec<u8>>) -> Option<String> {
        inspect(0x1000, |address, length| {
            memory.get(&address).filter(|b| b.len() == length).cloned()
        })
    }
    #[test]
    fn native_directory_requires_stable_bounded_os_records() {
        let memory = memory(&"C:\\scratch\\日本語".encode_utf16().collect::<Vec<_>>());
        assert_eq!(check(&memory).as_deref(), Some("C:\\scratch\\日本語"));
        let mut count = 0;
        assert!(
            inspect(0x1000, |address, length| {
                count += 1;
                let mut bytes = memory.get(&address)?.clone();
                if count == 6 {
                    bytes[6] ^= 1;
                }
                (bytes.len() == length).then_some(bytes)
            })
            .is_none()
        );
        for mutation in [0, 1, 2, 3] {
            let mut invalid = memory.clone();
            let header = invalid.get_mut(&0x2000).unwrap();
            match mutation {
                0 => header[56..58].copy_from_slice(&8194u16.to_le_bytes()),
                1 => header[56..58].copy_from_slice(&3u16.to_le_bytes()),
                2 => header[8..12].fill(0),
                _ => header[64..72].fill(0xff),
            }
            assert!(check(&invalid).is_none());
        }
        assert!(
            inspect(u64::MAX, |_, _| panic!(
                "invalid addresses must not be read"
            ))
            .is_none()
        );
    }
    #[test]
    fn invalid_unicode_controls_and_nonlocal_paths_refuse() {
        for path in [
            "relative",
            "\\\\server\\share",
            "C:\\bad\npath",
            "C:\\bad\0path",
        ] {
            assert!(check(&memory(&path.encode_utf16().collect::<Vec<_>>())).is_none());
        }
        assert!(check(&memory(&[b'C' as u16, b':' as u16, b'\\' as u16, 0xd800])).is_none());
    }
}
