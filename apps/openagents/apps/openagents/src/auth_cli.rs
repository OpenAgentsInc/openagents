//! Standalone OpenAgents sign-in, sign-out, and account status.
//! Commands share the desktop account controller and take the engine data lock
//! before changing its credentials. Normal logins use the platform store;
//! explicit launch credentials stay in memory.

use std::io::IsTerminal;

use openagents_engine::{AuthState, Engine, EngineConfig, InstanceLock, WorkspaceScope};

#[derive(Debug, PartialEq, Eq)]
struct AccountStatus {
    mode: &'static str,
    auth: String,
    healthy: bool,
}

fn account_status(scope: WorkspaceScope, auth: &AuthState) -> AccountStatus {
    match scope {
        WorkspaceScope::Local => match auth {
            AuthState::SignedOut => AccountStatus {
                mode: "local only",
                auth: "signed out (optional in local-only mode)".into(),
                healthy: true,
            },
            AuthState::NeedsOrganization { user } => AccountStatus {
                mode: "local only",
                auth: format!(
                    "signed in as {}; finish workspace setup to enable sync",
                    user.email
                ),
                healthy: true,
            },
            AuthState::SignedIn { user, .. } => AccountStatus {
                mode: "local only",
                auth: format!("signed in as {}; sync is ready after restart", user.email),
                healthy: true,
            },
        },
        WorkspaceScope::Development => AccountStatus {
            mode: "development",
            auth: "dev mode (bearer = user id)".into(),
            healthy: true,
        },
        WorkspaceScope::Synced => match auth {
            AuthState::SignedIn { user, org_id } => AccountStatus {
                mode: "synced",
                auth: format!(
                    "signed in as {}{}",
                    user.email,
                    org_id
                        .as_ref()
                        .map(|org| format!(" (workspace {org})"))
                        .unwrap_or_default()
                ),
                healthy: true,
            },
            AuthState::NeedsOrganization { user } => AccountStatus {
                mode: "synced",
                auth: format!(
                    "signed in as {} but no workspace selected — run `openagents login`",
                    user.email
                ),
                healthy: false,
            },
            AuthState::SignedOut => AccountStatus {
                mode: "synced",
                auth: "saved session is no longer valid — run `openagents login`".into(),
                healthy: false,
            },
        },
    }
}

/// Authenticate through the shared OpenAgents device flow and platform store.
pub async fn login(config: EngineConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;
    let _lock = engine_lock(&config, "sign in")?;
    let (controller, credential) = openagents_account::desktop::configured(&config.data_dir)?;
    let temporary = credential.is_some();
    if let Some(credential) = credential {
        controller.import_credential(credential).await;
    } else {
        controller.restore().await;
        if controller.lease().is_none() {
            if !std::io::stdin().is_terminal() {
                anyhow::bail!(
                    "Run login in a terminal, or provide an OpenAgents credential through CODER_DESKTOP_CREDENTIAL."
                );
            }
            let mut updates = controller.watch();
            let operation = controller.sign_in("OpenAgents".into());
            tokio::pin!(operation);
            loop {
                tokio::select! {
                    _ = &mut operation => break,
                    changed = updates.changed() => {
                        if changed.is_err() { break; }
                        if let openagents_account::Phase::Waiting { code, url } = &updates.borrow_and_update().phase {
                            println!("Open {url} and approve code {code} for OpenAgents.");
                        }
                    }
                }
            }
        }
    }
    match controller.snapshot().phase {
        openagents_account::Phase::SignedIn { account } => {
            println!("Signed in as {}.", account.label);
            if temporary {
                println!(
                    "Keep CODER_DESKTOP_CREDENTIAL set when starting OpenAgents; this credential was not saved."
                );
            } else {
                println!("Start OpenAgents to sync new sessions; local sessions stay local.");
            }
        }
        openagents_account::Phase::Failed { message, .. } => anyhow::bail!("{message}"),
        _ => anyhow::bail!("Sign-in did not complete."),
    }
    Ok(())
}

/// Restore a shared app account for a headless runtime. Explicit dev runs stay separate.
pub async fn openagents_account(
    config: &EngineConfig,
) -> anyhow::Result<Option<std::sync::Arc<openagents_account::Controller>>> {
    if config.edge_token.is_some()
        || config.org_id.is_some()
        || openagents_brand::env_var("OPENAGENTS_WORKSPACE_SCOPE").is_ok_and(|scope| scope == "development")
    {
        return Ok(None);
    }
    let (controller, credential) = openagents_account::desktop::configured(&config.data_dir)?;
    let explicit = credential.is_some();
    if let Some(credential) = credential {
        controller.import_credential(credential).await;
    } else {
        controller.restore().await;
    }
    if explicit && controller.lease().is_none() {
        let message = match controller.snapshot().phase {
            openagents_account::Phase::Failed { message, .. } => message,
            _ => "Couldn't sign in. Check the OpenAgents credential.".into(),
        };
        anyhow::bail!("{message}");
    }
    Ok(Some(controller))
}

