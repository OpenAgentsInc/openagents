//! Batched web calls (ADR 0004): a pass's commands as `u32` words in Wasm
//! memory, replayed by one JavaScript call (`replay` in
//! `apps/render-lab/gpu-batch.ts`) instead of one import call each.
//!
//! Each command is an opcode word followed by its arguments. Handles and
//! counts are one word; byte offsets and sizes are two (low, high), so any
//! value up to 2^53 survives the trip to a JavaScript number. `u64::MAX`
//! (`WGPU_WHOLE_SIZE`) means "to the end of the buffer". The pass a buffer
//! belongs to is an argument of the replay call, not of each command.
//!
//! This module is plain Rust, so it builds and is tested on every target; the
//! web backend (`web.rs`) writes through it.

/// The opcodes, shared with `OP` in `apps/render-lab/gpu-batch.ts`.
pub mod op {
    pub const SET_PIPELINE: u32 = 1;
    pub const SET_BIND_GROUP: u32 = 2;
    pub const SET_VERTEX_BUFFER: u32 = 3;
    pub const SET_INDEX_BUFFER: u32 = 4;
    pub const DRAW: u32 = 5;
    pub const DRAW_INDEXED: u32 = 6;
    pub const DRAW_INDIRECT: u32 = 7;
    pub const DRAW_INDEXED_INDIRECT: u32 = 8;
    pub const DISPATCH: u32 = 9;
    pub const DISPATCH_INDIRECT: u32 = 10;
    pub const END_PASS: u32 = 11;
}

/// One pass command, with handles as raw `u32` table indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    SetPipeline {
        pipeline: u32,
    },
    SetBindGroup {
        index: u32,
        group: u32,
    },
    SetVertexBuffer {
        slot: u32,
        buffer: u32,
        offset: u64,
        size: u64,
    },
    SetIndexBuffer {
        buffer: u32,
        uint32: bool,
        offset: u64,
        size: u64,
    },
    Draw {
        vertices: u32,
        instances: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    DrawIndexed {
        indices: u32,
        instances: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    },
    DrawIndirect {
        buffer: u32,
        offset: u64,
    },
    DrawIndexedIndirect {
        buffer: u32,
        offset: u64,
    },
    Dispatch {
        x: u32,
        y: u32,
        z: u32,
    },
    DispatchIndirect {
        buffer: u32,
        offset: u64,
    },
    EndPass,
}

impl Command {
    /// Words this command takes, opcode included.
    pub fn words(&self) -> usize {
        match self {
            Command::EndPass => 1,
            Command::SetPipeline { .. } => 2,
            Command::SetBindGroup { .. } => 3,
            Command::DrawIndirect { .. }
            | Command::DrawIndexedIndirect { .. }
            | Command::DispatchIndirect { .. } => 4,
            Command::Dispatch { .. } => 4,
            Command::Draw { .. } => 5,
            Command::DrawIndexed { .. } => 6,
            Command::SetVertexBuffer { .. } | Command::SetIndexBuffer { .. } => 7,
        }
    }
}

fn split(value: u64) -> [u32; 2] {
    [value as u32, (value >> 32) as u32]
}

fn join(low: u32, high: u32) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}

