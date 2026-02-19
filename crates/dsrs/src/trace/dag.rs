use crate::{Example, MetaSignature, Prediction};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub enum NodeType {
    Root, // Initial input
    Predict {
        signature_name: String,
        signature: Arc<dyn MetaSignature>,
    },
    Operator {
        name: String,
    },
    Map {
        // Describes: for each field in output, where does it come from?
        // Key: output field name
        // Value: (Node Index, input field name)
        mapping: Vec<(String, (usize, String))>,
    },
}

impl fmt::Debug for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "Root"),
            Self::Predict { signature_name, .. } => f
                .debug_struct("Predict")
                .field("signature_name", signature_name)
                .finish(),
            Self::Operator { name } => f.debug_struct("Operator").field("name", name).finish(),
            Self::Map { mapping } => f.debug_struct("Map").field("mapping", mapping).finish(),
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub id: usize,
    pub node_type: NodeType,
    pub inputs: Vec<usize>, // IDs of parent nodes
    pub output: Option<Prediction>,
    pub input_data: Option<Example>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("id", &self.id)
            .field("node_type", &self.node_type)
            .field("inputs", &self.inputs)
            .field("output", &self.output)
            .field("input_data", &self.input_data)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(
        &mut self,
        node_type: NodeType,
        inputs: Vec<usize>,
        input_data: Option<Example>,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            node_type,
            inputs,
            output: None,
            input_data,
        });
        id
    }

    pub fn set_output(&mut self, id: usize, output: Prediction) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.output = Some(output);
        }
    }

    pub fn get_node(&self, id: usize) -> Option<&Node> {
        self.nodes.get(id)
    }
}
