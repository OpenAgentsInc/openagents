//! Snapshot encode/decode traits and message types for replication.
//!
//! This module intentionally stays simple for v0; later phases can swap in
//! better encoders or deltas without breaking clients of these traits.

/// Types implementing snapshot encoding write themselves into a byte buffer.
pub trait SnapshotEncode {
    fn encode(&self, out: &mut Vec<u8>);
}

/// Types implementing snapshot decoding reconstruct themselves from a byte slice.
pub trait SnapshotDecode: Sized {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self>;
}

/// Minimal entity header stub for per-entity records in a snapshot stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityHeader {
    pub id: u64,
    pub archetype: u16,
}

/// A compact CPU mesh delta for a single voxel chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkMeshDelta {
    pub did: u64,
    pub chunk: (u32, u32, u32),
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

const VERSION: u8 = 1;
const ACTOR_SNAP_VERSION: u8 = 2;
const ACTOR_SNAP_DELTA_VERSION: u8 = 4;
pub const TAG_ACTOR_SNAPSHOT: u8 = 0xA2;
pub const TAG_ACTOR_SNAPSHOT_DELTA: u8 = 0xA3;
// Legacy TickSnapshot tag removed; ActorSnapshot v2 is canonical.
const MAX_MESH_ELEMS: usize = 262_144; // conservative cap to prevent OOM