/// Appends one command to `out`.
pub fn encode(out: &mut Vec<u32>, command: &Command) {
    match *command {
        Command::SetPipeline { pipeline } => out.extend_from_slice(&[op::SET_PIPELINE, pipeline]),
        Command::SetBindGroup { index, group } => {
            out.extend_from_slice(&[op::SET_BIND_GROUP, index, group])
        }
        Command::SetVertexBuffer {
            slot,
            buffer,
            offset,
            size,
        } => {
            let [o0, o1] = split(offset);
            let [s0, s1] = split(size);
            out.extend_from_slice(&[op::SET_VERTEX_BUFFER, slot, buffer, o0, o1, s0, s1]);
        }
        Command::SetIndexBuffer {
            buffer,
            uint32,
            offset,
            size,
        } => {
            let [o0, o1] = split(offset);
            let [s0, s1] = split(size);
            out.extend_from_slice(&[op::SET_INDEX_BUFFER, buffer, uint32 as u32, o0, o1, s0, s1]);
        }
        Command::Draw {
            vertices,
            instances,
            first_vertex,
            first_instance,
        } => {
            out.extend_from_slice(&[op::DRAW, vertices, instances, first_vertex, first_instance]);
        }
        Command::DrawIndexed {
            indices,
            instances,
            first_index,
            base_vertex,
            first_instance,
        } => {
            out.extend_from_slice(&[
                op::DRAW_INDEXED,
                indices,
                instances,
                first_index,
                base_vertex as u32,
                first_instance,
            ]);
        }
        Command::DrawIndirect { buffer, offset } => {
            let [o0, o1] = split(offset);
            out.extend_from_slice(&[op::DRAW_INDIRECT, buffer, o0, o1]);
        }
        Command::DrawIndexedIndirect { buffer, offset } => {
            let [o0, o1] = split(offset);
            out.extend_from_slice(&[op::DRAW_INDEXED_INDIRECT, buffer, o0, o1]);
        }
        Command::Dispatch { x, y, z } => out.extend_from_slice(&[op::DISPATCH, x, y, z]),
        Command::DispatchIndirect { buffer, offset } => {
            let [o0, o1] = split(offset);
            out.extend_from_slice(&[op::DISPATCH_INDIRECT, buffer, o0, o1]);
        }
        Command::EndPass => out.push(op::END_PASS),
    }
}

/// Why words could not be read back as commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    UnknownOpcode { at: usize, opcode: u32 },
    Truncated { at: usize },
}

/// Reads words back into commands; the reference for the JavaScript replay.
pub fn decode(words: &[u32]) -> Result<Vec<Command>, DecodeError> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < words.len() {
        let opcode = words[at];
        let arity = match opcode {
            op::END_PASS => 0,
            op::SET_PIPELINE => 1,
            op::SET_BIND_GROUP => 2,
            op::DRAW_INDIRECT
            | op::DRAW_INDEXED_INDIRECT
            | op::DISPATCH_INDIRECT
            | op::DISPATCH => 3,
            op::DRAW => 4,
            op::DRAW_INDEXED => 5,
            op::SET_VERTEX_BUFFER | op::SET_INDEX_BUFFER => 6,
            _ => return Err(DecodeError::UnknownOpcode { at, opcode }),
        };
        let Some(a) = words.get(at + 1..at + 1 + arity) else {
            return Err(DecodeError::Truncated { at });
        };
        out.push(match opcode {
            op::SET_PIPELINE => Command::SetPipeline { pipeline: a[0] },
            op::SET_BIND_GROUP => Command::SetBindGroup {
                index: a[0],
                group: a[1],
            },
            op::SET_VERTEX_BUFFER => Command::SetVertexBuffer {
                slot: a[0],
                buffer: a[1],
                offset: join(a[2], a[3]),
                size: join(a[4], a[5]),
            },
            op::SET_INDEX_BUFFER => Command::SetIndexBuffer {
                buffer: a[0],
                uint32: a[1] != 0,
                offset: join(a[2], a[3]),
                size: join(a[4], a[5]),
            },
            op::DRAW => Command::Draw {
                vertices: a[0],
                instances: a[1],
                first_vertex: a[2],
                first_instance: a[3],
            },
            op::DRAW_INDEXED => Command::DrawIndexed {
                indices: a[0],
                instances: a[1],
                first_index: a[2],
                base_vertex: a[3] as i32,
                first_instance: a[4],
            },
            op::DRAW_INDIRECT => Command::DrawIndirect {
                buffer: a[0],
                offset: join(a[1], a[2]),
            },
            op::DRAW_INDEXED_INDIRECT => Command::DrawIndexedIndirect {
                buffer: a[0],
                offset: join(a[1], a[2]),
            },
            op::DISPATCH => Command::Dispatch {
                x: a[0],
                y: a[1],
                z: a[2],
            },
            op::DISPATCH_INDIRECT => Command::DispatchIndirect {
                buffer: a[0],
                offset: join(a[1], a[2]),
            },
            _ => Command::EndPass,
        });
        at += 1 + arity;
    }
    Ok(out)
}

