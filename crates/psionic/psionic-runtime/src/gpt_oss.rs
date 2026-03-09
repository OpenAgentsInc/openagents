/// Backend-agnostic GPT-OSS decode-graph node kinds mirrored from the current
/// OpenAI-MoE high-level graph order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GptOssDecodeGraphNodeKind {
    AttnNorm,
    AttnQkv,
    AttnQRope,
    AttnKRope,
    AttnOut,
    FfnInp,
    AttnPostNorm,
    FfnMoeTopk,
    FfnMoeGateUp,
    FfnMoeDown,
    FfnMoeOut,
    ResultNorm,
    ResultOutput,
}

/// One backend-agnostic GPT-OSS decode-graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GptOssDecodeGraphNode {
    pub kind: GptOssDecodeGraphNodeKind,
    pub name: &'static str,
}

impl GptOssDecodeGraphNode {
    #[must_use]
    pub const fn new(kind: GptOssDecodeGraphNodeKind, name: &'static str) -> Self {
        Self { kind, name }
    }
}

/// Reusable high-level GPT-OSS decode-graph shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GptOssDecodeGraph {
    pub layer_nodes: Vec<Vec<GptOssDecodeGraphNode>>,
    pub terminal_nodes: Vec<GptOssDecodeGraphNode>,
}

impl GptOssDecodeGraph {
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.layer_nodes.iter().map(Vec::len).sum::<usize>() + self.terminal_nodes.len()
    }

    #[must_use]
    pub fn layer_node_count(&self) -> usize {
        self.layer_nodes.first().map_or(0, Vec::len)
    }

    #[must_use]
    pub fn signature_key(&self) -> String {
        let mut names = Vec::with_capacity(self.node_count());
        for layer in &self.layer_nodes {
            for node in layer {
                names.push(node.name);
            }
        }
        for node in &self.terminal_nodes {
            names.push(node.name);
        }
        names.join("|")
    }
}

fn gpt_oss_layer_decode_graph_nodes() -> Vec<GptOssDecodeGraphNode> {
    vec![
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnNorm, "attn_norm"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnQkv, "attn_qkv"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnQRope, "attn_q_rope"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnKRope, "attn_k_rope"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnOut, "attn_out"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnInp, "ffn_inp"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::AttnPostNorm, "attn_post_norm"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeTopk, "ffn_moe_topk"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeGateUp, "ffn_moe_gate_up"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeDown, "ffn_moe_down"),
        GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::FfnMoeOut, "ffn_moe_out"),
    ]
}

/// Builds the reusable GPT-OSS high-level decode graph for one decoder depth.
#[must_use]
pub fn build_gpt_oss_decode_graph(layer_count: usize) -> GptOssDecodeGraph {
    GptOssDecodeGraph {
        layer_nodes: (0..layer_count)
            .map(|_| gpt_oss_layer_decode_graph_nodes())
            .collect(),
        terminal_nodes: vec![
            GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::ResultNorm, "result_norm"),
            GptOssDecodeGraphNode::new(GptOssDecodeGraphNodeKind::ResultOutput, "result_output"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::build_gpt_oss_decode_graph;

    #[test]
    fn gpt_oss_decode_graph_matches_llama_cpp_high_level_order() {
        let graph = build_gpt_oss_decode_graph(2);
        let layer_names = graph.layer_nodes[0]
            .iter()
            .map(|node| node.name)
            .collect::<Vec<_>>();
        assert_eq!(
            layer_names,
            vec![
                "attn_norm",
                "attn_qkv",
                "attn_q_rope",
                "attn_k_rope",
                "attn_out",
                "ffn_inp",
                "attn_post_norm",
                "ffn_moe_topk",
                "ffn_moe_gate_up",
                "ffn_moe_down",
                "ffn_moe_out",
            ]
        );
        let terminal_names = graph
            .terminal_nodes
            .iter()
            .map(|node| node.name)
            .collect::<Vec<_>>();
        assert_eq!(terminal_names, vec!["result_norm", "result_output"]);
        assert_eq!(graph.layer_node_count(), 11);
        assert_eq!(graph.node_count(), 24);
    }

    #[test]
    fn decode_graph_signature_changes_with_layer_count() {
        let short_graph = build_gpt_oss_decode_graph(1);
        let long_graph = build_gpt_oss_decode_graph(2);
        assert_ne!(short_graph.signature_key(), long_graph.signature_key());
    }
}
