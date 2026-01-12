use crate::components::{ChainNode, NodeState};

pub fn demo_prompt() -> &'static str {
    "Summarize the markdown files in the root level of this repository."
}

pub fn demo_chain() -> Vec<ChainNode> {
    vec![
        ChainNode::new("TaskAnalysisSignature")
            .with_state(NodeState::Complete)
            .with_input("prompt", "Summarize the markdown files in the root level...")
            .with_output("task_type", "summarize")
            .with_output("file_pattern", "*.md")
            .with_output("scope", "root")
            .with_output("confidence", "0.95")
            .with_metrics(127, 0, 1200),
        ChainNode::new("FileDiscoverySignature")
            .with_state(NodeState::Complete)
            .with_input("pattern", "*.md")
            .with_input("scope", "root")
            .with_output(
                "paths",
                r#"["README.md", "CHANGELOG.md", "CONTRIBUTING.md", "LICENSE.md"]"#,
            )
            .with_output("count", "4")
            .with_metrics(89, 0, 450),
        ChainNode::new("ContentReaderSignature")
            .with_state(NodeState::Complete)
            .with_input("paths", "[4 files]")
            .with_output("total_size", "12847")
            .with_output("failed_paths", "[]")
            .with_metrics(0, 0, 23),
        ChainNode::new("ContentSummarizerSignature")
            .with_state(NodeState::Running)
            .with_input("filename", "README.md")
            .with_input("content_type", "markdown")
            .with_progress("Processing README.md (1/4)..."),
        ChainNode::new("SummaryAggregatorSignature")
            .with_state(NodeState::Pending),
    ]
}
