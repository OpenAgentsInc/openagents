use std::path::PathBuf;

use codex_client::{
    AppServerClient, ClientInfo, InitializeCapabilities, InitializeParams, ModelListParams,
    SkillScope, SkillsConfigWriteParams, SkillsListExtraRootsForCwd, SkillsListParams,
    ThreadListParams, ThreadSortKey, ThreadSourceKind, TurnStartParams, UserInput,
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
fn initialize_params_serialize_capabilities() -> anyhow::Result<()> {
    let params = InitializeParams {
        client_info: ClientInfo {
            name: "openagents".to_string(),
            title: Some("OpenAgents".to_string()),
            version: "0.1.0".to_string(),
        },
        capabilities: Some(InitializeCapabilities {
            experimental_api: true,
            opt_out_notification_methods: Some(vec![
                "item/agentMessage/delta".to_string(),
                "item/reasoning/textDelta".to_string(),
            ]),
        }),
    };

    let value = serde_json::to_value(params)?;
    assert_eq!(value["capabilities"]["experimentalApi"], Value::Bool(true));
    assert_eq!(
        value["capabilities"]["optOutNotificationMethods"],
        json!(["item/agentMessage/delta", "item/reasoning/textDelta"])
    );
    Ok(())
}

#[test]
fn thread_list_and_model_list_params_include_new_filters() -> anyhow::Result<()> {
    let thread_list = ThreadListParams {
        cursor: Some("cursor-1".to_string()),
        limit: Some(25),
        sort_key: Some(ThreadSortKey::UpdatedAt),
        model_providers: Some(vec!["openai".to_string()]),
        source_kinds: Some(vec![ThreadSourceKind::AppServer]),
        archived: Some(true),
        cwd: Some("/repo".to_string()),
        search_term: Some("refactor".to_string()),
    };
    let thread_value = serde_json::to_value(thread_list)?;
    assert_eq!(
        thread_value["sortKey"],
        Value::String("updated_at".to_string())
    );
    assert_eq!(thread_value["sourceKinds"], json!(["appServer"]));
    assert_eq!(thread_value["archived"], Value::Bool(true));
    assert_eq!(
        thread_value["searchTerm"],
        Value::String("refactor".to_string())
    );

    let model_list = ModelListParams {
        cursor: Some("cursor-2".to_string()),
        limit: Some(10),
        include_hidden: Some(true),
    };
    let model_value = serde_json::to_value(model_list)?;
    assert_eq!(model_value["includeHidden"], Value::Bool(true));
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

#[test]
fn skills_list_response_deserializes_errors_and_enabled_state() -> anyhow::Result<()> {
    let response_json = json!({
        "data": [
            {
                "cwd": "/repo",
                "skills": [
                    {
                        "name": "mezo",
                        "description": "Integrate with Mezo",
                        "interface": {
                            "displayName": "Mezo"
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
                    },
                    {
                        "name": "moneydevkit",
                        "description": "Integrate with MoneyDevKit",
                        "path": "/repo/skills/moneydevkit/SKILL.md",
                        "scope": "repo",
                        "enabled": false
                    }
                ],
                "errors": [
                    {
                        "path": "/repo/skills/bad",
                        "message": "invalid SKILL.md"
                    }
                ]
            }
        ]
    });

    let response: codex_client::SkillsListResponse = serde_json::from_value(response_json)?;
    assert_eq!(response.data.len(), 1);

    let entry = &response.data[0];
    assert_eq!(entry.cwd, PathBuf::from("/repo"));
    assert_eq!(entry.skills.len(), 2);
    assert_eq!(entry.errors.len(), 1);
    assert_eq!(entry.errors[0].path, PathBuf::from("/repo/skills/bad"));
    assert_eq!(entry.errors[0].message, "invalid SKILL.md");

    let first = &entry.skills[0];
    assert!(first.enabled);
    assert_eq!(first.scope, SkillScope::Repo);
    assert_eq!(
        first
            .interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref()),
        Some("Mezo")
    );
    assert_eq!(
        first
            .dependencies
            .as_ref()
            .map(|dependencies| dependencies.tools.len()),
        Some(1)
    );

    let second = &entry.skills[1];
    assert!(!second.enabled);
    assert_eq!(second.scope, SkillScope::Repo);

    Ok(())
}

#[test]
fn thread_list_response_deserializes_structured_status_and_paths() -> anyhow::Result<()> {
    let response_json = json!({
        "data": [
            {
                "id": "019ca108-a6c0-75b3-b11b-c4f3f0e67a9e",
                "preview": "hello",
                "modelProvider": "openai",
                "cwd": "/Users/christopherdavid/code/openagents",
                "path": "/Users/christopherdavid/.codex/sessions/2026/02/27/019ca108-a6c0-75b3-b11b-c4f3f0e67a9e.jsonl",
                "createdAt": 1761930000,
                "updatedAt": 1761930123,
                "status": {
                    "type": "active",
                    "activeFlags": ["waitingOnUserInput"]
                }
            }
        ],
        "nextCursor": null
    });

    let response: codex_client::ThreadListResponse = serde_json::from_value(response_json)?;
    assert_eq!(response.data.len(), 1);
    let thread = &response.data[0];
    assert_eq!(thread.id, "019ca108-a6c0-75b3-b11b-c4f3f0e67a9e");
    assert_eq!(
        thread.cwd.as_ref().map(|cwd| cwd.display().to_string()),
        Some("/Users/christopherdavid/code/openagents".to_string())
    );
    assert_eq!(
        thread.path.as_ref().map(|path| path.display().to_string()),
        Some(
            "/Users/christopherdavid/.codex/sessions/2026/02/27/019ca108-a6c0-75b3-b11b-c4f3f0e67a9e.jsonl"
                .to_string()
        )
    );
    assert!(thread.status.is_some());

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
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "mezo".to_string(),
                    path: PathBuf::from("/repo/skills/mezo/SKILL.md"),
                },
            ],
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
            personality: None,
            output_schema: None,
            collaboration_mode: None,
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
