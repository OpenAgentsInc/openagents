//! Run controls received through the admitted device RPC adapter.
use super::*;
use openagents_sdk::runs::{CommandReceipt, RunCommand};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostCommand {
    account: String,
    run: String,
    session: Option<String>,
    request_id: String,
    command: openagents_sdk::runs::SessionCommand,
    agent: Option<String>,
}
impl EngineRpc {
    pub(super) async fn run_command(
        &self,
        params: serde_json::Value,
    ) -> Result<RpcReply, RpcError> {
        let command: HostCommand = parse_params(params)?;
        if command.account.is_empty()
            || command
                .agent
                .as_deref()
                .is_some_and(|a| !openagents_sdk::agents::valid_id(a))
        {
            return Err(RpcError::BadParams(
                "Check the command account and agent.".into(),
            ));
        }
        let request = RunCommand {
            request_id: command.request_id,
            command: command.command,
        };
        if !openagents_sdk::runs::valid_id(&command.run) || !request.valid() {
            return Err(RpcError::BadParams("Check the run command.".into()));
        }
        let receipt: CommandReceipt = self
            .doc_host
            .apply_run_command(
                &command.account,
                command.agent.as_deref(),
                &command.run,
                command.session.as_deref(),
                &request,
            )
            .await
            .map_err(|e| RpcError::Failed(e.to_string()))?;
        RpcReply::value(&receipt)
    }
}
