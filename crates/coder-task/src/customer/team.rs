//! Durable team intents use current account authority and private token custody.
use super::*;
use serde_json::json;
#[cfg(test)]
mod tests;

pub const INVITATION_SCHEMA: &str = "openagents.customer-invitation.v1";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TeamAction {
    Create {
        name: String,
        seats: u32,
    },
    Invite {
        workspace: String,
        role: jev::TeamRole,
        ttl_secs: u64,
    },
    Accept {
        workspace: String,
    },
    Withdraw {
        workspace: String,
        invitation: String,
    },
    Role {
        workspace: String,
        account: String,
        role: jev::TeamRole,
    },
    Remove {
        workspace: String,
        account: String,
    },
    Transfer {
        workspace: String,
        account: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamCommand {
    pub id: String,
    pub origin: String,
    pub account: String,
    pub credential_alias: String,
    pub action: TeamAction,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TeamStatus {
    Pending,
    Applied,
    Refused,
    Unknown,
}
#[derive(Clone, Debug, Serialize)]
pub struct TeamView {
    pub command: TeamCommand,
    pub status: TeamStatus,
    pub result: Option<Value>,
    pub refusal_status: Option<u16>,
    pub invitation_file: Option<PathBuf>,
    pub limitation: Option<&'static str>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operation {
    pub(super) command: TeamCommand,
    pub(super) status: TeamStatus,
    authority: String,
    input: String,
    result: Option<Value>,
    refusal_status: Option<u16>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    schema: String,
    origin: String,
    invitation: jev::TeamInvitation,
    token: String,
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
impl TeamCommand {
    fn validate(&self) -> Result<()> {
        if !alias(&self.id)
            || !alias(&self.credential_alias)
            || !identifier(&self.account)
            || origin(&self.origin)? != self.origin
        {
            return Err("Invalid team intent identity.".into());
        }
        if self.workspace().is_some_and(|v| !identifier(v)) {
            return Err("Invalid team workspace.".into());
        }
        match &self.action {
            TeamAction::Create { name, seats }
                if name.is_empty()
                    || name.len() > 256
                    || name.chars().any(char::is_control)
                    || *seats == 0 =>
            {
                return Err("Invalid team workspace input.".into());
            }
            TeamAction::Invite { ttl_secs, .. } if *ttl_secs == 0 || *ttl_secs > 2_592_000 => {
                return Err("Invalid invitation lifetime.".into());
            }
            TeamAction::Withdraw { invitation, .. } if !identifier(invitation) => {
                return Err("Invalid invitation reference.".into());
            }
            TeamAction::Role { account, .. }
            | TeamAction::Remove { account, .. }
            | TeamAction::Transfer { account, .. }
                if !identifier(account) =>
            {
                return Err("Invalid target member.".into());
            }
            _ => {}
        }
        Ok(())
    }
    fn workspace(&self) -> Option<&str> {
        match &self.action {
            TeamAction::Create { .. } => None,
            TeamAction::Invite { workspace, .. }
            | TeamAction::Accept { workspace }
            | TeamAction::Withdraw { workspace, .. }
            | TeamAction::Role { workspace, .. }
            | TeamAction::Remove { workspace, .. }
            | TeamAction::Transfer { workspace, .. } => Some(workspace),
        }
    }
}
pub(super) fn check(operations: &BTreeMap<String, Operation>) -> Result<()> {
    if operations.len() > 128 {
        return Err("Team intent history exceeds its bound.".into());
    }
    for (id, op) in operations {
        op.command.validate()?;
        if id != &op.command.id
            || [op.authority.as_str(), op.input.as_str()].iter().any(|h| {
                !h.strip_prefix("sha256:")
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            })
            || op
                .result
                .as_ref()
                .is_some_and(|v| v.to_string().len() > 16 * 1024)
            || (op.status == TeamStatus::Applied) != op.result.is_some()
            || (op.status == TeamStatus::Refused) != op.refusal_status.is_some()
            || op
                .refusal_status
                .is_some_and(|s| !matches!(s, 400 | 401 | 403 | 404 | 409 | 410 | 422))
        {
            return Err("Invalid retained team intent.".into());
        }
    }
    Ok(())
}
fn encode<T: Serialize>(value: T) -> jev::Result<Value> {
    serde_json::to_value(value).map_err(|_| jev::Error::Config("Invalid team result.".into()))
}
impl Store {
    fn team_view(&self, operation: &Operation) -> TeamView {
        let file = self
            .dir
            .join("invitations")
            .join(format!("{}.json", operation.command.id));
        TeamView {
            command: operation.command.clone(),
            status: operation.status,
            result: operation.result.clone(),
            refusal_status: operation.refusal_status,
            invitation_file: (matches!(operation.command.action, TeamAction::Invite { .. })
                && operation.status == TeamStatus::Applied)
                .then_some(file),
            limitation: matches!(operation.status, TeamStatus::Unknown | TeamStatus::Pending)
                .then_some("The team effect is unresolved. Do not repeat it with a new ID; inspect current membership and invitations before a separately reviewed action."),
        }
    }
    async fn team_authority(&self, command: &TeamCommand) -> Result<jev::Client> {
        let client = self.client(&command.origin, &command.credential_alias)?;
        let details = client
            .account()
            .details()
            .await
            .map_err(|_| "Current team account authentication is unavailable.")?;
        if details.account.id != command.account {
            return Err("Authenticated account differs from the team intent.".into());
        }
        Ok(client)
    }
    /// Recover only the original intent, under fresh account and workspace rights.
    pub async fn inspect_team(&self, id: &str) -> Result<TeamView> {
        let op = self
            .book
            .team
            .get(id)
            .ok_or("Team intent is unavailable.")?;
        let mut current = op.command.clone();
        if let Some(selected) = self
            .selected()
            .filter(|s| s.origin == current.origin && s.context.account == current.account)
        {
            current.credential_alias = selected.credential_alias.clone();
        }
        let client = self.team_authority(&current).await?;
        if let Some(workspace) = op.command.workspace() {
            client
                .account()
                .team(&op.command.account)
                .members(workspace)
                .await
                .map_err(|_| "Current team membership is unavailable.")?;
        }
        Ok(self.team_view(op))
    }
    /// Persist before a single remote attempt. Unknown intent blocks renamed retries.
    pub async fn change_team(
        &mut self,
        command: TeamCommand,
        invitation: Option<Vec<u8>>,
    ) -> Result<TeamView> {
        command.validate()?;
        let supplied = if matches!(command.action, TeamAction::Accept { .. }) {
            let bytes = invitation
                .as_ref()
                .ok_or("A private invitation file is required.")?;
            if bytes.len() > 8192 {
                return Err("Private invitation exceeds its bound.".into());
            }
            let value: Invitation = serde_json::from_slice(bytes)
                .map_err(|_| "Invalid private invitation document.")?;
            if value.schema != INVITATION_SCHEMA
                || value.origin != command.origin
                || Some(value.invitation.workspace.as_str()) != command.workspace()
                || !value
                    .token
                    .starts_with(&format!("inv_{}.", value.invitation.id))
                || value.token.len() > 256
            {
                return Err(
                    "Private invitation differs from the reviewed origin or workspace.".into(),
                );
            }
            Some((jev::ApiKey::new(value.token), value.invitation.role))
        } else {
            if invitation.is_some() {
                return Err("Invitation input is only valid for acceptance.".into());
            }
            None
        };
        let input = digest_request(
            &json!({"action":command.action,"invitation":supplied.as_ref().map(|(token, role)| json!({"token":digest_request(&json!(token.expose())),"role":role}))}),
        );
        let authority =
            digest_request(&json!(self.credential(&command.credential_alias)?.expose()));
        let client = self.team_authority(&command).await?;
        if let Some(old) = self.book.team.get(&command.id) {
            if old.command != command || old.input != input || old.authority != authority {
                return Err("Team intent ID already binds different input or authority.".into());
            }
            if let Some(workspace) = command.workspace() {
                client
                    .account()
                    .team(&command.account)
                    .members(workspace)
                    .await
                    .map_err(|_| "Current team membership is unavailable.")?;
            }
            return Ok(self.team_view(old));
        }
        if self.book.team.len() >= 128 {
            return Err("Team intent history is full.".into());
        }
        if !matches!(command.action, TeamAction::Accept { .. }) {
            if let Some(workspace) = command.workspace() {
                client
                    .account()
                    .team(&command.account)
                    .members(workspace)
                    .await
                    .map_err(|_| "Current team membership is unavailable.")?;
            }
        }
        if self.book.team.values().any(|old| {
            matches!(old.status, TeamStatus::Unknown | TeamStatus::Pending)
                && old.command.origin == command.origin
                && old.command.account == command.account
                && old.input == input
        }) {
            return Err(
                "An earlier matching team intent is unresolved; a new ID is not replay authority."
                    .into(),
            );
        }
        let mut next = self.book.clone();
        next.team.insert(
            command.id.clone(),
            Operation {
                command: command.clone(),
                status: TeamStatus::Pending,
                authority,
                input,
                result: None,
                refusal_status: None,
            },
        );
        self.persist(next)?;
        let result = self.team_effect(&command, &client, supplied.as_ref()).await;
        let mut next = self.book.clone();
        let op = next.team.get_mut(&command.id).unwrap();
        match result {
            Ok(value) => {
                op.status = TeamStatus::Applied;
                op.result = Some(value);
            }
            Err(jev::Error::Api(error))
                if matches!(error.status, 400 | 401 | 403 | 404 | 409 | 410 | 422) =>
            {
                op.status = TeamStatus::Refused;
                op.refusal_status = Some(error.status);
            }
            Err(_) => {
                op.status = TeamStatus::Unknown;
            }
        }
        self.persist(next)?;
        Ok(self.team_view(&self.book.team[&command.id]))
    }
    async fn team_effect(
        &mut self,
        command: &TeamCommand,
        client: &jev::Client,
        invitation: Option<&(jev::ApiKey, jev::TeamRole)>,
    ) -> jev::Result<Value> {
        let team = client.account().team(&command.account);
        match &command.action {
            TeamAction::Create { name, seats } => team.create(name, *seats).await,
            TeamAction::Invite {
                workspace,
                role,
                ttl_secs,
            } => {
                let grant = team.invite(workspace, *role, *ttl_secs).await?;
                let invitation = Invitation {
                    schema: INVITATION_SCHEMA.into(),
                    origin: command.origin.clone(),
                    invitation: grant.invitation.clone(),
                    token: grant.token.expose().into(),
                };
                let directory = self.dir.join("invitations");
                task::prepare_directory(&directory).map_err(|_| {
                    jev::Error::Config("Private invitation custody is unavailable.".into())
                })?;
                let bytes = serde_json::to_vec(&invitation)
                    .map_err(|_| jev::Error::Config("Invalid private invitation.".into()))?;
                let mut file =
                    task::private_open(&directory.join(format!("{}.json", command.id)), true, true)
                        .map_err(|_| {
                            jev::Error::Config("Invitation custody is uncertain.".into())
                        })?;
                file.write_all(&bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| jev::Error::Config("Invitation custody is uncertain.".into()))?;
                task::sync_directory(&directory)
                    .map_err(|_| jev::Error::Config("Invitation custody is uncertain.".into()))?;
                encode(grant.invitation)
            }
            TeamAction::Accept { workspace } => {
                let (token, role) = invitation.unwrap();
                encode(team.accept_reviewed(workspace, *role, token).await?)
            }
            TeamAction::Withdraw {
                workspace,
                invitation,
            } => team.withdraw(workspace, invitation).await,
            TeamAction::Role {
                workspace,
                account,
                role,
            } => encode(team.role(workspace, account, *role).await?),
            TeamAction::Remove { workspace, account } => {
                encode(team.remove(workspace, account).await?)
            }
            TeamAction::Transfer { workspace, account } => team.transfer(workspace, account).await,
        }
    }
    /// Switching changes only future selection, after current server admission.
    pub async fn switch_team(
        &mut self,
        workspace: &str,
        door: &str,
        credential: Option<&str>,
    ) -> Result<Selection> {
        let selected = self.selected().cloned().ok_or("Select a customer first.")?;
        self.select_account(
            &selected.origin,
            credential.unwrap_or(&selected.credential_alias),
            &selected.context.account,
            workspace,
            door,
        )
        .await
    }
    pub async fn team_members(&self, workspace: &str) -> Result<Value> {
        let selected = self.selected().ok_or("Select a customer first.")?;
        let client = self.client(&selected.origin, &selected.credential_alias)?;
        let members = client
            .account()
            .team(&selected.context.account)
            .members(workspace)
            .await
            .map_err(|_| "Current team membership is unavailable.")?;
        let purchase = client
            .account()
            .purchase_context(workspace, &selected.context.door)
            .await
            .ok();
        if purchase
            .as_ref()
            .is_some_and(|p| p.account != selected.context.account)
        {
            return Err("The current purchase account changed.".into());
        }
        Ok(
            json!({"team":members,"purchase_context":purchase,"purchase_available":purchase.is_some()}),
        )
    }
}
