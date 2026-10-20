//! AgentGraph's GPUI view: any agent's graph (`crates/agent-graph`) drawn
//! with pan, zoom, drag and select, shared by the desktop's Agent Lab and
//! the web build (`web/agent-graph`).
//!
//! Units are drawn after Unit (samuelmtimbo/unit, MIT): rounded boxes with
//! typed pins on their edges, graph units holding their children, and data
//! moving along the links. Given an [`agent_graph::edit::Editable`], the
//! side panel offers changes and the view emits them as
//! [`GraphEvent::Edit`]; the host applies them through Agent Lab and calls
//! [`GraphView::set_graph`] with the result.

pub mod theme;
mod view;

pub use view::{GraphEvent, GraphView, changes};

#[cfg(test)]
mod tests {
    use agent_graph::edit::{Edit, Editable};

    #[test]
    fn every_change_label_reads_plainly() {
        let mut editable = Editable {
            check_ins: true,
            room_for_skill: true,
            ..Default::default()
        };
        let mut labels = vec![];
        for id in [
            "runtime",
            "check-ins",
            "authority",
            "stalls",
            "instructions",
            "tools",
            "sources",
            "skill-1",
            "source-the-app",
            "tools-chats",
        ] {
            labels.extend(super::changes(id, &editable).into_iter().map(|(l, _)| l));
        }
        editable.named = vec!["read_chat".into()];
        labels.extend(
            super::changes("tools-chats", &editable)
                .into_iter()
                .map(|(l, _)| l),
        );
        assert!(labels.contains(&"Remove read_chat".to_owned()));
        for label in &labels {
            assert!(oa_copy::violations(label, &[]).is_empty(), "{label}");
            assert!(!label.to_lowercase().contains("zeron"), "{label}");
        }
        assert!(
            super::changes("tools", &editable)
                .iter()
                .all(|(_, e)| !matches!(e, Edit::AddTool { name } if name == "read_chat"))
        );
    }
}