/// Sign out of the shared OpenAgents account and clear its platform credential.
pub async fn logout(config: EngineConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;
    let _lock = engine_lock(&config, "sign out")?;
    let (controller, _) = openagents_account::desktop::configured(&config.data_dir)?;
    controller.sign_out().await;
    println!("Signed out. The next engine start opens your local workspace.");
    Ok(())
}

/// `openagents status`: report the fixed scope a new engine would select, optional
/// auth, and engine liveness. Local-only is a healthy signed-out state.
pub async fn status(config: EngineConfig) -> anyhow::Result<()> {
    let _account = openagents_account(&config).await?;
    let auth = Engine::build_auth(&config).await;
    let next_scope = Engine::initial_workspace_scope(&auth);
    let scope = live_engine_scope(config.ipc_port)
        .await
        .unwrap_or(next_scope);
    let account = account_status(scope, &auth.state());
    println!("Data dir: {}", config.data_dir.display());
    println!("Edge:     {}", config.edge_url);
    println!("Mode:     {}", account.mode);
    println!("Auth:     {}", account.auth);
    match InstanceLock::holder(&config.data_dir) {
        Some(pid) => println!("Engine:   running (pid {pid})"),
        None => println!("Engine:   not running"),
    }
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], config.ipc_port));
    let ipc = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500));
    println!(
        "IPC:      {} 127.0.0.1:{}",
        if ipc.is_ok() {
            "listening on"
        } else {
            "not listening on"
        },
        config.ipc_port
    );
    if !account.healthy {
        std::process::exit(1);
    }
    Ok(())
}

/// Prefer the immutable scope of a live runtime. Falling back to the next-boot
/// derivation is correct when no engine is listening and tolerant of old
/// daemons that predate EngineInfo.
async fn live_engine_scope(ipc_port: u16) -> Option<WorkspaceScope> {
    let client = openagents_rpc::connect_ws(&format!("ws://127.0.0.1:{ipc_port}"))
        .await
        .ok()?;
    let value = client
        .call(openagents_rpc::methods::ENGINE_INFO, serde_json::json!({}))
        .await
        .ok()?;
    serde_json::from_value::<openagents_engine::EngineInfo>(value)
        .ok()
        .map(|info| info.workspace_scope)
}

/// The same exclusive data-dir lock the engine holds for its lifetime: taken for
/// the whole login/logout mutation so we never rotate or delete a session out
/// from under a running engine (whose in-memory copy would fight back — the next
/// token refresh re-persists it).
fn engine_lock(config: &EngineConfig, verb: &str) -> anyhow::Result<InstanceLock> {
    InstanceLock::acquire(&config.data_dir).map_err(|err| {
        anyhow::anyhow!(
            "{err}\nCannot {verb} while an engine is running — stop it first \
             (`openagents daemon stop`, or quit the OpenAgents app), or use the running UI instead."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_engine::{AuthUser, HarnessId};

    fn config(data_dir: &std::path::Path) -> EngineConfig {
        EngineConfig {
            data_dir: data_dir.to_path_buf(),
            edge_url: "http://127.0.0.1:1".into(),
            edge_token: None,
            ipc_port: 0,
            default_harness: HarnessId::Mock,
            device_origin: openagents_engine::DeviceOrigin::Unknown,
            org_id: None,
            workos_client_id: Some("client_test".into()),
        }
    }

    #[test]
    fn signed_out_local_status_is_healthy() {
        let status = account_status(WorkspaceScope::Local, &AuthState::SignedOut);
        assert_eq!(status.mode, "local only");
        assert_eq!(status.auth, "signed out (optional in local-only mode)");
        assert!(status.healthy);
    }

    #[test]
    fn synced_status_still_requires_a_complete_account() {
        let user = AuthUser {
            id: "user-1".into(),
            email: "user@example.com".into(),
            name: None,
        };
        assert!(
            !account_status(
                WorkspaceScope::Synced,
                &AuthState::NeedsOrganization { user: user.clone() },
            )
            .healthy
        );
        assert!(
            account_status(
                WorkspaceScope::Synced,
                &AuthState::SignedIn {
                    user,
                    org_id: Some("org-1".into()),
                },
            )
            .healthy
        );
    }

    #[cfg(unix)]
    #[test]
    fn login_and_logout_lock_out_a_running_engine() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let _engine = InstanceLock::acquire(dir.path()).unwrap();

        for verb in ["sign in", "sign out"] {
            let error = engine_lock(&config, verb).unwrap_err().to_string();
            assert!(error.contains("while an engine is running"));
            assert!(error.contains("stop it first"));
        }
    }
}
