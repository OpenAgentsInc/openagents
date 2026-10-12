// SPDX-FileCopyrightText: 2026 Academa, Inc. (the take's design, from ManimGX)
// SPDX-FileCopyrightText: 2026 OpenAgents
// SPDX-License-Identifier: MIT

//! A take as bytes. A message is its length (u32), its op (u8), then its
//! fields, little-endian; a byte string is its length (u32), then its bytes.
//! A take begins with `START` (version, width, height, frame rate) and ends
//! with `END` (whether the scene failed). Between them, uploads (`PATH`,
//! `IMAGE`) name what frames draw, each under a content key, and each
//! `FRAME` carries its view, how many frames it is held, and one record per
//! drawn object. `NOTE` carries the director's notes as JSON.

use std::collections::BTreeMap;

use serde_json::Value;

/// The wire contract. Bump when an older reader would misread a new writer.
pub const VERSION: u32 = 1;

const START: u8 = 1;
const PATH: u8 = 2;
const IMAGE: u8 = 3;
const FRAME: u8 = 4;
const NOTE: u8 = 5;
const END: u8 = 6;

/// Bytes per record on the wire: key, kind, transform, two colors, width,
/// reveal, depth.
const RECORD: usize = 8 + 1 + 6 * 4 + 4 * 4 + 4 * 4 + 4 + 2 * 4 + 4;

/// A shape or an image, under its content key.
#[derive(Clone, Debug, PartialEq)]
pub enum Upload {
    /// A path of cubic Bézier curves: `3n + 1` control points.
    Path { closed: bool, points: Vec<[f32; 2]> },
    /// RGBA8 pixels, row-major.
    Image {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

/// What a record draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Path,
    Image,
}

/// One drawn object in a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub key: u64,
    pub kind: Kind,
    /// A 2×3 affine matrix, column-major: `[a, b, c, d, tx, ty]`.
    pub transform: [f32; 6],
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
    pub stroke_width: f32,
    /// The part of the path shown, as fractions of its length.
    pub reveal: [f32; 2],
    pub depth: f32,
}

impl Record {
    pub fn new(key: u64, kind: Kind) -> Self {
        Self {
            key,
            kind,
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            fill: [0.0; 4],
            stroke: [0.0; 4],
            stroke_width: 0.0,
            reveal: [0.0, 1.0],
            depth: 0.0,
        }
    }
}

/// The frame's camera: the scene rectangle shown, in scene units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub view: View,
    /// How many output frames this picture is shown for.
    pub hold: u32,
    pub records: Vec<Record>,
}

/// A take read back.
#[derive(Clone, Debug, PartialEq)]
pub struct Take {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub uploads: BTreeMap<u64, Upload>,
    pub frames: Vec<Frame>,
    pub notes: Vec<Value>,
    /// `None` when the take was cut short, else whether the scene failed.
    pub failed: Option<bool>,
}

impl Take {
    /// Output frames, counting holds.
    pub fn frame_count(&self) -> u64 {
        self.frames.iter().map(|frame| u64::from(frame.hold)).sum()
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.fps > 0.0 {
            self.frame_count() as f64 / self.fps
        } else {
            0.0
        }
    }

    /// A summary for inspection: the header, counts, and the notes.
    pub fn summary(&self) -> Value {
        let paths = self
            .uploads
            .values()
            .filter(|u| matches!(u, Upload::Path { .. }))
            .count();
        serde_json::json!({
            "version": VERSION,
            "width": self.width,
            "height": self.height,
            "fps": self.fps,
            "uploads": { "paths": paths, "images": self.uploads.len() - paths },
            "pictures": self.frames.len(),
            "frames": self.frame_count(),
            "seconds": self.duration_seconds(),
            "records": self.frames.iter().map(|f| f.records.len()).sum::<usize>(),
            "notes": self.notes,
            "failed": self.failed,
        })
    }
}

/// Records a take from Rust: uploads as they are added, then frames.
pub struct Film {
    bytes: Vec<u8>,
    keys: BTreeMap<u64, Kind>,
}

