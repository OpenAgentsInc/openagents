//! Unpacking a release archive: `coder-<v>-<platform>.tar.gz` on macOS and
//! Linux, `.zip` on Windows. Only regular files at the archive's top level
//! whose names the caller asks for are written; a wanted name that is a
//! link, a directory, or appears twice refuses the archive.

use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::Path,
};

/// The most one unpacked command may hold.
const MAX_ENTRY: u64 = 1 << 30;

/// Writes each wanted top-level file of `archive` into `out`.
pub fn unpack(archive: &Path, wanted: &[String], out: &Path) -> Result<(), String> {
    let name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let result = if name.ends_with(".zip") {
        unzip(archive, wanted, out)
    } else {
        untar_gz(archive, wanted, out)
    };
    result?;
    for want in wanted {
        if !out.join(want).is_file() {
            return Err(format!("{name} has no {want}."));
        }
    }
    Ok(())
}

/// The top-level name an archive path stands for, if it is one.
fn top_level(path: &str) -> Option<&str> {
    let path = path.strip_prefix("./").unwrap_or(path);
    let path = path.strip_suffix('/').unwrap_or(path);
    (!path.is_empty() && !path.contains(['/', '\\']) && path != ".." && path != ".").then_some(path)
}

fn write_entry(
    out: &Path,
    name: &str,
    mut reader: impl Read,
    seen: &mut HashSet<String>,
) -> Result<(), String> {
    if !seen.insert(name.to_owned()) {
        return Err(format!("The archive holds {name} twice."));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join(name))
        .map_err(|error| format!("Cannot unpack {name}: {error}."))?;
    let copied = io::copy(&mut (&mut reader).take(MAX_ENTRY + 1), &mut file)
        .map_err(|error| format!("Cannot unpack {name}: {error}."))?;
    if copied > MAX_ENTRY {
        return Err(format!("{name} in the archive is too large."));
    }
    file.flush()
        .map_err(|error| format!("Cannot unpack {name}: {error}."))
}

fn untar_gz(archive: &Path, wanted: &[String], out: &Path) -> Result<(), String> {
    let file =
        fs::File::open(archive).map_err(|error| format!("Cannot open the archive: {error}."))?;
    let mut tar = flate2::read::GzDecoder::new(io::BufReader::new(file));
    let mut seen = HashSet::new();
    let mut long_name: Option<String> = None;
    let bad = |what: &str| format!("The archive is damaged ({what}).");
    loop {
        let mut header = [0u8; 512];
        if let Err(error) = tar.read_exact(&mut header) {
            return if error.kind() == io::ErrorKind::UnexpectedEof {
                Err(bad("it ends early"))
            } else {
                Err(bad(&error.to_string()))
            };
        }
        if header.iter().all(|byte| *byte == 0) {
            return Ok(());
        }
        let stored: u32 = header[148..156]
            .iter()
            .take_while(|byte| **byte != 0 && **byte != b' ')
            .try_fold(0u32, |sum, byte| {
                (b'0'..=b'7')
                    .contains(byte)
                    .then(|| sum * 8 + u32::from(byte - b'0'))
            })
            .ok_or_else(|| bad("a header checksum"))?;
        let computed: u32 = header
            .iter()
            .enumerate()
            .map(|(at, byte)| {
                if (148..156).contains(&at) {
                    u32::from(b' ')
                } else {
                    u32::from(*byte)
                }
            })
            .sum();
        if stored != computed {
            return Err(bad("a header checksum"));
        }
        let size = tar_size(&header[124..136]).ok_or_else(|| bad("an entry size"))?;
        let kind = header[156];
        let mut name = long_name.take().unwrap_or_else(|| {
            let field = |range: std::ops::Range<usize>| {
                let bytes = &header[range];
                let end = bytes
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            };
            let base = field(0..100);
            let prefix = if &header[257..262] == b"ustar" {
                field(345..500)
            } else {
                String::new()
            };
            if prefix.is_empty() {
                base
            } else {
                format!("{prefix}/{base}")
            }
        });
        let padded = size.div_ceil(512) * 512;
        let mut data = (&mut tar).take(size);
        match kind {
            b'x' | b'L' => {
                if size > 1 << 20 {
                    return Err(bad("an extended header"));
                }
                let mut text = Vec::new();
                data.read_to_end(&mut text)
                    .map_err(|e| bad(&e.to_string()))?;
                if kind == b'L' {
                    let end = text
                        .iter()
                        .position(|byte| *byte == 0)
                        .unwrap_or(text.len());
                    long_name = Some(String::from_utf8_lossy(&text[..end]).into_owned());
                } else {
                    long_name = pax_path(&text);
                }
                name.clear();
            }
            b'0' | 0 | b'7' => {
                if let Some(top) = top_level(&name).filter(|top| wanted.iter().any(|w| w == top)) {
                    let top = top.to_owned();
                    write_entry(out, &top, &mut data, &mut seen)?;
                }
            }
            _ => {
                if let Some(top) = top_level(&name).filter(|top| wanted.iter().any(|w| w == top)) {
                    return Err(format!("The archive's {top} is not a regular file."));
                }
            }
        }
        io::copy(&mut data, &mut io::sink()).map_err(|e| bad(&e.to_string()))?;
        io::copy(&mut (&mut tar).take(padded - size), &mut io::sink())
            .map_err(|e| bad(&e.to_string()))?;
    }
}

