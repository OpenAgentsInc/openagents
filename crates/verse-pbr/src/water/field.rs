//! The water's baked field and its streaming (`docs/verse/water.md`,
//! streaming across 8 m cells, phase W10): depth under the surface at
//! rest, distance to the shore, and the current, over a zone's water.
//!
//! A [`Field`] is the zone's record: small, resident on the CPU for the
//! whole zone, and baked from the zone's own height function, so every
//! client builds the same one without a download. Its GPU texture streams.
//! The field is cut into pages of [`PAGE`] texels a side, each a whole
//! number of the renderer's 8 m cells, and each page is an image chunk
//! under `verse_engine::streaming`'s [`Residency`]: the chunks within a
//! window around the eye are requested nearest first, baked from the
//! record by a source job, uploaded under the per-frame byte budget, and
//! evicted least recently used, all within the tier's [`budget`], which
//! counts toward the zone's resident bytes. A page is a chunk rather than
//! each 8 m cell because the scheduler bounds a view to 64 roots.
//!
//! On the GPU the pages share one atlas, [`Atlas`], addressed toroidally
//! as Losasso and Hoppe address a clipmap's level textures: page `(x, z)`
//! lives in slot `(x mod n, z mod n)` of a window `n` pages a side, so the
//! atlas sampled with repeat addressing at the world position finds the
//! page, and bilinear filtering crosses page edges without seams. The
//! uniform's slot table ([`Stream::rows`]) names the page each slot holds;
//! where the table names another page, or none yet, the shader uses the
//! field's [`Field::outside`] values. [`Stream::sample`] is that lookup on
//! the CPU.

use std::collections::BTreeMap;
use std::sync::Arc;

use glam::Vec2;
use verse_engine::quality::Tier;
use verse_engine::streaming::{
    Budget, Decoded, Descriptor, Manifest, Metrics, Residency, cook_image,
};

use super::frame::OPEN_WATER;

/// Texels along a page's side.
pub const PAGE: u32 = 64;
/// Bytes a texel: four half floats.
pub const TEXEL_BYTES: usize = 8;
/// Bytes a row of a page.
pub const ROW_BYTES: usize = PAGE as usize * TEXEL_BYTES;
/// Bytes a page's chunk carries: its rows and one more row naming the page,
/// so pages that look alike stay distinct chunks.
pub const CHUNK_BYTES: usize = (PAGE as usize + 1) * ROW_BYTES;
/// The most pages along a window's side: 64 slots, the scheduler's most
/// roots in one view.
pub const MAX_WINDOW: u32 = 8;
/// The uniform rows of the field's shape ([`Stream::rows`]).
pub const ROWS: usize = 3;
/// The uniform rows of the slot table, four slots a row.
pub const SLOT_ROWS: usize = (MAX_WINDOW * MAX_WINDOW) as usize / 4;
/// The format of the GPU atlas.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The most pages along a field's side.
pub const MAX_PAGES: u32 = 64;

/// One texel of the field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Texel {
    /// Water depth under the surface at rest, m; negative over dry ground.
    pub depth: f32,
    /// Distance to the shore, m; zero over dry ground.
    pub shore: f32,
    /// The current, x and z, m/s.
    pub flow: [f32; 2],
}

impl Texel {
    /// Deep open water `depth` m deep with no current.
    #[must_use]
    pub fn open(depth: f32) -> Self {
        Self {
            depth,
            shore: OPEN_WATER,
            flow: [0.0; 2],
        }
    }

    fn half(self) -> [u16; 4] {
        [self.depth, self.shore, self.flow[0], self.flow[1]].map(super::ocean::half)
    }

    fn of(h: [u16; 4]) -> Self {
        let f = h.map(|v| half::f16::from_bits(v).to_f32());
        Self {
            depth: f[0],
            shore: f[1],
            flow: [f[2], f[3]],
        }
    }

    fn lerp(self, other: Self, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self {
            depth: l(self.depth, other.depth),
            shore: l(self.shore, other.shore),
            flow: [
                l(self.flow[0], other.flow[0]),
                l(self.flow[1], other.flow[1]),
            ],
        }
    }
}