impl Film {
    pub fn new(width: u32, height: u32, fps: f64) -> Self {
        let mut film = Self {
            bytes: Vec::new(),
            keys: BTreeMap::new(),
        };
        film.message(START, |m| {
            m.extend(VERSION.to_le_bytes());
            m.extend(width.to_le_bytes());
            m.extend(height.to_le_bytes());
            m.extend(fps.to_le_bytes());
        });
        film
    }

    /// A content key for bytes: the same content gets the same key.
    pub fn key(parts: &[&[u8]]) -> u64 {
        // FNV-1a over the parts, each length-prefixed.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |byte: u8| {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for part in parts {
            for byte in (part.len() as u32).to_le_bytes() {
                eat(byte);
            }
            for &byte in *part {
                eat(byte);
            }
        }
        hash
    }

    /// Uploads a path of cubic Bézier curves (`3n + 1` points) and returns
    /// its key. The same points are uploaded once.
    pub fn path(&mut self, points: &[[f32; 2]], closed: bool) -> Result<u64, Error> {
        if points.len() < 4 || (points.len() - 1) % 3 != 0 {
            return Err(Error::Malformed(format!(
                "a path has 3n + 1 control points, not {}",
                points.len()
            )));
        }
        let flat: Vec<u8> = points
            .iter()
            .flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes()))
            .collect();
        let key = Self::key(&[b"path", &[u8::from(closed)], &flat]);
        if self.keys.insert(key, Kind::Path).is_none() {
            self.message(PATH, |m| {
                m.extend(key.to_le_bytes());
                m.push(u8::from(closed));
                m.extend((points.len() as u32).to_le_bytes());
                m.extend(&flat);
            });
        }
        Ok(key)
    }

    /// Uploads RGBA8 pixels and returns their key.
    pub fn image(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<u64, Error> {
        let expected = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected {
            return Err(Error::Malformed(format!(
                "a {width}×{height} RGBA image has {expected} bytes, not {}",
                pixels.len()
            )));
        }
        let key = Self::key(&[
            b"image",
            &width.to_le_bytes(),
            &height.to_le_bytes(),
            pixels,
        ]);
        if self.keys.insert(key, Kind::Image).is_none() {
            self.message(IMAGE, |m| {
                m.extend(key.to_le_bytes());
                m.extend(width.to_le_bytes());
                m.extend(height.to_le_bytes());
                bytes_field(m, pixels);
            });
        }
        Ok(key)
    }

    /// Records a picture shown for `hold` frames. Every record's key must be
    /// uploaded already, as the kind the record says.
    pub fn frame(&mut self, view: View, hold: u32, records: &[Record]) -> Result<(), Error> {
        if hold == 0 {
            return Err(Error::Malformed(
                "a frame is held for at least one frame".into(),
            ));
        }
        for record in records {
            match self.keys.get(&record.key) {
                Some(kind) if *kind == record.kind => {}
                Some(_) => {
                    return Err(Error::Malformed(format!(
                        "record {:016x} draws its upload as the wrong kind",
                        record.key
                    )));
                }
                None => {
                    return Err(Error::Malformed(format!(
                        "record {:016x} draws nothing uploaded",
                        record.key
                    )));
                }
            }
        }
        self.message(FRAME, |m| {
            for c in [view.x, view.y, view.width, view.height] {
                m.extend(c.to_le_bytes());
            }
            m.extend(hold.to_le_bytes());
            m.extend((records.len() as u32).to_le_bytes());
            for r in records {
                m.extend(r.key.to_le_bytes());
                m.push(match r.kind {
                    Kind::Path => 0,
                    Kind::Image => 1,
                });
                for c in r.transform {
                    m.extend(c.to_le_bytes());
                }
                for c in r.fill.into_iter().chain(r.stroke) {
                    m.extend(c.to_le_bytes());
                }
                m.extend(r.stroke_width.to_le_bytes());
                for c in r.reveal {
                    m.extend(c.to_le_bytes());
                }
                m.extend(r.depth.to_le_bytes());
            }
        });
        Ok(())
    }

    /// The director's notes: plays, sections, captions, as JSON.
    pub fn note(&mut self, note: &Value) {
        let text = note.to_string();
        self.message(NOTE, |m| bytes_field(m, text.as_bytes()));
    }

    /// Ends the take and returns its bytes.
    pub fn finish(mut self, failed: bool) -> Vec<u8> {
        self.message(END, |m| m.push(u8::from(failed)));
        self.bytes
    }

    fn message(&mut self, op: u8, fields: impl FnOnce(&mut Vec<u8>)) {
        let mut body = vec![op];
        fields(&mut body);
        self.bytes.extend((body.len() as u32).to_le_bytes());
        self.bytes.extend(body);
    }
}

