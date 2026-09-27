//! Client->Server commands (authoritative input/actions).
//!
//! Scope
//! - Defines a minimal binary protocol for client actions with a leading tag
//!   (`TAG_CLIENT_CMD`) distinct from `TickSnapshot` framing, so decoders can
//!   quickly reject the wrong payload type.
//! - Used by the client renderer to send cast/projectile actions to the server.
//!
//! Extending
//! - Add new enum variants (e.g., melee swings, toggles). Keep payloads small
//!   and fixed-size where possible. Versioning is handled at the message level
//!   with `TAG_CLIENT_CMD`. If the wire evolves, introduce a new tag.
//! - Never include gameplay tuning in commands (e.g., damage, radius). The
//!   server is authoritative over projectile parameters and resolves them from
//!   data specs. Clients send only intent (kind + origin + direction).

use crate::snapshot::SnapshotDecode;

pub const TAG_CLIENT_CMD: u8 = 0xC1;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientCmd {
    FireBolt { pos: [f32; 3], dir: [f32; 3] },
    Fireball { pos: [f32; 3], dir: [f32; 3] },
    MagicMissile { pos: [f32; 3], dir: [f32; 3] },
    // Authoritative movement/aim intents
    Move { dx: f32, dz: f32, run: u8 },
    Aim { yaw: f32 },
}

impl ClientCmd {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.push(TAG_CLIENT_CMD);
        match self {
            ClientCmd::FireBolt { pos, dir } => {
                out.push(0);
                for c in pos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
                for c in dir {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            ClientCmd::Fireball { pos, dir } => {
                out.push(1);
                for c in pos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
                for c in dir {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            ClientCmd::MagicMissile { pos, dir } => {
                out.push(2);
                for c in pos {
                    out.extend_from_slice(&c.to_le_bytes());
                }
                for c in dir {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
            ClientCmd::Move { dx, dz, run } => {
                out.push(3);
                out.extend_from_slice(&dx.to_le_bytes());
                out.extend_from_slice(&dz.to_le_bytes());
                out.push(*run);
            }
            ClientCmd::Aim { yaw } => {
                out.push(4);
                out.extend_from_slice(&yaw.to_le_bytes());
            }
        }
    }
}

impl SnapshotDecode for ClientCmd {
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
        if tag != TAG_CLIENT_CMD {
            bail!("not a client cmd tag");
        }
        let kind = inp
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("short read"))?;
        *inp = &inp[1..];
        let out = match kind {
            0..=2 => {
                let mut pos = [0.0f32; 3];
                for v in &mut pos {
                    *v = f32::from_le_bytes(take::<4>(inp)?);
                }
                let mut dir = [0.0f32; 3];
                for v in &mut dir {
                    *v = f32::from_le_bytes(take::<4>(inp)?);
                }
                if kind == 1 {
                    Self::Fireball { pos, dir }
                } else if kind == 2 {
                    Self::MagicMissile { pos, dir }
                } else {
                    Self::FireBolt { pos, dir }
                }
            }
            3 => {
                let dx = f32::from_le_bytes(take::<4>(inp)?);
                let dz = f32::from_le_bytes(take::<4>(inp)?);
                let run = inp
                    .first()
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("short read"))?;
                *inp = &inp[1..];
                Self::Move { dx, dz, run }
            }
            4 => {
                let yaw = f32::from_le_bytes(take::<4>(inp)?);
                Self::Aim { yaw }
            }
            _ => anyhow::bail!("unknown client cmd kind"),
        };
        Ok(out)
    }
}