impl SnapshotEncode for ChunkMeshDelta {
    fn encode(&self, out: &mut Vec<u8>) {
        // versioned encoding (scaffold)
        out.push(VERSION);
        out.extend_from_slice(&self.did.to_le_bytes());
        out.extend_from_slice(&self.chunk.0.to_le_bytes());
        out.extend_from_slice(&self.chunk.1.to_le_bytes());
        out.extend_from_slice(&self.chunk.2.to_le_bytes());
        let npos = u32::try_from(self.positions.len()).expect("positions len fits u32");
        out.extend_from_slice(&npos.to_le_bytes());
        for p in &self.positions {
            for c in p {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        let nnrm = u32::try_from(self.normals.len()).expect("normals len fits u32");
        out.extend_from_slice(&nnrm.to_le_bytes());
        for n in &self.normals {
            for c in n {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        let nidx = u32::try_from(self.indices.len()).expect("indices len fits u32");
        out.extend_from_slice(&nidx.to_le_bytes());
        for i in &self.indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
}

impl SnapshotDecode for ChunkMeshDelta {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            bail!("unsupported version: {ver}");
        }
        let did = u64::from_le_bytes(take::<8>(inp)?);
        let cx = u32::from_le_bytes(take::<4>(inp)?);
        let cy = u32::from_le_bytes(take::<4>(inp)?);
        let cz = u32::from_le_bytes(take::<4>(inp)?);
        let npos = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if npos > MAX_MESH_ELEMS {
            bail!("npos too large: {npos}");
        }
        let mut positions = Vec::with_capacity(npos);
        for _ in 0..npos {
            positions.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nnrm = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if nnrm > MAX_MESH_ELEMS {
            bail!("nnrm too large: {nnrm}");
        }
        let mut normals = Vec::with_capacity(nnrm);
        for _ in 0..nnrm {
            normals.push([
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
                f32::from_le_bytes(take::<4>(inp)?),
            ]);
        }
        let nidx = u32::from_le_bytes(take::<4>(inp)?) as usize;
        if nidx > MAX_MESH_ELEMS * 6 {
            bail!("nidx too large: {nidx}");
        }
        let mut indices = Vec::with_capacity(nidx);
        for _ in 0..nidx {
            indices.push(u32::from_le_bytes(take::<4>(inp)?));
        }
        if positions.len() != normals.len() {
            bail!(
                "positions/normals length mismatch: {}/{}",
                positions.len(),
                normals.len()
            );
        }
        Ok(Self {
            did,
            chunk: (cx, cy, cz),
            positions,
            normals,
            indices,
        })
    }
}

// Legacy TickSnapshot removed; use ActorSnapshot v2.

// Actor-centric snapshot (v2)
#[derive(Debug, Clone, PartialEq)]
pub struct ActorRep {
    pub id: u32,
    pub kind: u8,    // presentation-only bucket; clients should prefer archetype_id
    pub faction: u8, // formerly `team`
    pub archetype_id: u16, // data id for model/UI mapping
    pub name_id: u16, // data id for display name
    pub unique: u8,  // 0/1 unique flag for bosses/etc.
    pub pos: [f32; 3],
    pub yaw: f32,
    pub radius: f32,
    pub hp: i32,
    pub max: i32,
    pub alive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorSnapshot {
    pub v: u8, // must be 2
    pub tick: u64,
    pub actors: Vec<ActorRep>,
    pub projectiles: Vec<ProjectileRep>,
}

impl SnapshotEncode for ActorSnapshot {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_ACTOR_SNAPSHOT);
        out.push(self.v);
        out.extend_from_slice(&self.tick.to_le_bytes());
        let na = u32::try_from(self.actors.len()).unwrap_or(0);
        out.extend_from_slice(&na.to_le_bytes());
        for a in &self.actors {
            out.extend_from_slice(&a.id.to_le_bytes());
            out.push(a.kind);
            // ActorSnapshot v2 continues to write the compact faction value in the legacy `team` slot
            out.push(a.faction);
            // ActorSnapshot v2 does not carry archetype/name/unique; omitted here intentionally
            for c in &a.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
            out.extend_from_slice(&a.yaw.to_le_bytes());
            out.extend_from_slice(&a.radius.to_le_bytes());
            out.extend_from_slice(&a.hp.to_le_bytes());
            out.extend_from_slice(&a.max.to_le_bytes());
            out.push(u8::from(a.alive));
        }
        let np = u32::try_from(self.projectiles.len()).unwrap_or(0);
        out.extend_from_slice(&np.to_le_bytes());
        for p in &self.projectiles {
            out.extend_from_slice(&p.id.to_le_bytes());
            out.push(p.kind);
            for c in &p.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
            for c in &p.vel {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
}

impl SnapshotDecode for ActorSnapshot {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let tag = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if tag != TAG_ACTOR_SNAPSHOT {
            bail!("not an ActorSnapshot tag");
        }
        let v = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if v != ACTOR_SNAP_VERSION {
            bail!("unsupported version: {v}");
        }
        let tick = u64::from_le_bytes(take::<8>(inp)?);
        let na = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut actors = Vec::with_capacity(na);
        for _ in 0..na {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            let kind = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let team = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            let yaw = f32::from_le_bytes(take::<4>(inp)?);
            let radius = f32::from_le_bytes(take::<4>(inp)?);
            let hp = i32::from_le_bytes(take::<4>(inp)?);
            let max = i32::from_le_bytes(take::<4>(inp)?);
            let alive = match inp.first().copied() {
                Some(0) => false,
                Some(_) => true,
                None => anyhow::bail!("short read"),
            };
            *inp = &inp[1..];
            actors.push(ActorRep {
                id,
                kind,
                faction: team,
                archetype_id: 0,
                name_id: 0,
                unique: 0,
                pos,
                yaw,
                radius,
                hp,
                max,
                alive,
            });
        }
        let np = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut projectiles = Vec::with_capacity(np);
        for _ in 0..np {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            let kind = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            let mut vel = [0.0f32; 3];
            for v in &mut vel {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            projectiles.push(ProjectileRep { id, kind, pos, vel });
        }
        Ok(Self {
            v,
            tick,
            actors,
            projectiles,
        })
    }
}

// Actor-centric delta snapshot (v3)
// Fields: spawns (full reps), updates (bitmasked), removals (ids), projectiles (full).
#[derive(Debug, Clone, PartialEq)]
pub struct ActorDeltaRec {
    pub id: u32,
    pub flags: u8, // 1=pos,2=yaw,4=hp,8=alive
    pub qpos: [i32; 3],
    pub qyaw: u16,
    pub hp: i32,
    pub alive: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorSnapshotDelta {
    pub v: u8, // must be 3
    pub tick: u64,
    pub baseline: u64,
    pub spawns: Vec<ActorRep>,
    pub updates: Vec<ActorDeltaRec>,
    pub removals: Vec<u32>,
    pub projectiles: Vec<ProjectileRep>,
    pub hits: Vec<HitFx>,
}

/// Lightweight hit effect for client VFX without client-side gameplay.
#[derive(Debug, Clone, PartialEq)]
pub struct HitFx {
    pub kind: u8,      // 0=Firebolt, 1=Fireball (optional), 2=MagicMissile, etc.
    pub pos: [f32; 3], // world-space position
}

impl SnapshotEncode for ActorSnapshotDelta {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_ACTOR_SNAPSHOT_DELTA);
        out.push(self.v);
        out.extend_from_slice(&self.tick.to_le_bytes());
        out.extend_from_slice(&self.baseline.to_le_bytes());
        // spawns (v4 layout only)
        let ns = u32::try_from(self.spawns.len()).unwrap_or(0);
        out.extend_from_slice(&ns.to_le_bytes());
        for a in &self.spawns {
            out.extend_from_slice(&a.id.to_le_bytes());
            out.push(a.kind);
            out.push(a.faction);
            out.extend_from_slice(&a.archetype_id.to_le_bytes());
            out.extend_from_slice(&a.name_id.to_le_bytes());
            out.push(a.unique);
            for c in &a.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
            out.extend_from_slice(&a.yaw.to_le_bytes());
            out.extend_from_slice(&a.radius.to_le_bytes());
            out.extend_from_slice(&a.hp.to_le_bytes());
            out.extend_from_slice(&a.max.to_le_bytes());
            out.push(u8::from(a.alive));
        }
        // updates
        let nu = u32::try_from(self.updates.len()).unwrap_or(0);
        out.extend_from_slice(&nu.to_le_bytes());
        for u in &self.updates {
            out.extend_from_slice(&u.id.to_le_bytes());
            out.push(u.flags);
            if u.flags & 1 != 0 {
                for c in &u.qpos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            if u.flags & 2 != 0 {
                out.extend_from_slice(&u.qyaw.to_le_bytes());
            }
            if u.flags & 4 != 0 {
                out.extend_from_slice(&u.hp.to_le_bytes());
            }
            if u.flags & 8 != 0 {
                out.push(u.alive);
            }
        }
        // removals
        let nr = u32::try_from(self.removals.len()).unwrap_or(0);
        out.extend_from_slice(&nr.to_le_bytes());
        for id in &self.removals {
            out.extend_from_slice(&id.to_le_bytes());
        }
        // projectiles (full)
        let np = u32::try_from(self.projectiles.len()).unwrap_or(0);
        out.extend_from_slice(&np.to_le_bytes());
        for p in &self.projectiles {
            out.extend_from_slice(&p.id.to_le_bytes());
            out.push(p.kind);
            for c in &p.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
            for c in &p.vel {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        // hitfx (effects only)
        let nh = u32::try_from(self.hits.len()).unwrap_or(0);
        out.extend_from_slice(&nh.to_le_bytes());
        for h in &self.hits {
            out.push(h.kind);
            for c in &h.pos {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
}

impl SnapshotDecode for ActorSnapshotDelta {
    #[allow(clippy::too_many_lines)]
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let tag = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if tag != TAG_ACTOR_SNAPSHOT_DELTA {
            bail!("not an ActorSnapshotDelta tag");
        }
        let v = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if v != ACTOR_SNAP_DELTA_VERSION {
            bail!("unsupported version: {v}");
        }
        let tick = u64::from_le_bytes(take::<8>(inp)?);
        let baseline = u64::from_le_bytes(take::<8>(inp)?);
        // spawns
        let ns = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut spawns = Vec::with_capacity(ns);
        for _ in 0..ns {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            // v4 spawns: kind, faction, archetype_id, name_id, unique
            let kind: u8 = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let faction: u8 = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let archetype_id: u16 = u16::from_le_bytes(take::<2>(inp)?);
            let name_id: u16 = u16::from_le_bytes(take::<2>(inp)?);
            let unique: u8 = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            let yaw = f32::from_le_bytes(take::<4>(inp)?);
            let radius = f32::from_le_bytes(take::<4>(inp)?);
            let hp = i32::from_le_bytes(take::<4>(inp)?);
            let max = i32::from_le_bytes(take::<4>(inp)?);
            let alive = match inp.first().copied() {
                Some(0) => false,
                Some(_) => true,
                None => anyhow::bail!("short read"),
            };
            *inp = &inp[1..];
            spawns.push(ActorRep {
                id,
                kind,
                faction,
                archetype_id,
                name_id,
                unique,
                pos,
                yaw,
                radius,
                hp,
                max,
                alive,
            });
        }
        // updates
        let nu = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut updates = Vec::with_capacity(nu);
        for _ in 0..nu {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            let flags = {
                let b = inp
                    .first()
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("short read"))?;
                *inp = &inp[1..];
                b
            };
            let mut qpos = [0i32; 3];
            if flags & 1 != 0 {
                for v in &mut qpos {
                    *v = i32::from_le_bytes(take::<4>(inp)?);
                }
            }
            let mut qyaw = 0u16;
            if flags & 2 != 0 {
                qyaw = u16::from_le_bytes(take::<2>(inp)?);
            }
            let mut hp = 0i32;
            if flags & 4 != 0 {
                hp = i32::from_le_bytes(take::<4>(inp)?);
            }
            let mut alive = 0u8;
            if flags & 8 != 0 {
                alive = {
                    let b = inp
                        .first()
                        .copied()
                        .ok_or_else(|| anyhow::anyhow!("short read"))?;
                    *inp = &inp[1..];
                    b
                };
            }
            updates.push(ActorDeltaRec {
                id,
                flags,
                qpos,
                qyaw,
                hp,
                alive,
            });
        }
        // removals
        let nr = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut removals = Vec::with_capacity(nr);
        for _ in 0..nr {
            removals.push(u32::from_le_bytes(take::<4>(inp)?));
        }
        // projectiles
        let np = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut projectiles = Vec::with_capacity(np);
        for _ in 0..np {
            let id = u32::from_le_bytes(take::<4>(inp)?);
            let kind = {
                let b = inp
                    .first()
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("short read"))?;
                *inp = &inp[1..];
                b
            };
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            let mut vel = [0.0f32; 3];
            for v in &mut vel {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            projectiles.push(ProjectileRep { id, kind, pos, vel });
        }
        // hits
        let nh = u32::from_le_bytes(take::<4>(inp)?) as usize;
        let mut hits = Vec::with_capacity(nh);
        for _ in 0..nh {
            let kind = {
                let b = inp
                    .first()
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("short read"))?;
                *inp = &inp[1..];
                b
            };
            let mut pos = [0.0f32; 3];
            for v in &mut pos {
                *v = f32::from_le_bytes(take::<4>(inp)?);
            }
            hits.push(HitFx { kind, pos });
        }
        Ok(Self {
            v,
            tick,
            baseline,
            spawns,
            updates,
            removals,
            projectiles,
            hits,
        })
    }
}

// Quantization helpers for snapshot deltas
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn qpos(x: f32) -> i32 {
    (x * 64.0).round() as i32
}
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn dqpos(x: i32) -> f32 {
    (x as f32) / 64.0
}
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn qyaw(y: f32) -> u16 {
    let two_pi = std::f32::consts::TAU;
    let r = y % two_pi;
    let r = if r < 0.0 { r + two_pi } else { r };
    ((r * (65535.0 / two_pi)).round() as u32 & 0xFFFF) as u16
}
#[must_use]
pub fn dqyaw(y: u16) -> f32 {
    f32::from(y) * (std::f32::consts::TAU / 65535.0)
}

// ---------------------------------------------------------------------------
// HUD status (per-local-player)
// ---------------------------------------------------------------------------

pub const TAG_HUD_STATUS: u8 = 0xB1;
pub const HUD_STATUS_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct HudStatusMsg {
    pub v: u8,
    pub mana: u16,
    pub mana_max: u16,
    pub gcd_ms: u16,
    pub spell_cds: Vec<(u8, u16)>,
    pub burning_ms: u16,
    pub slow_ms: u16,
    pub stunned_ms: u16,
}

impl SnapshotEncode for HudStatusMsg {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_HUD_STATUS);
        out.push(self.v);
        out.extend_from_slice(&self.mana.to_le_bytes());
        out.extend_from_slice(&self.mana_max.to_le_bytes());
        out.extend_from_slice(&self.gcd_ms.to_le_bytes());
        let n = u8::try_from(self.spell_cds.len()).unwrap_or(0);
        out.push(n);
        for (id, ms) in &self.spell_cds {
            out.push(*id);
            out.extend_from_slice(&ms.to_le_bytes());
        }
        out.extend_from_slice(&self.burning_ms.to_le_bytes());
        out.extend_from_slice(&self.slow_ms.to_le_bytes());
        out.extend_from_slice(&self.stunned_ms.to_le_bytes());
    }
}

impl SnapshotDecode for HudStatusMsg {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let tag = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if tag != TAG_HUD_STATUS {
            bail!("not a HudStatus tag");
        }
        let v = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if v != HUD_STATUS_VERSION {
            bail!("unsupported version: {v}");
        }
        let mana = u16::from_le_bytes(take::<2>(inp)?);
        let mana_max = u16::from_le_bytes(take::<2>(inp)?);
        let gcd_ms = u16::from_le_bytes(take::<2>(inp)?);
        let n = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))? as usize;
        *inp = &inp[1..];
        let mut spell_cds = Vec::with_capacity(n);
        for _ in 0..n {
            let id = inp
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("short read"))?;
            *inp = &inp[1..];
            let ms = u16::from_le_bytes(take::<2>(inp)?);
            spell_cds.push((id, ms));
        }
        let burning_ms = u16::from_le_bytes(take::<2>(inp)?);
        let slow_ms = u16::from_le_bytes(take::<2>(inp)?);
        let stunned_ms = u16::from_le_bytes(take::<2>(inp)?);
        Ok(HudStatusMsg {
            v,
            mana,
            mana_max,
            gcd_ms,
            spell_cds,
            burning_ms,
            slow_ms,
            stunned_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// HUD toast (short, transient client message)
// ---------------------------------------------------------------------------

pub const TAG_HUD_TOAST: u8 = 0xB2;
pub const HUD_TOAST_VERSION: u8 = 1;

/// Minimal HUD toast: client shows a short message based on code.
/// Codes: 1 = Not enough mana
#[derive(Debug, Clone, PartialEq)]
pub struct HudToastMsg {
    pub v: u8,
    pub code: u8,
}

impl SnapshotEncode for HudToastMsg {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_HUD_TOAST);
        out.push(self.v);
        out.push(self.code);
    }
}

impl SnapshotDecode for HudToastMsg {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        use anyhow::bail;
        let tag = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if tag != TAG_HUD_TOAST {
            bail!("not a HudToast tag");
        }
        let v = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if v != HUD_TOAST_VERSION {
            bail!("unsupported version: {v}");
        }
        let code = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        Ok(HudToastMsg { v, code })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct WizardRep {
    pub id: u32,
    pub kind: u8, // 0=PC,1=NPC wizard
    pub pos: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcRep {
    pub id: u32,
    pub archetype: u8, // 0=Zombie, 1=DK, etc.
    pub pos: [f32; 3],
    pub yaw: f32,
    pub radius: f32,
    pub hp: i32,
    pub max: i32,
    pub alive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileRep {
    pub id: u32,
    pub kind: u8, // 0=firebolt,1=fireball,..
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BossRep {
    pub id: u32,
    pub name: String,
    pub pos: [f32; 3],
    pub hp: i32,
    pub max: i32,
    pub ac: i32,
}

/// Minimal replicated record for a destructible instance's world AABB.
#[derive(Debug, Clone, PartialEq)]
pub struct DestructibleInstance {
    pub did: u64,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
}

impl SnapshotEncode for DestructibleInstance {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(VERSION);
        out.extend_from_slice(&self.did.to_le_bytes());
        for c in &self.world_min {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for c in &self.world_max {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

impl SnapshotDecode for DestructibleInstance {
    fn decode(inp: &mut &[u8]) -> anyhow::Result<Self> {
        fn take<const N: usize>(inp: &mut &[u8]) -> anyhow::Result<[u8; N]> {
            if inp.len() < N {
                anyhow::bail!("short read");
            }
            let (a, b) = inp.split_at(N);
            *inp = b;
            let mut buf = [0u8; N];
            buf.copy_from_slice(a);
            Ok(buf)
        }
        let ver = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        if ver != VERSION {
            anyhow::bail!("unsupported version: {ver}");
        }
        let did = u64::from_le_bytes(take::<8>(inp)?);
        let mut world_min = [0.0f32; 3];
        for v in &mut world_min {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        let mut world_max = [0.0f32; 3];
        for v in &mut world_max {
            *v = f32::from_le_bytes(take::<4>(inp)?);
        }
        Ok(Self {
            did,
            world_min,
            world_max,
        })
    }
}

// Compact list of NPC statuses for client UI/presentation.
// Legacy snapshot messages (BossStatusMsg/NpcListMsg) have been removed from runtime.

#[derive(Debug, Clone, PartialEq)]
pub struct NpcItem {
    pub id: u32,
    pub hp: i32,
    pub max: i32,
    pub pos: [f32; 3],
    pub radius: f32,
    pub alive: u8,
    pub attack_anim: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delta_roundtrip_naive() {
        let d = ChunkMeshDelta {
            did: 1,
            chunk: (2, 3, 4),
            positions: vec![[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]],
            normals: vec![[0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
        };
        let mut buf = Vec::new();
        d.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let d2 = ChunkMeshDelta::decode(&mut slice).expect("decode");
        assert_eq!(d.did, d2.did);
        assert_eq!(d.chunk, d2.chunk);
        assert_eq!(d.positions.len(), d2.positions.len());
        assert_eq!(d.indices, d2.indices);
    }
    #[test]
    fn destructible_instance_roundtrip() {
        let d = DestructibleInstance {
            did: 42,
            world_min: [-1.0, 0.0, -2.0],
            world_max: [3.0, 4.0, 5.0],
        };
        let mut buf = Vec::new();
        d.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let d2 = DestructibleInstance::decode(&mut slice).expect("decode");
        assert_eq!(d, d2);
    }
    // Legacy BossStatusMsg/NpcListMsg removed from runtime.
    // Legacy TickSnapshot roundtrip removed.

    #[test]
    fn actor_delta_roundtrip_minimal() {
        let delta = ActorSnapshotDelta {
            v: ACTOR_SNAP_DELTA_VERSION,
            tick: 42,
            baseline: 40,
            spawns: vec![ActorRep {
                id: 1,
                kind: 0,
                faction: 0,
                archetype_id: 0,
                name_id: 0,
                unique: 0,
                pos: [1.0, 2.0, 3.0],
                yaw: 0.5,
                radius: 0.6,
                hp: 99,
                max: 100,
                alive: true,
            }],
            updates: vec![ActorDeltaRec {
                id: 1,
                flags: 1 | 2 | 4 | 8,
                qpos: [qpos(1.0), qpos(2.0), qpos(3.0)],
                qyaw: qyaw(0.5),
                hp: 98,
                alive: 1,
            }],
            removals: vec![2, 3],
            projectiles: vec![ProjectileRep {
                id: 9,
                kind: 0,
                pos: [0.0, 1.0, 2.0],
                vel: [3.0, 4.0, 5.0],
            }],
            hits: vec![HitFx {
                kind: 0,
                pos: [1.0, 0.6, 2.0],
            }],
        };
        let mut buf = Vec::new();
        delta.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let dec = ActorSnapshotDelta::decode(&mut slice).expect("decode");
        assert_eq!(dec.v, ACTOR_SNAP_DELTA_VERSION);
        assert_eq!(dec.tick, 42);
        assert_eq!(dec.baseline, 40);
        assert_eq!(dec.spawns.len(), 1);
        assert_eq!(dec.updates.len(), 1);
        assert_eq!(dec.removals, vec![2, 3]);
        assert_eq!(dec.projectiles.len(), 1);
        assert_eq!(dec.hits.len(), 1);
    }

    #[test]
    fn actor_delta_rejects_bad_tag_and_version() {
        // bad tag
        let buf = vec![0xEE];
        let mut slice: &[u8] = &buf;
        assert!(ActorSnapshotDelta::decode(&mut slice).is_err());
        // good tag, bad version
        let buf = vec![TAG_ACTOR_SNAPSHOT_DELTA, 99];
        let mut slice: &[u8] = &buf;
        assert!(ActorSnapshotDelta::decode(&mut slice).is_err());
    }

    #[test]
    fn hud_status_roundtrip() {
        let msg = HudStatusMsg {
            v: HUD_STATUS_VERSION,
            mana: 10,
            mana_max: 20,
            gcd_ms: 250,
            spell_cds: vec![(0, 0), (1, 1500), (2, 500)],
            burning_ms: 1000,
            slow_ms: 500,
            stunned_ms: 0,
        };
        let mut buf = Vec::new();
        msg.encode(&mut buf);
        let mut slice: &[u8] = &buf;
        let dec = HudStatusMsg::decode(&mut slice).expect("decode");
        assert_eq!(msg, dec);
    }

    #[test]
    fn hud_status_rejects_bad() {
        let buf = vec![0xEE, 1, 0, 0];
        let mut slice: &[u8] = &buf;
        assert!(HudStatusMsg::decode(&mut slice).is_err());
    }
}