/// One of every command kind, with edge values (a negative base vertex,
/// offsets past 4 GiB, and a whole-buffer size). The JavaScript replay test
/// reads the same list from `fixtures/batch.json`.
pub fn canonical() -> Vec<Command> {
    vec![
        Command::SetPipeline { pipeline: 7 },
        Command::SetBindGroup {
            index: 1,
            group: 42,
        },
        Command::SetVertexBuffer {
            slot: 1,
            buffer: 9,
            offset: 5_000_000_000,
            size: 4096,
        },
        Command::SetVertexBuffer {
            slot: 0,
            buffer: 3,
            offset: 0,
            size: u64::MAX,
        },
        Command::SetIndexBuffer {
            buffer: 4,
            uint32: true,
            offset: 256,
            size: u64::MAX,
        },
        Command::SetIndexBuffer {
            buffer: 5,
            uint32: false,
            offset: 0,
            size: 64,
        },
        Command::Draw {
            vertices: 3,
            instances: 1,
            first_vertex: 0,
            first_instance: 0,
        },
        Command::DrawIndexed {
            indices: 36,
            instances: 120,
            first_index: 6,
            base_vertex: -4,
            first_instance: 2,
        },
        Command::DrawIndirect {
            buffer: 11,
            offset: 16,
        },
        Command::DrawIndexedIndirect {
            buffer: 12,
            offset: (1 << 40) + 20,
        },
        Command::Dispatch { x: 64, y: 2, z: 1 },
        Command::DispatchIndirect {
            buffer: 13,
            offset: 12,
        },
        Command::EndPass,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_all(commands: &[Command]) -> Vec<u32> {
        let mut words = Vec::new();
        for c in commands {
            encode(&mut words, c);
        }
        words
    }

    #[test]
    fn every_command_kind_round_trips() {
        let commands = canonical();
        let words = encode_all(&commands);
        assert_eq!(
            words.len(),
            commands.iter().map(Command::words).sum::<usize>()
        );
        assert_eq!(decode(&words).unwrap(), commands);
        // Every opcode appears.
        let mut seen: Vec<u32> = Vec::new();
        let mut at = 0;
        for c in &commands {
            seen.push(words[at]);
            at += c.words();
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen, (1..=11).collect::<Vec<u32>>());
    }

    #[test]
    fn extremes_survive() {
        for offset in [
            0,
            1,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            (1 << 53) - 1,
            u64::MAX,
        ] {
            let c = Command::SetVertexBuffer {
                slot: 7,
                buffer: u32::MAX,
                offset,
                size: offset,
            };
            assert_eq!(decode(&encode_all(&[c])).unwrap(), vec![c]);
        }
        for base_vertex in [i32::MIN, -1, 0, i32::MAX] {
            let c = Command::DrawIndexed {
                indices: 1,
                instances: 1,
                first_index: 0,
                base_vertex,
                first_instance: 0,
            };
            assert_eq!(decode(&encode_all(&[c])).unwrap(), vec![c]);
        }
    }

    #[test]
    fn bad_words_are_refused() {
        assert_eq!(
            decode(&[99]),
            Err(DecodeError::UnknownOpcode { at: 0, opcode: 99 })
        );
        assert_eq!(
            decode(&[op::DRAW, 3, 1]),
            Err(DecodeError::Truncated { at: 0 })
        );
        assert_eq!(
            decode(&[op::END_PASS, 0]),
            Err(DecodeError::UnknownOpcode { at: 1, opcode: 0 })
        );
    }

    /// The JavaScript replay test reads this fixture; regenerate it with
    /// `AF_WRITE_FIXTURES=1 cargo test -p af-gpu`.
    #[test]
    fn fixture_is_current() {
        let words = encode_all(&canonical());
        let text = format!(
            "{{\n  \"note\": \"af_gpu::batch::canonical(), encoded. Regenerate with AF_WRITE_FIXTURES=1 cargo test -p af-gpu.\",\n  \"words\": [{}]\n}}\n",
            words
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/batch.json");
        if std::env::var_os("AF_WRITE_FIXTURES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap_or_default(),
            text,
            "fixtures/batch.json is stale"
        );
    }
}
