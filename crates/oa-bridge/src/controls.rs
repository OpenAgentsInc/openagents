//! Control message schema and parser for WS payloads.
//!
//! The mobile/desktop clients send small JSON control messages over the same
//! WebSocket used for Codex stream broadcasting. This module defines the
//! supported control verbs and a tolerant parser that extracts them from the
//! inbound payload.

#[derive(Debug)]
pub enum ControlCommand {
    Interrupt,
    Projects,
    Skills,
    BridgeStatus,
    Echo { payload: Option<String>, tag: Option<String> },
    ConvexStatus,
    ConvexCreateDemo,
    ConvexCreateThreads,
    ConvexCreateDemoThread,
    ConvexBackfill,
    ProjectSave {
        project: crate::projects::Project,
    },
    ProjectDelete {
        id: String,
    },
    RunSubmit {
        thread_doc_id: String,
        text: String,
        project_id: Option<String>,
        resume_id: Option<String>,
    },
}

/// Parse a control command from a raw JSON string. Returns None on errors.
pub fn parse_control_command(payload: &str) -> Option<ControlCommand> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let ty = v.get("control").and_then(|x| x.as_str())?;
    match ty {
        "interrupt" => Some(ControlCommand::Interrupt),
        "projects" => Some(ControlCommand::Projects),
        "skills" => Some(ControlCommand::Skills),
        "echo" | "debug.echo" | "debug.ping" => {
            let payload = v.get("payload").and_then(|x| x.as_str()).map(|s| s.to_string());
            let tag = v.get("tag").and_then(|x| x.as_str()).map(|s| s.to_string());
            Some(ControlCommand::Echo { payload, tag })
        }
        "bridge.status" => Some(ControlCommand::BridgeStatus),
        "convex.status" => Some(ControlCommand::ConvexStatus),
        "convex.create_demo" => Some(ControlCommand::ConvexCreateDemo),
        "convex.create_threads" => Some(ControlCommand::ConvexCreateThreads),
        "convex.create_demo_thread" => Some(ControlCommand::ConvexCreateDemoThread),
        "convex.backfill" => Some(ControlCommand::ConvexBackfill),
        "project.save" => {
            let proj: crate::projects::Project =
                serde_json::from_value(v.get("project")?.clone()).ok()?;
            Some(ControlCommand::ProjectSave { project: proj })
        }
        "project.delete" => v
            .get("id")
            .and_then(|x| x.as_str())
            .map(|id| ControlCommand::ProjectDelete { id: id.to_string() }),
        "run.submit" => {
            let thread_doc_id = v
                .get("threadDocId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())?;
            let text = v
                .get("text")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let project_id = v
                .get("projectId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let resume_id = v
                .get("resumeId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            Some(ControlCommand::RunSubmit {
                thread_doc_id,
                text,
                project_id,
                resume_id,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_verbs() {
        assert!(matches!(
            parse_control_command("{\"control\":\"interrupt\"}"),
            Some(ControlCommand::Interrupt)
        ));
        assert!(matches!(
            parse_control_command("{\"control\":\"projects\"}"),
            Some(ControlCommand::Projects)
        ));
        assert!(matches!(
            parse_control_command("{\"control\":\"skills\"}"),
            Some(ControlCommand::Skills)
        ));
        assert!(matches!(
            parse_control_command("{\"control\":\"bridge.status\"}"),
            Some(ControlCommand::BridgeStatus)
        ));
        assert!(matches!(
            parse_control_command("{\"control\":\"convex.status\"}"),
            Some(ControlCommand::ConvexStatus)
        ));
    }

    #[test]
    fn parses_project_save_delete() {
        let save = parse_control_command(
            "{\"control\":\"project.save\",\"project\":{\"id\":\"p1\",\"name\":\"N\",\"workingDir\":\"/tmp\"}}",
        );
        match save {
            Some(ControlCommand::ProjectSave { project }) => {
                assert_eq!(project.id, "p1");
            }
            _ => panic!("bad parse"),
        }
        let del = parse_control_command("{\"control\":\"project.delete\",\"id\":\"p1\"}");
        assert!(matches!(del, Some(ControlCommand::ProjectDelete { id }) if id=="p1"));
    }

    #[test]
    fn parses_run_submit() {
        let s = parse_control_command(
            "{\"control\":\"run.submit\",\"threadDocId\":\"t1\",\"text\":\"hi\",\"projectId\":\"p\",\"resumeId\":\"last\"}",
        );
        match s {
            Some(ControlCommand::RunSubmit {
                thread_doc_id,
                text,
                project_id,
                resume_id,
            }) => {
                assert_eq!(thread_doc_id, "t1");
                assert_eq!(text, "hi");
                assert_eq!(project_id.as_deref(), Some("p"));
                assert_eq!(resume_id.as_deref(), Some("last"));
            }
            _ => panic!("bad parse"),
        }
    }

    #[test]
    fn rejects_non_json_or_multi_line() {
        assert!(parse_control_command("hello").is_none());
        let multi = "{\"control\":\"interrupt\"}\n{\"control\":\"projects\"}";
        assert!(parse_control_command(multi).is_none());
    }

    #[test]
    fn rejects_missing_control_field() {
        assert!(parse_control_command("{\"foo\":1}").is_none());
        assert!(parse_control_command("{} ").is_none());
    }

    #[test]
    fn rejects_malformed_run_submit() {
        // Missing threadDocId
        assert!(parse_control_command("{\"control\":\"run.submit\",\"text\":\"hi\"}").is_none());
        // Wrong types
        assert!(
            parse_control_command("{\"control\":\"run.submit\",\"threadDocId\":1,\"text\":true}")
                .is_none()
        );
    }

    #[test]
    fn parses_echo_synonyms() {
        let a = parse_control_command("{\"control\":\"echo\",\"payload\":\"x\",\"tag\":\"t\"}");
        let b = parse_control_command("{\"control\":\"debug.echo\"}");
        let c = parse_control_command("{\"control\":\"debug.ping\"}");
        match a { Some(ControlCommand::Echo{ payload, tag }) => { assert_eq!(payload.as_deref(), Some("x")); assert_eq!(tag.as_deref(), Some("t")); }, _ => panic!("bad echo parse") }
        assert!(matches!(b, Some(ControlCommand::Echo{..})));
        assert!(matches!(c, Some(ControlCommand::Echo{..})));
    }
}
