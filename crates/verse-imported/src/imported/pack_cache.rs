//! A pack cached on disk for quick reloads: the pack as JSON without its
//! geometry, and the geometry as raw little-endian arrays beside it (JSON of
//! millions of vertices takes a minute to read). For large imported
//! characters, such as a MetaHuman export with its strand groom.
use std::io::{Read, Write};
use std::path::Path;
use verse_engine::assets::{Pack, Vertex};

/// The pack as JSON without its geometry, and the geometry as raw
/// little-endian arrays beside it (JSON of three million vertices would
/// take a minute to read).
pub fn write_pack(dir: &Path, pack: &Pack) -> Result<(), String> {
    let mut light = pack.clone();
    let mut bin = std::io::BufWriter::new(
        std::fs::File::create(dir.join("geometry.bin")).map_err(|e| e.to_string())?,
    );
    for model in light.models.values_mut() {
        for s in &mut model.surfaces {
            let mut put = |x: u32| bin.write_all(&x.to_le_bytes()).map_err(|e| e.to_string());
            put(s.vertices.len() as u32)?;
            put(s.indices.len() as u32)?;
            for v in &s.vertices {
                for x in v.position.iter().chain(&v.normal).chain(&v.uv) {
                    put(x.to_bits())?;
                }
                for j in v.joints {
                    put(j)?;
                }
                for w in v.weights {
                    put(w.to_bits())?;
                }
            }
            for &i in &s.indices {
                put(i)?;
            }
            s.vertices.clear();
            s.indices.clear();
        }
    }
    bin.flush().map_err(|e| e.to_string())?;
    let json = serde_json::to_vec(&light).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("pack.json"), json).map_err(|e| e.to_string())
}

pub fn read_pack(dir: &Path) -> Result<Pack, String> {
    let json = std::fs::read(dir.join("pack.json")).map_err(|e| e.to_string())?;
    let mut pack: Pack = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::fs::File::open(dir.join("geometry.bin"))
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u32], String> {
        let s = words.get(at..at + n).ok_or("The geometry cache is short")?;
        at += n;
        Ok(s)
    };
    let f = |x: u32| f32::from_bits(x);
    for model in pack.models.values_mut() {
        for s in &mut model.surfaces {
            let head = take(2)?;
            let (nv, ni) = (head[0] as usize, head[1] as usize);
            let raw = take(nv * 16)?;
            s.vertices = raw
                .chunks_exact(16)
                .map(|w| Vertex {
                    position: [f(w[0]), f(w[1]), f(w[2])],
                    normal: [f(w[3]), f(w[4]), f(w[5])],
                    uv: [f(w[6]), f(w[7])],
                    joints: [w[8], w[9], w[10], w[11]],
                    weights: [f(w[12]), f(w[13]), f(w[14]), f(w[15])],
                })
                .collect();
            s.indices = take(ni)?.to_vec();
        }
    }
    Ok(pack)
}