/// An octal size, or the GNU base-256 form.
fn tar_size(field: &[u8]) -> Option<u64> {
    if field[0] & 0x80 != 0 {
        let mut value = u64::from(field[0] & 0x7f);
        for byte in &field[1..] {
            value = value.checked_mul(256)?.checked_add(u64::from(*byte))?;
        }
        return Some(value);
    }
    let text = field
        .iter()
        .skip_while(|byte| **byte == b' ')
        .take_while(|byte| **byte != 0 && **byte != b' ');
    let mut value = 0u64;
    for byte in text {
        if !(b'0'..=b'7').contains(byte) {
            return None;
        }
        value = value.checked_mul(8)?.checked_add(u64::from(byte - b'0'))?;
    }
    Some(value)
}

/// The `path` record of a pax extended header (`<len> path=<value>\n`).
fn pax_path(text: &[u8]) -> Option<String> {
    let mut rest = text;
    while !rest.is_empty() {
        let space = rest.iter().position(|byte| *byte == b' ')?;
        let length: usize = std::str::from_utf8(&rest[..space]).ok()?.parse().ok()?;
        if length <= space || length > rest.len() {
            return None;
        }
        let record = &rest[space + 1..length];
        let record = record.strip_suffix(b"\n").unwrap_or(record);
        if let Some(value) = record.strip_prefix(b"path=") {
            return Some(String::from_utf8_lossy(value).into_owned());
        }
        rest = &rest[length..];
    }
    None
}

fn unzip(archive: &Path, wanted: &[String], out: &Path) -> Result<(), String> {
    let bytes = fs::read(archive).map_err(|error| format!("Cannot open the archive: {error}."))?;
    let bad = |what: &str| format!("The archive is damaged ({what}).");
    let u16_at = |at: usize| -> Result<usize, String> {
        bytes
            .get(at..at + 2)
            .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
            .ok_or_else(|| bad("it ends early"))
    };
    let u32_at = |at: usize| -> Result<u32, String> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| bad("it ends early"))
    };
    let floor = bytes.len().saturating_sub(22 + 65_535);
    let end = (floor..=bytes.len().saturating_sub(22))
        .rev()
        .find(|at| bytes.get(*at..*at + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or_else(|| bad("no directory"))?;
    let count = u16_at(end + 10)?;
    let mut at = u32_at(end + 16)? as usize;
    if at == u32::MAX as usize {
        return Err(bad("zip64 is not supported"));
    }
    let mut seen = HashSet::new();
    for _ in 0..count {
        if u32_at(at)? != 0x0201_4b50 {
            return Err(bad("a directory entry"));
        }
        let flags = u16_at(at + 8)?;
        let method = u16_at(at + 10)?;
        let crc = u32_at(at + 16)?;
        let packed = u32_at(at + 20)? as usize;
        let size = u64::from(u32_at(at + 24)?);
        let name_len = u16_at(at + 28)?;
        let extra_len = u16_at(at + 30)?;
        let comment_len = u16_at(at + 32)?;
        let mode = u32_at(at + 38)? >> 16;
        let local = u32_at(at + 42)? as usize;
        let name = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(|| bad("a name"))?;
        let name = String::from_utf8_lossy(name).into_owned();
        at += 46 + name_len + extra_len + comment_len;
        let Some(top) = top_level(&name).filter(|top| wanted.iter().any(|w| w == top)) else {
            continue;
        };
        let top = top.to_owned();
        let is_link = mode & 0o170_000 == 0o120_000;
        if name.ends_with('/') || is_link {
            return Err(format!("The archive's {top} is not a regular file."));
        }
        if flags & 1 != 0 {
            return Err(format!("The archive's {top} is encrypted."));
        }
        if u32_at(local)? != 0x0403_4b50 {
            return Err(bad("a file header"));
        }
        let data_at = local + 30 + u16_at(local + 26)? + u16_at(local + 28)?;
        let data = bytes
            .get(data_at..data_at + packed)
            .ok_or_else(|| bad("it ends early"))?;
        let mut hasher = crc32fast::Hasher::new();
        let checked = Crc {
            inner: Box::new(match method {
                0 => Box::new(data) as Box<dyn Read>,
                8 => Box::new(flate2::read::DeflateDecoder::new(data)),
                _ => return Err(format!("The archive's {top} uses an unknown compression.")),
            }),
            hasher: &mut hasher,
        };
        write_entry(out, &top, checked, &mut seen)?;
        let written = fs::metadata(out.join(&top)).map(|m| m.len()).unwrap_or(0);
        if hasher.finalize() != crc || written != size {
            return Err(format!("The archive's {top} is damaged."));
        }
    }
    Ok(())
}

struct Crc<'a> {
    inner: Box<dyn Read + 'a>,
    hasher: &'a mut crc32fast::Hasher,
}

impl Read for Crc<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}
