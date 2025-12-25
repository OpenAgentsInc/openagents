use crate::hit_test::{Hit, HitTestEntry, NodeId};
use crate::{Bounds, Point};

#[derive(Clone, Debug)]
pub struct DispatchNode {
    pub node_id: NodeId,
    pub bounds: Bounds,
    pub depth: u32,
}

#[derive(Default)]
pub struct DispatchTree {
    nodes: Vec<DispatchNode>,
}

impl DispatchTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    pub fn register(&mut self, id: u64, bounds: Bounds, depth: u32) -> NodeId {
        let node_id = NodeId(id);
        self.nodes.push(DispatchNode {
            node_id,
            bounds,
            depth,
        });
        node_id
    }

    pub fn nodes(&self) -> &[DispatchNode] {
        &self.nodes
    }

    pub fn hit_test(&self, point: Point) -> Hit {
        let mut entries: Vec<HitTestEntry> = self
            .nodes
            .iter()
            .filter(|node| node.bounds.contains(point))
            .map(|node| HitTestEntry {
                node_id: node.node_id,
                depth: node.depth,
            })
            .collect();

        entries.sort_by(|a, b| b.depth.cmp(&a.depth));
        Hit { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_hit_test_ordering() {
        let mut dispatch = DispatchTree::new();
        dispatch.register(1, Bounds::new(0.0, 0.0, 100.0, 100.0), 0);
        dispatch.register(2, Bounds::new(10.0, 10.0, 20.0, 20.0), 2);
        dispatch.register(3, Bounds::new(5.0, 5.0, 30.0, 30.0), 1);

        let hit = dispatch.hit_test(Point::new(12.0, 12.0));
        assert_eq!(hit.entries.len(), 3);
        assert_eq!(hit.entries[0].node_id, NodeId(2));
        assert_eq!(hit.entries[1].node_id, NodeId(3));
        assert_eq!(hit.entries[2].node_id, NodeId(1));
    }
}