/// A zone's water field: whole pages of texels from `origin`, stored as
/// half floats, with the digests of their chunks.
#[derive(Clone, Debug)]
pub struct Field {
    /// The corner of texel (0, 0), x and z, m.
    pub origin: [f32; 2],
    /// A texel's side, m.
    pub texel: f32,
    /// Pages along x and z.
    pub pages: [u32; 2],
    /// What the shader uses away from the field and where a page is not
    /// resident: open water.
    pub outside: Texel,
    texels: Vec<[u16; 4]>,
    manifest: Manifest,
    /// Each page's chunk id, row by row.
    ids: Vec<String>,
}

impl Field {
    /// Bakes `pages` (x, z) pages of `texel` m texels from `origin`: each
    /// texel's depth and current from `water` at its center, and its
    /// distance to the shore from the texels that are dry, by the exact
    /// Euclidean distance transform ([`super::bake::shore_distance`]).
    ///
    /// # Errors
    /// Refuses an empty or oversized field, a texel that is not positive,
    /// or a value that is not finite.
    pub fn bake(
        origin: [f32; 2],
        texel: f32,
        pages: [u32; 2],
        outside: Texel,
        water: impl Fn(f32, f32) -> (f32, [f32; 2]),
    ) -> Result<Self, String> {
        if !(texel.is_finite() && texel > 0.0)
            || !origin.iter().all(|v| v.is_finite())
            || !(1..=MAX_PAGES).contains(&pages[0])
            || !(1..=MAX_PAGES).contains(&pages[1])
        {
            return Err("A water field has 1 to 64 pages a side of a positive texel".into());
        }
        let cols = (pages[0] * PAGE) as usize;
        let rows = (pages[1] * PAGE) as usize;
        let mut depth = Vec::with_capacity(cols * rows);
        let mut flow = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let x = origin[0] + (c as f32 + 0.5) * texel;
                let z = origin[1] + (r as f32 + 0.5) * texel;
                let (d, f) = water(x, z);
                if !d.is_finite() || !f.iter().all(|v| v.is_finite()) {
                    return Err(format!("The water field is not finite at ({x}, {z})"));
                }
                depth.push(d);
                flow.push(f);
            }
        }
        let wet: Vec<bool> = depth.iter().map(|&d| d > 0.0).collect();
        let shore = super::bake::shore_distance(cols, rows, &wet, texel);
        let texels = depth
            .iter()
            .zip(&flow)
            .zip(&shore)
            .map(|((&depth, &flow), &shore)| {
                Texel {
                    depth,
                    shore: shore.min(OPEN_WATER),
                    flow,
                }
                .half()
            })
            .collect();
        let mut field = Self {
            origin,
            texel,
            pages,
            outside,
            texels,
            manifest: Manifest {
                version: 1,
                chunks: BTreeMap::new(),
            },
            ids: Vec::new(),
        };
        for pz in 0..pages[1] {
            for px in 0..pages[0] {
                let (descriptor, _) = field.cook([px, pz])?;
                field.ids.push(descriptor.sha256.clone());
                field
                    .manifest
                    .chunks
                    .insert(descriptor.sha256.clone(), descriptor);
            }
        }
        field.manifest.validate()?;
        Ok(field)
    }

    /// Texels along x and z.
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        [self.pages[0] * PAGE, self.pages[1] * PAGE]
    }

    /// A page's side, m.
    #[must_use]
    pub fn page_meters(&self) -> f32 {
        self.texel * PAGE as f32
    }

    /// The pages' chunks.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The chunk id of page `page` (x, z).
    #[must_use]
    pub fn id(&self, page: [u32; 2]) -> Option<&str> {
        (page[0] < self.pages[0] && page[1] < self.pages[1])
            .then(|| self.ids[(page[1] * self.pages[0] + page[0]) as usize].as_str())
    }

    /// The bytes the record holds on the CPU.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.texels.len() * TEXEL_BYTES + self.ids.len() * 64
    }

    /// The texel at column `c` and row `r`, or [`Self::outside`] beyond the
    /// field.
    #[must_use]
    pub fn texel_at(&self, c: i64, r: i64) -> Texel {
        let [cols, rows] = self.size().map(i64::from);
        if (0..cols).contains(&c) && (0..rows).contains(&r) {
            Texel::of(self.texels[(r * cols + c) as usize])
        } else {
            self.outside
        }
    }

    /// The field at (x, z), filtered bilinearly between texel centers as
    /// the shader samples it, m.
    #[must_use]
    pub fn sample(&self, x: f32, z: f32) -> Texel {
        bilinear(self, x, z, |c, r| self.texel_at(c, r))
    }

    /// Page `page`'s chunk: its texels as half floats, row by row, and a
    /// last row naming the page, cooked as an image chunk twice as wide as
    /// the page (two RGBA8 texels hold one RGBA16F texel).
    ///
    /// # Errors
    /// Refuses a page outside the field.
    pub fn cook(&self, page: [u32; 2]) -> Result<(Descriptor, Vec<u8>), String> {
        if page[0] >= self.pages[0] || page[1] >= self.pages[1] {
            return Err("No such water field page".into());
        }
        let cols = self.size()[0] as usize;
        let mut bytes = Vec::with_capacity(CHUNK_BYTES);
        for r in 0..PAGE as usize {
            let row = page[1] as usize * PAGE as usize + r;
            let start = row * cols + page[0] as usize * PAGE as usize;
            for t in &self.texels[start..start + PAGE as usize] {
                for v in t {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        let mut name = vec![0u8; ROW_BYTES];
        name[..4].copy_from_slice(&page[0].to_le_bytes());
        name[4..8].copy_from_slice(&page[1].to_le_bytes());
        bytes.extend_from_slice(&name);
        cook_image(2 * PAGE, PAGE + 1, &bytes)
    }
}

impl PartialEq for Field {
    /// Fields are equal when their texels and pages are; the chunk ids
    /// stand for the manifest.
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.texel == other.texel
            && self.pages == other.pages
            && self.outside == other.outside
            && self.texels == other.texels
            && self.ids == other.ids
    }
}

fn bilinear(field: &Field, x: f32, z: f32, at: impl Fn(i64, i64) -> Texel) -> Texel {
    let u = (x - field.origin[0]) / field.texel - 0.5;
    let v = (z - field.origin[1]) / field.texel - 0.5;
    let (c, r) = (u.floor(), v.floor());
    let (fu, fv) = (u - c, v - r);
    let (c, r) = (c as i64, r as i64);
    let top = at(c, r).lerp(at(c + 1, r), fu);
    let bottom = at(c, r + 1).lerp(at(c + 1, r + 1), fu);
    top.lerp(bottom, fv)
}

/// Pages along a tier's window side: 5 on Low, 7 on Medium, 8 on High.
#[must_use]
pub fn window(tier: Tier) -> u32 {
    match tier {
        Tier::Low => 5,
        Tier::Medium => 7,
        Tier::High => MAX_WINDOW,
    }
}

/// A tier's residency budget for the field's pages: the window's pages on
/// the GPU and on the CPU, two pages baked and two uploaded a frame. On
/// Low this is 0.8 MB of each, on High 2.1 MB, against the water's
/// 8 and 64 MiB budgets (`docs/verse/water.md`, budgets per tier).
#[must_use]
pub fn budget(tier: Tier) -> Budget {
    let pages = u64::from(window(tier).pow(2));
    Budget {
        cpu_bytes: pages * (CHUNK_BYTES as u64 + 32),
        gpu_bytes: pages * CHUNK_BYTES as u64,
        source_jobs: 2,
        upload_bytes_per_frame: 2 * CHUNK_BYTES,
        upload_ms_per_frame: 1.0,
    }
}

/// What one [`Stream::update`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Frame {
    pub baked: usize,
    pub uploaded_bytes: usize,
    /// Pages of the window with their texels in place.
    pub resident: usize,
    /// Pages of the field inside the window.
    pub wanted: usize,
}