fn bytes_field(m: &mut Vec<u8>, bytes: &[u8]) {
    m.extend((bytes.len() as u32).to_le_bytes());
    m.extend(bytes);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Malformed(why) => write!(f, "malformed take: {why}"),
        }
    }
}

impl std::error::Error for Error {}

struct Fields<'a>(&'a [u8]);

impl Fields<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        if self.0.len() < n {
            return Err(Error::Malformed("a message ends early".into()));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, Error> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32s<const N: usize>(&mut self) -> Result<[f32; N], Error> {
        let mut out = [0.0; N];
        for slot in &mut out {
            *slot = self.f32()?;
        }
        Ok(out)
    }
    fn bytes(&mut self) -> Result<&[u8], Error> {
        let n = self.u32()? as usize;
        self.take(n)
    }
}

/// Reads a take. A stream cut short reads as far as it goes, with
/// `failed: None`.
pub fn read(stream: &[u8]) -> Result<Take, Error> {
    let mut rest = stream;
    let mut take: Option<Take> = None;
    while rest.len() >= 4 {
        let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        if rest.len() < 4 + len || len == 0 {
            break;
        }
        let mut f = Fields(&rest[4..4 + len]);
        rest = &rest[4 + len..];
        let op = f.u8()?;
        if op == START {
            let version = f.u32()?;
            if version != VERSION {
                return Err(Error::Malformed(format!(
                    "take version {version}; this reader reads {VERSION}"
                )));
            }
            take = Some(Take {
                width: f.u32()?,
                height: f.u32()?,
                fps: f.f64()?,
                uploads: BTreeMap::new(),
                frames: Vec::new(),
                notes: Vec::new(),
                failed: None,
            });
            continue;
        }
        let Some(t) = take.as_mut() else {
            return Err(Error::Malformed(
                "the take does not begin with START".into(),
            ));
        };
        match op {
            PATH => {
                let key = f.u64()?;
                let closed = f.u8()? != 0;
                let count = f.u32()? as usize;
                let mut points = Vec::with_capacity(count);
                for _ in 0..count {
                    points.push(f.f32s::<2>()?);
                }
                t.uploads.insert(key, Upload::Path { closed, points });
            }
            IMAGE => {
                let key = f.u64()?;
                let width = f.u32()?;
                let height = f.u32()?;
                let pixels = f.bytes()?.to_vec();
                t.uploads.insert(
                    key,
                    Upload::Image {
                        width,
                        height,
                        pixels,
                    },
                );
            }
            FRAME => {
                let [x, y, width, height] = f.f32s::<4>()?;
                let hold = f.u32()?;
                let count = f.u32()? as usize;
                if f.0.len() != count * RECORD {
                    return Err(Error::Malformed(
                        "a frame's records have the wrong size".into(),
                    ));
                }
                let mut records = Vec::with_capacity(count);
                for _ in 0..count {
                    let key = f.u64()?;
                    let kind = match f.u8()? {
                        0 => Kind::Path,
                        1 => Kind::Image,
                        other => {
                            return Err(Error::Malformed(format!("record kind {other}")));
                        }
                    };
                    if !t.uploads.contains_key(&key) {
                        return Err(Error::Malformed(format!(
                            "record {key:016x} draws nothing uploaded"
                        )));
                    }
                    records.push(Record {
                        key,
                        kind,
                        transform: f.f32s::<6>()?,
                        fill: f.f32s::<4>()?,
                        stroke: f.f32s::<4>()?,
                        stroke_width: f.f32()?,
                        reveal: f.f32s::<2>()?,
                        depth: f.f32()?,
                    });
                }
                t.frames.push(Frame {
                    view: View {
                        x,
                        y,
                        width,
                        height,
                    },
                    hold,
                    records,
                });
            }
            NOTE => {
                let text = f.bytes()?;
                let note = serde_json::from_slice(text)
                    .map_err(|e| Error::Malformed(format!("a note is not JSON: {e}")))?;
                t.notes.push(note);
            }
            END => t.failed = Some(f.u8()? != 0),
            other => return Err(Error::Malformed(format!("unknown op {other}"))),
        }
    }
    take.ok_or_else(|| Error::Malformed("the take is empty".into()))
}

