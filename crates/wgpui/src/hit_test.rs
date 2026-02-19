#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, Debug, Default)]
pub struct HitTestIndex(pub usize);

#[derive(Clone, Debug)]
pub struct HitTestEntry {
    pub node_id: NodeId,
    pub depth: u32,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub entries: Vec<HitTestEntry>,
}

impl Hit {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl Default for Hit {
    fn default() -> Self {
        Self::new()
    }
}