/// A field's pages streaming into a tier's atlas around the eye.
pub struct Stream {
    field: Arc<Field>,
    residency: Residency,
    window: u32,
    /// The page whose texels each slot holds, once wholly uploaded.
    slots: Vec<Option<[u32; 2]>>,
    pages: BTreeMap<String, [u32; 2]>,
    pub last: Frame,
}

impl Stream {
    /// `field`'s pages under `tier`'s window and budget.
    ///
    /// # Errors
    /// Refuses a field whose manifest the scheduler does not admit.
    pub fn new(field: Arc<Field>, tier: Tier) -> Result<Self, String> {
        let window = window(tier);
        let residency = Residency::new(field.manifest().clone(), budget(tier))?;
        let mut pages = BTreeMap::new();
        for pz in 0..field.pages[1] {
            for px in 0..field.pages[0] {
                if let Some(id) = field.id([px, pz]) {
                    pages.insert(id.to_owned(), [px, pz]);
                }
            }
        }
        Ok(Self {
            field,
            residency,
            window,
            slots: vec![None; (window * window) as usize],
            pages,
            last: Frame::default(),
        })
    }

    #[must_use]
    pub fn field(&self) -> &Field {
        &self.field
    }

    #[must_use]
    pub fn metrics(&self) -> Metrics {
        self.residency.metrics()
    }