/// A short take for demos and tests: a square that slides across the frame
/// over `pictures` pictures, each held `hold` frames.
pub fn demo(pictures: u32, hold: u32) -> Vec<u8> {
    let mut film = Film::new(1280, 720, 60.0);
    let square = film
        .path(
            &[
                [-1.0, -1.0],
                [-1.0, -1.0],
                [1.0, -1.0],
                [1.0, -1.0],
                [1.0, -1.0],
                [1.0, 1.0],
                [1.0, 1.0],
                [1.0, 1.0],
                [-1.0, 1.0],
                [-1.0, 1.0],
                [-1.0, 1.0],
                [-1.0, -1.0],
                [-1.0, -1.0],
            ],
            true,
        )
        .expect("a square is a path");
    film.note(&serde_json::json!({ "title": "demo", "plays": [{ "name": "slide", "pictures": pictures }] }));
    let view = View {
        x: -8.0,
        y: -4.5,
        width: 16.0,
        height: 9.0,
    };
    for i in 0..pictures {
        let t = if pictures > 1 {
            i as f32 / (pictures - 1) as f32
        } else {
            0.0
        };
        let mut record = Record::new(square, Kind::Path);
        record.transform[4] = -5.0 + 10.0 * t;
        record.fill = [0.35, 0.77, 0.87, 1.0];
        record.stroke = [1.0, 1.0, 1.0, 1.0];
        record.stroke_width = 0.04;
        film.frame(view, hold, &[record])
            .expect("the square is uploaded");
    }
    film.finish(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_demo_take_reads_back_whole() {
        let bytes = demo(3, 2);
        let take = read(&bytes).unwrap();
        assert_eq!((take.width, take.height, take.fps), (1280, 720, 60.0));
        assert_eq!(take.uploads.len(), 1);
        assert_eq!(take.frames.len(), 3);
        assert_eq!(take.frame_count(), 6);
        assert_eq!(take.failed, Some(false));
        assert_eq!(take.notes[0]["title"], "demo");
        assert_eq!(take.frames[2].records[0].transform[4], 5.0);
    }

    #[test]
    fn the_same_path_is_uploaded_once() {
        let mut film = Film::new(8, 8, 1.0);
        let pts = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let a = film.path(&pts, false).unwrap();
        let b = film.path(&pts, false).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, film.path(&pts, true).unwrap());
        let take = read(&film.finish(false)).unwrap();
        assert_eq!(take.uploads.len(), 2);
    }

    #[test]
    fn a_record_of_nothing_uploaded_is_refused_by_writer_and_reader() {
        let mut film = Film::new(8, 8, 1.0);
        let view = View {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        assert!(film.frame(view, 1, &[Record::new(7, Kind::Path)]).is_err());
        assert!(film.path(&[[0.0, 0.0], [1.0, 1.0]], false).is_err());
        assert!(film.image(2, 2, &[0; 3]).is_err());
    }

    #[test]
    fn a_take_cut_short_reads_as_far_as_it_goes() {
        let bytes = demo(2, 1);
        let take = read(&bytes[..bytes.len() - 6]).unwrap();
        assert_eq!(take.frames.len(), 2);
        assert_eq!(take.failed, None);
        assert!(read(&[]).is_err());
        assert!(read(&bytes[4..]).is_err());
    }
}
