use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kernel_math::Point3;
use crate::{CadError, CadResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VertexId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HalfEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LoopId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShellId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SolidId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vertex {
    pub point: Point3,
    pub half_edge: Option<HalfEdgeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HalfEdge {
    pub origin: VertexId,
    pub twin: Option<HalfEdgeId>,
    pub next: Option<HalfEdgeId>,
    pub prev: Option<HalfEdgeId>,
    pub edge: Option<EdgeId>,
    pub loop_id: Option<LoopId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub half_edge: HalfEdgeId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Loop {
    pub half_edge: HalfEdgeId,
    pub face: Option<FaceId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Orientation {
    Forward,
    Reversed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Face {
    pub outer_loop: LoopId,
    pub inner_loops: Vec<LoopId>,
    pub surface_index: usize,
    pub orientation: Orientation,
    pub shell: Option<ShellId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShellType {
    Outer,
    Void,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shell {
    pub faces: Vec<FaceId>,
    pub solid: Option<SolidId>,
    pub shell_type: ShellType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Solid {
    pub outer_shell: ShellId,
    pub void_shells: Vec<ShellId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Topology {
    pub vertices: BTreeMap<VertexId, Vertex>,
    pub half_edges: BTreeMap<HalfEdgeId, HalfEdge>,
    pub edges: BTreeMap<EdgeId, Edge>,
    pub loops: BTreeMap<LoopId, Loop>,
    pub faces: BTreeMap<FaceId, Face>,
    pub shells: BTreeMap<ShellId, Shell>,
    pub solids: BTreeMap<SolidId, Solid>,
    next_vertex: u64,
    next_half_edge: u64,
    next_edge: u64,
    next_loop: u64,
    next_face: u64,
    next_shell: u64,
    next_solid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyCounts {
    pub vertex_count: usize,
    pub half_edge_count: usize,
    pub edge_count: usize,
    pub loop_count: usize,
    pub face_count: usize,
    pub shell_count: usize,
    pub solid_count: usize,
}

impl Topology {
    pub fn new() -> Self {
        Self {
            vertices: BTreeMap::new(),
            half_edges: BTreeMap::new(),
            edges: BTreeMap::new(),
            loops: BTreeMap::new(),
            faces: BTreeMap::new(),
            shells: BTreeMap::new(),
            solids: BTreeMap::new(),
            next_vertex: 1,
            next_half_edge: 1,
            next_edge: 1,
            next_loop: 1,
            next_face: 1,
            next_shell: 1,
            next_solid: 1,
        }
    }

    pub fn add_vertex(&mut self, point: Point3) -> VertexId {
        let id = VertexId(self.next_vertex);
        self.next_vertex += 1;
        self.vertices.insert(
            id,
            Vertex {
                point,
                half_edge: None,
            },
        );
        id
    }

    pub fn add_half_edge(&mut self, origin: VertexId) -> CadResult<HalfEdgeId> {
        if !self.vertices.contains_key(&origin) {
            return Err(CadError::InvalidFeatureGraph {
                reason: format!("half-edge origin vertex not found: {}", origin.0),
            });
        }
        let id = HalfEdgeId(self.next_half_edge);
        self.next_half_edge += 1;
        self.half_edges.insert(
            id,
            HalfEdge {
                origin,
                twin: None,
                next: None,
                prev: None,
                edge: None,
                loop_id: None,
            },
        );
        if self
            .vertices
            .get(&origin)
            .and_then(|vertex| vertex.half_edge)
            .is_none()
        {
            self.vertices
                .get_mut(&origin)
                .expect("origin exists")
                .half_edge = Some(id);
        }
        Ok(id)
    }

    pub fn add_edge(&mut self, he1: HalfEdgeId, he2: HalfEdgeId) -> CadResult<EdgeId> {
        if !self.half_edges.contains_key(&he1) || !self.half_edges.contains_key(&he2) {
            return Err(CadError::InvalidFeatureGraph {
                reason: "edge requires existing half-edges".to_string(),
            });
        }
        let edge_id = EdgeId(self.next_edge);
        self.next_edge += 1;
        self.edges.insert(edge_id, Edge { half_edge: he1 });
        self.half_edges.get_mut(&he1).expect("exists").twin = Some(he2);
        self.half_edges.get_mut(&he2).expect("exists").twin = Some(he1);
        self.half_edges.get_mut(&he1).expect("exists").edge = Some(edge_id);
        self.half_edges.get_mut(&he2).expect("exists").edge = Some(edge_id);
        Ok(edge_id)
    }

    pub fn add_loop(&mut self, half_edges: &[HalfEdgeId]) -> CadResult<LoopId> {
        if half_edges.is_empty() {
            return Err(CadError::InvalidFeatureGraph {
                reason: "loop must have at least one half-edge".to_string(),
            });
        }
        for id in half_edges {
            if !self.half_edges.contains_key(id) {
                return Err(CadError::InvalidFeatureGraph {
                    reason: format!("loop contains missing half-edge: {}", id.0),
                });
            }
        }
        let loop_id = LoopId(self.next_loop);
        self.next_loop += 1;
        self.loops.insert(
            loop_id,
            Loop {
                half_edge: half_edges[0],
                face: None,
            },
        );
        let n = half_edges.len();
        for i in 0..n {
            let current = half_edges[i];
            let next = half_edges[(i + 1) % n];
            let prev = half_edges[(i + n - 1) % n];
            let he = self.half_edges.get_mut(&current).expect("exists");
            he.next = Some(next);
            he.prev = Some(prev);
            he.loop_id = Some(loop_id);
        }
        Ok(loop_id)
    }

    pub fn add_face(
        &mut self,
        outer_loop: LoopId,
        surface_index: usize,
        orientation: Orientation,
    ) -> CadResult<FaceId> {
        if !self.loops.contains_key(&outer_loop) {
            return Err(CadError::InvalidFeatureGraph {
                reason: format!("face outer loop not found: {}", outer_loop.0),
            });
        }
        let face_id = FaceId(self.next_face);
        self.next_face += 1;
        self.faces.insert(
            face_id,
            Face {
                outer_loop,
                inner_loops: Vec::new(),
                surface_index,
                orientation,
                shell: None,
            },
        );
        self.loops.get_mut(&outer_loop).expect("exists").face = Some(face_id);
        Ok(face_id)
    }

    pub fn add_inner_loop(&mut self, face_id: FaceId, inner_loop: LoopId) -> CadResult<()> {
        if !self.faces.contains_key(&face_id) || !self.loops.contains_key(&inner_loop) {
            return Err(CadError::InvalidFeatureGraph {
                reason: "inner loop requires existing face and loop".to_string(),
            });
        }
        self.faces
            .get_mut(&face_id)
            .expect("exists")
            .inner_loops
            .push(inner_loop);
        self.loops.get_mut(&inner_loop).expect("exists").face = Some(face_id);
        Ok(())
    }

    pub fn add_shell(&mut self, faces: Vec<FaceId>, shell_type: ShellType) -> CadResult<ShellId> {
        for face_id in &faces {
            if !self.faces.contains_key(face_id) {
                return Err(CadError::InvalidFeatureGraph {
                    reason: format!("shell face not found: {}", face_id.0),
                });
            }
        }
        let shell_id = ShellId(self.next_shell);
        self.next_shell += 1;
        for face_id in &faces {
            self.faces.get_mut(face_id).expect("exists").shell = Some(shell_id);
        }
        self.shells.insert(
            shell_id,
            Shell {
                faces,
                solid: None,
                shell_type,
            },
        );
        Ok(shell_id)
    }

    pub fn add_solid(&mut self, outer_shell: ShellId) -> CadResult<SolidId> {
        if !self.shells.contains_key(&outer_shell) {
            return Err(CadError::InvalidFeatureGraph {
                reason: format!("solid outer shell not found: {}", outer_shell.0),
            });
        }
        let solid_id = SolidId(self.next_solid);
        self.next_solid += 1;
        self.solids.insert(
            solid_id,
            Solid {
                outer_shell,
                void_shells: Vec::new(),
            },
        );
        self.shells.get_mut(&outer_shell).expect("exists").solid = Some(solid_id);
        Ok(solid_id)
    }

    pub fn validate_loop_ring(&self, loop_id: LoopId) -> CadResult<()> {
        let loop_data = self
            .loops
            .get(&loop_id)
            .ok_or_else(|| CadError::InvalidFeatureGraph {
                reason: format!("loop not found: {}", loop_id.0),
            })?;
        let start = loop_data.half_edge;
        let mut visited = BTreeMap::<HalfEdgeId, ()>::new();
        let mut current = start;
        for _ in 0..=self.half_edges.len().max(1) {
            if visited.insert(current, ()).is_some() {
                return Err(CadError::InvalidFeatureGraph {
                    reason: "loop traversal detected non-start cycle".to_string(),
                });
            }
            let next = self
                .half_edges
                .get(&current)
                .and_then(|half_edge| half_edge.next)
                .ok_or_else(|| CadError::InvalidFeatureGraph {
                    reason: format!("half-edge {} missing next pointer", current.0),
                })?;
            if next == start {
                return Ok(());
            }
            if visited.contains_key(&next) {
                return Err(CadError::InvalidFeatureGraph {
                    reason: "loop traversal detected non-start cycle".to_string(),
                });
            }
            current = next;
        }
        Err(CadError::InvalidFeatureGraph {
            reason: "loop traversal exceeded topology half-edge bound".to_string(),
        })
    }

    pub fn counts(&self) -> TopologyCounts {
        TopologyCounts {
            vertex_count: self.vertices.len(),
            half_edge_count: self.half_edges.len(),
            edge_count: self.edges.len(),
            loop_count: self.loops.len(),
            face_count: self.faces.len(),
            shell_count: self.shells.len(),
            solid_count: self.solids.len(),
        }
    }
}

impl Default for Topology {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Orientation, ShellType, Topology};
    use crate::kernel_math::Point3;

    #[test]
    fn topology_can_build_single_loop_face_shell_solid() {
        let mut topo = Topology::new();
        let v1 = topo.add_vertex(Point3::new(0.0, 0.0, 0.0));
        let v2 = topo.add_vertex(Point3::new(1.0, 0.0, 0.0));
        let v3 = topo.add_vertex(Point3::new(1.0, 1.0, 0.0));

        let he1 = topo.add_half_edge(v1).expect("he1");
        let he2 = topo.add_half_edge(v2).expect("he2");
        let he3 = topo.add_half_edge(v3).expect("he3");
        let _edge_12 = topo.add_edge(he1, he2).expect("edge 12");

        let loop_id = topo.add_loop(&[he1, he2, he3]).expect("loop");
        topo.validate_loop_ring(loop_id).expect("valid loop");
        let face = topo
            .add_face(loop_id, 0, Orientation::Forward)
            .expect("face");
        let shell = topo.add_shell(vec![face], ShellType::Outer).expect("shell");
        let _solid = topo.add_solid(shell).expect("solid");
        let counts = topo.counts();
        assert_eq!(counts.vertex_count, 3);
        assert_eq!(counts.half_edge_count, 3);
        assert_eq!(counts.edge_count, 1);
        assert_eq!(counts.loop_count, 1);
        assert_eq!(counts.face_count, 1);
        assert_eq!(counts.shell_count, 1);
        assert_eq!(counts.solid_count, 1);
    }

    #[test]
    fn invalid_empty_loop_is_rejected() {
        let mut topo = Topology::new();
        let error = topo.add_loop(&[]).expect_err("empty loop should fail");
        assert_eq!(
            error.to_string(),
            "invalid feature graph: loop must have at least one half-edge"
        );
    }
}