    #[must_use]
    pub fn budget(&self) -> Budget {
        self.residency.budget()
    }

    /// The pages of the field within the window around `eye`, nearest
    /// first.
    #[must_use]
    pub fn wanted(&self, eye: Vec2) -> Vec<[u32; 2]> {
        let f = &self.field;
        let page = f.page_meters();
        let n = self.window as f32;
        let at = (eye - Vec2::from(f.origin)) / page;
        let start = (at - Vec2::splat(n * 0.5 - 0.5)).floor();
        let mut out = Vec::new();
        for dz in 0..self.window {
            for dx in 0..self.window {
                let p = start + Vec2::new(dx as f32, dz as f32);
                if p.x >= 0.0 && p.y >= 0.0 && p.x < f.pages[0] as f32 && p.y < f.pages[1] as f32 {
                    out.push([p.x as u32, p.y as u32]);
                }
            }
        }
        let center = |p: &[u32; 2]| Vec2::new(p[0] as f32 + 0.5, p[1] as f32 + 0.5);
        out.sort_by(|a, b| {
            center(a)
                .distance_squared(at)
                .total_cmp(&center(b).distance_squared(at))
                .then(a.cmp(b))
        });
        out
    }

    fn slot(&self, page: [u32; 2]) -> usize {
        slot_of(self.window, page)
    }

    /// Requests the window around `eye`, bakes up to the budget's source
    /// jobs of pages from the record, and uploads up to its bytes a frame
    /// through `write(slot, first_row, rows_bytes)`, whose rows are whole
    /// rows of [`ROW_BYTES`] for the slot's page.
    ///
    /// # Errors
    /// Passes on what the scheduler refuses.
    pub fn update(
        &mut self,
        eye: Vec2,
        write: &mut dyn FnMut(usize, u32, &[u8]),
    ) -> Result<Frame, String> {
        let wanted = self.wanted(eye);
        let roots: Vec<String> = wanted
            .iter()
            .filter_map(|p| self.field.id(*p).map(str::to_owned))
            .collect();
        self.residency.request(&roots)?;
        let mut frame = Frame {
            wanted: wanted.len(),
            ..Frame::default()
        };
        while let Some(ticket) = self.residency.next_source()? {
            let page = self.pages[ticket.id()];
            let descriptor = self.field.manifest().chunks[ticket.id()].clone();
            let result = self
                .field
                .cook(page)
                .and_then(|(_, bytes)| Decoded::decode(bytes, &descriptor));
            self.residency.source_result(ticket, result)?;
            frame.baked += 1;
        }
        let mut remaining = self.budget().upload_bytes_per_frame;
        while remaining >= ROW_BYTES {
            let allowed = remaining / ROW_BYTES * ROW_BYTES;
            let window = self.window;
            let Some(upload) = self.residency.next_upload(allowed) else {
                break;
            };
            let page = self.pages[upload.ticket.id()];
            let slot = slot_of(window, page);
            let (offset, length, ticket) =
                (upload.offset, upload.bytes.len(), upload.ticket.clone());
            // The page's own rows; the naming row stays on the CPU.
            let first = offset / ROW_BYTES;
            let end = (offset + length).min(PAGE as usize * ROW_BYTES);
            if upload.allocate || offset == 0 {
                self.slots[slot] = None;
            }
            if end > offset {
                write(slot, first as u32, &upload.bytes[..end - offset]);
            }
            self.residency.uploaded(&ticket, offset, length)?;
            if self.residency.committed(ticket.id()) {
                self.slots[slot] = Some(page);
            }
            frame.uploaded_bytes += length;
            remaining -= length;
        }
        frame.resident = wanted
            .iter()
            .filter(|p| self.slots[self.slot(**p)] == Some(**p))
            .count();
        self.last = frame;
        Ok(frame)
    }

