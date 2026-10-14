//! STL meshes, binary or ASCII, read into bounded triangle lists.

/// The most triangles one mesh may hold.
pub const MAX_TRIANGLES: usize = 1_000_000;

/// A triangle list in the mesh file's own units and axes.
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    /// Each triangle's three corners.
    pub triangles: Vec<[[f32; 3]; 3]>,
    /// The smallest corner of the bounding box.
    pub min: [f32; 3],
    /// The largest corner of the bounding box.
    pub max: [f32; 3],
}

impl TriangleMesh {
    fn new(triangles: Vec<[[f32; 3]; 3]>) -> Result<Self, String> {
        if triangles.is_empty() {
            return Err("has no triangles".into());
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for corner in triangles.iter().flatten() {
            if !corner.iter().all(|c| c.is_finite()) {
                return Err("has a corner that is not a finite number".into());
            }
            for axis in 0..3 {
                min[axis] = min[axis].min(corner[axis]);
                max[axis] = max[axis].max(corner[axis]);
            }
        }
        Ok(Self {
            triangles,
            min,
            max,
        })
    }
}

/// Reads an STL file: binary when its length matches its triangle count,
/// otherwise ASCII starting with `solid`.
pub fn parse(bytes: &[u8]) -> Result<TriangleMesh, String> {
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if count.checked_mul(50).and_then(|n| n.checked_add(84)) == Some(bytes.len()) {
            return binary(bytes, count);
        }
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "is neither binary STL nor ASCII STL".to_string())?;
    if !text.trim_start().starts_with("solid") {
        return Err("is neither binary STL nor ASCII STL".into());
    }
    ascii(text)
}

fn binary(bytes: &[u8], count: usize) -> Result<TriangleMesh, String> {
    if count > MAX_TRIANGLES {
        return Err(format!(
            "has {count} triangles, over the limit of {MAX_TRIANGLES}"
        ));
    }
    let float =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let triangles = (0..count)
        .map(|i| {
            // Skip the 12-byte facet normal; corners follow.
            let base = 84 + i * 50 + 12;
            std::array::from_fn(|corner| {
                std::array::from_fn(|axis| float(base + corner * 12 + axis * 4))
            })
        })
        .collect();
    TriangleMesh::new(triangles)
}

fn ascii(text: &str) -> Result<TriangleMesh, String> {
    let mut corners = Vec::new();
    let mut words = text.split_ascii_whitespace();
    while let Some(word) = words.next() {
        if word != "vertex" {
            continue;
        }
        let mut corner = [0.0f32; 3];
        for value in &mut corner {
            *value = words
                .next()
                .and_then(|w| w.parse().ok())
                .ok_or("has a vertex without three numbers")?;
        }
        corners.push(corner);
        if corners.len() > MAX_TRIANGLES * 3 {
            return Err(format!("has over {MAX_TRIANGLES} triangles"));
        }
    }
    if corners.len() % 3 != 0 {
        return Err("has a facet without three vertices".into());
    }
    TriangleMesh::new(
        corners
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_and_ascii_read_the_same_triangle() {
        let mut bytes = vec![0u8; 80];
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 12]);
        for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&[0u8; 2]);
        let binary = parse(&bytes).unwrap();
        let ascii = parse(
            b"solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 2 0\nendloop\nendfacet\nendsolid t\n",
        )
        .unwrap();
        assert_eq!(binary, ascii);
        assert_eq!(binary.max, [1.0, 2.0, 0.0]);
    }

    #[test]
    fn other_bytes_and_broken_facets_are_refused() {
        assert!(parse(b"glTF binary").is_err());
        assert!(parse(b"solid t\nvertex 0 0 0\nvertex 1 0 0\nendsolid").is_err());
        assert!(parse(b"solid empty\nendsolid empty").is_err());
    }
}
