//! The launch film's MacBook and iPhone as Grid line art.
//!
//! `scripts/blender/help_desk_devices.py` exports the film's procedural
//! devices (`scripts/film/studio/devices.py`) to glTF under
//! `assets/verse/generated/help_desk/`. A test here compiles those files
//! into [`PINNED`]: each device's faces, welded and quantized, its sharp
//! edges as lines, and its display as a textured panel. The page draws the
//! faces dark, the edges as the Grid's white lines, and the display with
//! its still screen ([`MACBOOK_SCREEN`], [`IPHONE_SCREEN`]), so the Grid
//! downloads no glTF and links no glTF reader.
//!
//! The format is little-endian: `OAHD`, a `u16` version, a `u16` device
//! count, then for each device its name (a `u8` length and bytes), the
//! bounds (`f32` x 6), the corners (`u32` count, `u16` x 3 each, quantized
//! to the bounds), the triangles (`u32` count, `u16` x 3 each), the edges
//! (`u32` count, `u16` x 2 and a `u8` tone each), and the display (`u32`
//! corner count, `f32` x 5 each for position and texture coordinates, then
//! a `u32` index count and `u16` indices).

/// The compiled devices, from the workspace root.
pub const PINNED_PATH: &str = "assets/verse/grid/help-desk-devices.bin";
/// The compiled devices.
pub const PINNED: &[u8] = include_bytes!("../../../assets/verse/grid/help-desk-devices.bin");
/// The laptop's still screen: the OpenAgents chat.
pub const MACBOOK_SCREEN: &[u8] =
    include_bytes!("../../../assets/verse/generated/help_desk/macbook-screen.png");
/// The phone's still screen: the OpenAgents app's home.
pub const IPHONE_SCREEN: &[u8] =
    include_bytes!("../../../assets/verse/generated/help_desk/iphone-screen.png");

const MAGIC: &[u8; 4] = b"OAHD";
const VERSION: u16 = 1;

/// The most triangles a device's body may have. The Blender script holds
/// the MacBook to 3,000 and the iPhone to 800.
pub const MAX_TRIANGLES: usize = 3_200;
/// The most bytes the compiled file may have.
pub const MAX_BYTES: usize = 96 * 1024;
/// The most bytes each still screen may have.
pub const MAX_SCREEN_BYTES: usize = 48 * 1024;

/// The tone of a quiet edge: the keys, the speaker grilles.
pub const QUIET: u8 = 1;
/// The tone of an outline.
pub const PRESENT: u8 = 2;

/// One device as line art, in meters, glTF's +Y up, its base centered on
/// the origin and its front toward +Z.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub name: String,
    pub corners: Vec<[f32; 3]>,
    pub triangles: Vec<[u16; 3]>,
    pub edges: Vec<([u16; 2], u8)>,
    /// The display's corners: position and texture coordinates.
    pub screen: Vec<([f32; 3], [f32; 2])>,
    pub screen_indices: Vec<u16>,
}

impl Device {
    /// The device's height, m.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.corners
            .iter()
            .chain(self.screen.iter().map(|(p, _)| p))
            .map(|p| p[1])
            .fold(0.0, f32::max)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.bytes.len() < n {
            return Err("The help desk's devices end early".into());
        }
        let (head, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<usize, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    }
    fn f32(&mut self) -> Result<f32, String> {
        let b = self.take(4)?;
        let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if v.is_finite() {
            Ok(v)
        } else {
            Err("The help desk's devices hold a number that isn't finite".into())
        }
    }
    /// A count no larger than `most`.
    fn count(&mut self, most: usize) -> Result<usize, String> {
        let n = self.u32()?;
        if n > most {
            return Err("The help desk's devices are over their budget".into());
        }
        Ok(n)
    }
}