    /// The uniform rows the shader reads: the field's origin, texel, and
    /// window; its pages along x and z, a page's side, and 1; and its
    /// outside values; then the slot table, each slot's page as
    /// `x × 4096 + z`, or −1 when it holds none.
    #[must_use]
    pub fn rows(&self) -> ([[f32; 4]; ROWS], [[f32; 4]; SLOT_ROWS]) {
        let f = &self.field;
        let o = f.outside;
        let shape = [
            [f.origin[0], f.origin[1], f.texel, self.window as f32],
            [f.pages[0] as f32, f.pages[1] as f32, f.page_meters(), 1.0],
            [o.depth, o.shore, o.flow[0], o.flow[1]],
        ];
        let mut slots = [[-1.0; 4]; SLOT_ROWS];
        for (i, page) in self.slots.iter().enumerate() {
            if let Some(p) = page {
                slots[i / 4][i % 4] = key(*p);
            }
        }
        (shape, slots)
    }

    /// The field at (x, z) as the shader finds it through the atlas: the
    /// field's value where the page is in place, else outside.
    #[must_use]
    pub fn sample(&self, x: f32, z: f32) -> Texel {
        let f = &self.field;
        let page = ((Vec2::new(x, z) - Vec2::from(f.origin)) / f.page_meters()).floor();
        if page.x < 0.0
            || page.y < 0.0
            || page.x >= f.pages[0] as f32
            || page.y >= f.pages[1] as f32
        {
            return f.outside;
        }
        let page = [page.x as u32, page.y as u32];
        if self.slots[self.slot(page)] != Some(page) {
            return f.outside;
        }
        // Inside a resident page the atlas holds the field's texels; a
        // neighbor page that is not in place blends in what its slot holds,
        // which this mirror leaves out.
        f.sample(x, z)
    }
}

fn slot_of(window: u32, page: [u32; 2]) -> usize {
    ((page[1] % window) * window + page[0] % window) as usize
}

/// A slot table's key for page `p`.
#[must_use]
pub fn key(p: [u32; 2]) -> f32 {
    (p[0] * 4096 + p[1]) as f32
}

/// The field's atlas on the GPU: a tier's window of pages.
pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    window: u32,
}

