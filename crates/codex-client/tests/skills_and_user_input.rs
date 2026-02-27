use std::path::PathBuf;

use codex_client::{
    AppServerClient, SkillScope, SkillsConfigWriteParams, SkillsListExtraRootsForCwd,
    SkillsListParams, TurnStartParams, UserInput,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn user_input_skill_and_mention_round_trip() -> anyhow::Result<()> {
    let skill_json = json!({
        "type": "skill",
        "name": "mezo",
        "path": "/repo/skills/mezo/SKILL.md"
    });
    let mention_json = json!({
        "type": "mention",
        "name": "config.toml",
        "path": "./config.toml"
    });

    let skill: UserInput = serde_json::from_value(skill_json)?;
    let mention: UserInput = serde_json::from_value(mention_json)?;

    match skill {
        UserInput::Skill { name, path } => {
            assert_eq!(name, "mezo");
            assert_eq!(path, PathBuf::from("/repo/skills/mezo/SKILL.md"));
        }
        _ => return Err(anyhow::anyhow!("expected skill variant")),
    }

    match mention {
        UserInput::Mention { name, path } => {
            assert_eq!(name, "config.toml");
            assert_eq!(path, "./config.toml");
        }
        _ => return Err(anyhow::anyhow!("expected mention variant")),
    }

    Ok(())
}

#[test]
fn skills_list_params_serialize_extra_roots() -> anyhow::Result<()> {
    let params = SkillsListParams {
        cwds: vec![PathBuf::from("/repo")],
        force_reload: true,
        per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
            cwd: PathBuf::from("/repo"),
            extra_user_roots: vec![PathBuf::from("/repo/skills")],
        }]),
    };

    let value = serde_json::to_value(params)?;

    assert_eq!(value["forceReload"], Value::Bool(true));
    assert_eq!(value["cwds"], json!(["/repo"]));
    assert_eq!(
        value["perCwdExtraUserRoots"],
        json!([
            {
                "cwd": "/repo",
                "extraUserRoots": ["/repo/skills"]
            }
        ])
    );

    Ok(())
}

#[test]
fn skill_metadata_deserializes_enabled_interface_dependencies() -> anyhow::Result<()> {
    let metadata_json = json!({
        "name": "mezo",
        "description": "Integrate with Mezo",
        "interface": {
            "displayName": "Mezo",
            "shortDescription": "Use Mezo SDK"
        },
        "dependencies": {
            "tools": [
                {
                    "type": "cmd",
                    "value": "cargo"
                }
            ]
        },
        "path": "/repo/skills/mezo/SKILL.md",
        "scope": "repo",
        "enabled": true
    });

    let skill: codex_client::SkillMetadata = serde_json::from_value(metadata_json)?;
    assert_eq!(skill.name, "mezo");
    assert_eq!(skill.scope, SkillScope::Repo);
    assert!(skill.enabled);

    let interface = match skill.interface {
        Some(interface) => interface,
        None => return Err(anyhow::anyhow!("missing interface")),
    };
    assert_eq!(interface.display_name.as_deref(), Some("Mezo"));

    let dependencies = match skill.dependencies {
        Some(deps) => deps,
        None => return Err(anyhow::anyhow!("missing dependencies")),
    };
    assert_eq!(dependencies.tools.len(), 1);
    assert_eq!(dependencies.tools[0].r#type, "cmd");

    Ok(())
}

#[tokio::test]
async fn turn_start_request_includes_skill_input() -> anyhow::Result<()> {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);

    let (client, _channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);

    let server = tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let mut request_line = String::new();
        let bytes = reader.read_line(&mut request_line).await?;
        if bytes == 0 {
            return Err(anyhow::anyhow!("missing request"));
        }

        let request_json: Value = serde_json::from_str(request_line.trim())?;
        assert_eq!(
            request_json["method"],
            Value::String("turn/start".to_string())
        );

        let params = &request_json["params"];
        assert_eq!(params["threadId"], Value::String("thread-1".to_string()));
        assert_eq!(
            params["input"],
            json!([
                {"type": "text", "text": "run integration"},
                {
                    "type": "skill",
                    "name": "mezo",
                    "path": "/repo/skills/mezo/SKILL.md"
                }
            ])
        );

        let response = json!({
            "id": request_json["id"].clone(),
            "result": {
                "turn": {"id": "turn-1"}
            }
        });
        let line = format!("{}\n", serde_json::to_string(&response)?);
        server_write.write_all(line.as_bytes()).await?;
        server_write.flush().await?;

        Ok::<(), anyhow::Error>(())
    });

    let response = client
        .turn_start(TurnStartParams {
            thread_id: "thread-1".to_string(),
            input: vec![
                UserInput::Text {
                    text: "run integration".to_string(),
                },
                UserInput::Skill {
                    name: "mezo".to_string(),
                    path: PathBuf::from("/repo/skills/mezo/SKILL.md"),
                },
            ],
            model: None,
            effort: None,
            summary: None,
            approval_policy: None,
            sandbox_policy: None,
            cwd: None,
        })
        .await?;

    assert_eq!(response.turn.id, "turn-1");
    server.await??;
    Ok(())
}

#[tokio::test]
async fn skills_config_write_request_round_trip() -> anyhow::Result<()> {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);

    let (client, _channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);

    let server = tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let mut request_line = String::new();
        let bytes = reader.read_line(&mut request_line).await?;
        if bytes == 0 {
            return Err(anyhow::anyhow!("missing request"));
        }

        let request_json: Value = serde_json::from_str(request_line.trim())?;
        assert_eq!(
            request_json["method"],
            Value::String("skills/config/write".to_string())
        );
        assert_eq!(
            request_json["params"],
            json!({
                "path": "/repo/skills/mezo/SKILL.md",
                "enabled": false
            })
        );

        let response = json!({
            "id": request_json["id"].clone(),
            "result": {
                "effectiveEnabled": false
            }
        });
        let line = format!("{}\n", serde_json::to_string(&response)?);
        server_write.write_all(line.as_bytes()).await?;
        server_write.flush().await?;

        Ok::<(), anyhow::Error>(())
    });

    let response = client
        .skills_config_write(SkillsConfigWriteParams {
            path: PathBuf::from("/repo/skills/mezo/SKILL.md"),
            enabled: false,
        })
        .await?;

    assert!(!response.effective_enabled);
    server.await??;

    Ok(())
}