/// Reads compiled devices.
///
/// # Errors
///
/// The bytes are not a whole, in-budget file of this format, or an index
/// points past its corners.
pub fn decode(bytes: &[u8]) -> Result<Vec<Device>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("The help desk's devices are over their budget".into());
    }
    let mut r = Reader { bytes };
    if r.take(4)? != MAGIC || r.u16()? != VERSION {
        return Err("The help desk's devices are in an unknown format".into());
    }
    let count = usize::from(r.u16()?);
    if count > 4 {
        return Err("The help desk's devices are over their budget".into());
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let len = usize::from(r.u8()?);
        let name = String::from_utf8(r.take(len)?.to_vec())
            .map_err(|_| "A help desk device's name isn't text")?;
        let lo = [r.f32()?, r.f32()?, r.f32()?];
        let hi = [r.f32()?, r.f32()?, r.f32()?];
        let n = r.count(usize::from(u16::MAX))?;
        let mut corners = Vec::with_capacity(n);
        for _ in 0..n {
            let mut p = [0.0; 3];
            for (axis, value) in p.iter_mut().enumerate() {
                let q = f32::from(r.u16()?) / f32::from(u16::MAX);
                *value = lo[axis] + q * (hi[axis] - lo[axis]);
            }
            corners.push(p);
        }
        let in_range = |i: u16, n: usize| -> Result<u16, String> {
            if usize::from(i) < n {
                Ok(i)
            } else {
                Err("A help desk device points past its corners".into())
            }
        };
        let t = r.count(MAX_TRIANGLES)?;
        let mut triangles = Vec::with_capacity(t);
        for _ in 0..t {
            triangles.push([
                in_range(r.u16()?, n)?,
                in_range(r.u16()?, n)?,
                in_range(r.u16()?, n)?,
            ]);
        }
        let e = r.count(MAX_TRIANGLES * 3)?;
        let mut edges = Vec::with_capacity(e);
        for _ in 0..e {
            let pair = [in_range(r.u16()?, n)?, in_range(r.u16()?, n)?];
            let tone = r.u8()?.min(3);
            edges.push((pair, tone));
        }
        let s = r.count(1024)?;
        let mut screen = Vec::with_capacity(s);
        for _ in 0..s {
            screen.push(([r.f32()?, r.f32()?, r.f32()?], [r.f32()?, r.f32()?]));
        }
        let si = r.count(3 * 1024)?;
        let mut screen_indices = Vec::with_capacity(si);
        for _ in 0..si {
            screen_indices.push(in_range(r.u16()?, s)?);
        }
        out.push(Device {
            name,
            corners,
            triangles,
            edges,
            screen,
            screen_indices,
        });
    }
    if !r.bytes.is_empty() {
        return Err("The help desk's devices have bytes past their end".into());
    }
    Ok(out)
}

/// Writes devices in the pinned format.
#[cfg(test)]
pub(crate) fn encode(devices: &[Device]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(devices.len() as u16).to_le_bytes());
    for device in devices {
        out.push(device.name.len() as u8);
        out.extend_from_slice(device.name.as_bytes());
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &device.corners {
            for axis in 0..3 {
                lo[axis] = lo[axis].min(p[axis]);
                hi[axis] = hi[axis].max(p[axis]);
            }
        }
        for v in lo.iter().chain(&hi) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(device.corners.len() as u32).to_le_bytes());
        for p in &device.corners {
            for axis in 0..3 {
                let span = (hi[axis] - lo[axis]).max(f32::MIN_POSITIVE);
                let q = ((p[axis] - lo[axis]) / span * f32::from(u16::MAX)).round();
                out.extend_from_slice(&(q as u16).to_le_bytes());
            }
        }
        out.extend_from_slice(&(device.triangles.len() as u32).to_le_bytes());
        for t in &device.triangles {
            for i in t {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        out.extend_from_slice(&(device.edges.len() as u32).to_le_bytes());
        for ([a, b], tone) in &device.edges {
            out.extend_from_slice(&a.to_le_bytes());
            out.extend_from_slice(&b.to_le_bytes());
            out.push(*tone);
        }
        out.extend_from_slice(&(device.screen.len() as u32).to_le_bytes());
        for (p, uv) in &device.screen {
            for v in p.iter().chain(uv) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&(device.screen_indices.len() as u32).to_le_bytes());
        for i in &device.screen_indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    out
}

/// The pinned devices.
///
/// # Errors
///
/// As [`decode`].
pub fn pinned() -> Result<Vec<Device>, String> {
    decode(PINNED)
}