impl Atlas {
    /// An atlas for `tier`'s window, cleared to zero.
    #[must_use]
    pub fn new(device: &wgpu::Device, tier: Tier) -> Self {
        let window = window(tier);
        let side = window * PAGE;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verse water field"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse water field"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
            window,
        }
    }

    /// The bytes the atlas holds.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        u64::from(self.window * PAGE).pow(2) * TEXEL_BYTES as u64
    }

    /// Writes whole rows of a page into `slot` from `first_row`.
    pub fn write(&self, queue: &wgpu::Queue, slot: usize, first_row: u32, rows: &[u8]) {
        let slot = slot as u32;
        let count = (rows.len() / ROW_BYTES) as u32;
        if count == 0 || slot >= self.window * self.window {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: (slot % self.window) * PAGE,
                    y: (slot / self.window) * PAGE + first_row,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &rows[..count as usize * ROW_BYTES],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW_BYTES as u32),
                rows_per_image: Some(count),
            },
            wgpu::Extent3d {
                width: PAGE,
                height: count,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// The atlas's and its sampler's layout entries at `binding` and the next,
/// for the vertex stage only, which places the clipmap.
#[must_use]
pub fn entries(binding: u32) -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: binding + 1,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A beach: land to the north (−z), deepening southward at 1 in 20,
    /// with a current along it.
    fn beach() -> Field {
        Field::bake([-256.0, -256.0], 2.0, [4, 4], Texel::open(30.0), |_, z| {
            ((z + 40.0) / 20.0, [0.3, 0.0])
        })
        .unwrap()
    }

    /// The record holds the depth, current, and shore distance it was
    /// baked from, and a page's chunk decodes under its own digest.
    #[test]
    fn the_field_bakes_depth_shore_and_flow() {
        let f = beach();
        let t = f.sample(10.0, 60.0);
        assert!((t.depth - 5.0).abs() < 0.01, "{t:?}");
        assert!((t.shore - 100.0).abs() < 2.5, "{t:?}");
        assert!((t.flow[0] - 0.3).abs() < 1e-3);
        assert_eq!(f.sample(0.0, -100.0).shore, 0.0);
        assert_eq!(f.sample(0.0, 900.0), f.outside);
        assert_eq!(f.manifest().chunks.len(), 16);
        let id = f.id([1, 2]).unwrap();
        let (_, bytes) = f.cook([1, 2]).unwrap();
        let decoded = Decoded::decode(bytes, &f.manifest().chunks[id]).unwrap();
        assert_eq!(decoded.payload().len(), CHUNK_BYTES);
    }

    /// Walking across a field larger than the window, the pages stream in
    /// nearest first, the shader's lookup finds the field wherever its page
    /// is in place, and the resident bytes never pass the tier's budget.
    #[test]
    fn pages_stream_within_the_budget() {
        let field = Arc::new(
            Field::bake(
                [-1024.0, -1024.0],
                2.0,
                [16, 16],
                Texel::open(30.0),
                |x, z| ((x * 0.01).sin() * 10.0 + z * 0.02, [0.0, 0.1]),
            )
            .unwrap(),
        );
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let mut stream = Stream::new(field.clone(), tier).unwrap();
            let budget = stream.budget();
            let mut written = vec![0usize; (window(tier) * window(tier)) as usize];
            let mut eye = Vec2::new(-900.0, -900.0);
            let mut settled = 0;
            for step in 0..400 {
                let frame = stream
                    .update(eye, &mut |slot, row, bytes| {
                        assert_eq!(bytes.len() % ROW_BYTES, 0);
                        assert!(
                            row as usize * ROW_BYTES + bytes.len() <= PAGE as usize * ROW_BYTES
                        );
                        written[slot] += bytes.len();
                    })
                    .unwrap();
                let m = stream.metrics();
                assert!(m.gpu_bytes <= budget.gpu_bytes && m.cpu_bytes <= budget.cpu_bytes);
                assert!(frame.uploaded_bytes <= budget.upload_bytes_per_frame);
                if frame.resident == frame.wanted {
                    settled += 1;
                    let (shape, slots) = stream.rows();
                    assert_eq!(shape[0][3], window(tier) as f32);
                    assert!(slots.iter().flatten().any(|k| *k >= 0.0));
                    let t = stream.sample(eye.x + 3.0, eye.y - 2.0);
                    assert_eq!(t, field.sample(eye.x + 3.0, eye.y - 2.0));
                }
                // Walk at a run, 4 m a frame, diagonally across the field.
                if step % 2 == 0 {
                    eye += Vec2::new(4.0, 3.5) * 2.0;
                }
            }
            let m = stream.metrics();
            assert!(m.gpu_high_water <= budget.gpu_bytes);
            assert!(m.evictions > 0, "{tier:?}: the walk leaves pages behind");
            assert!(settled > 100, "{tier:?}: {settled}");
            assert!(written.iter().all(|w| *w > 0));
        }
    }
}
