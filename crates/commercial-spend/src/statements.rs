//! Reviewed joined-read authority, independent of spending and payout authority.
use crate::{Controller, Error, Grant, Native, Result, now};
use pay_ledger::shared::{
    StatementActor,
    statement::{StatementQuery, StatementScope},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tenancy::{Accounts, MemberRef, Registry, Role};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementGrant {
    pub native_binding: String,
    pub native_member: MemberRef,
    pub canonical_member: MemberRef,
    pub sources: Vec<String>,
    pub full_customer: bool,
    /// Explicit historical reads preserve original records after attribution
    /// retirement; they still require current native and canonical rights.
    #[serde(default)]
    pub include_retired: bool,
    /// An explicit original account-to-payee read approval. Wallet possession
    /// and customer spending never establish this independent earnings right.
    pub payee: Option<String>,
    pub reviewed_at: u64,
    pub valid_until: u64,
}
impl Controller {
    fn statement_actor(&self, g: &Grant, actor: &StatementActor) -> Result<(MemberRef, Value)> {
        if actor.credential.is_empty() || actor.credential.len() > 8192 {
            return Err(Error::Denied);
        }
        let Native::Tenancy { directory, .. } = &g.native else {
            return Err(Error::Denied);
        };
        let workspace = g.binding.source.workspace.as_deref().ok_or(Error::Denied)?;
        let registry = Registry::open(directory).map_err(|_| Error::Denied)?;
        let accounts = Accounts::open(directory)?;
        accounts.read_locked::<_,Error>(|store| {
            let (member, principal)=if actor.credential.starts_with("sess_") {
                let sessions=tenancy::Sessions::open(directory).map_err(|_|Error::Denied)?;
                let book=sessions.store().map_err(|_|Error::Denied)?;
                let session=book.book.session_of_token(&actor.credential).ok_or(Error::Denied)?;
                if session.kind!=tenancy::SessionKind::User || session.standing(now())!=tenancy::SessionState::Active {return Err(Error::Denied);}
                (accounts.authorize(workspace,session.user.as_str()).map_err(|_|Error::Denied)?,format!("session:{}",session.id))
            }else{
                let key=tenancy::keys::authenticate(directory,registry.manifest(),&actor.credential).map_err(|_|Error::Denied)?;
                if key.scopes.as_ref().is_some_and(|s|!s.permits_action("accounts")){return Err(Error::Denied);}
                (accounts.authenticate_key(registry.manifest(),workspace,&actor.credential).map_err(|_|Error::Denied)?,format!("key:{}",key.key_id))
            };
            let proof=json!({"member":member,"principal":principal,"credential":crate::digest(actor.credential.as_bytes()),"revision":store.digest});
            Ok((member,proof))
        })
    }
    fn statement_scope(
        &self,
        g: &Grant,
        actor: &StatementActor,
    ) -> Result<(StatementScope, Option<String>, Vec<Value>)> {
        self.policy.bytes(256 * 1024)?;
        let (member, native_proof) = self.statement_actor(g, actor)?;
        let reviews = self
            .config
            .statements
            .iter()
            .filter(|r| r.native_binding == g.binding.id && r.native_member == member)
            .collect::<Vec<_>>();
        if reviews.len() != 1 {
            return Err(Error::Denied);
        }
        let review = reviews[0];
        if now() < review.reviewed_at
            || now() >= review.valid_until
            || review.sources.is_empty()
            || review.sources.len() > 128
            || !review.sources.iter().any(|id| id == &g.binding.id)
        {
            return Err(Error::Denied);
        }
        let canonical = Accounts::open(&self.config.canonical_directory)?;
        let canonical_member = canonical
            .authorize(
                &review.canonical_member.workspace,
                &review.canonical_member.account,
            )
            .map_err(|_| Error::Denied)?;
        if canonical_member != review.canonical_member
            || review.full_customer
                && (!matches!(member.role, Role::Owner | Role::Admin)
                    || !matches!(canonical_member.role, Role::Owner | Role::Admin))
        {
            return Err(Error::Denied);
        }
        if canonical_member.workspace != g.binding.commercial.workspace {
            return Err(Error::Denied);
        }
        let mut proofs = Vec::new();
        let mut attribution = Vec::new();
        for source in &review.sources {
            let source = self.grant(source)?;
            if source.binding.commercial.customer != g.binding.commercial.customer
                || source.binding.commercial.workspace != canonical_member.workspace
                || source.binding.pool != g.binding.pool
            {
                return Err(Error::Denied);
            }
            let native = self.current(source, false)?;
            let mapping_current = self.source_mapping_current(source)?;
            if !mapping_current && !review.include_retired {
                return Err(Error::Denied);
            }
            proofs.push(json!({"native":native,"current_attribution":mapping_current}));
            attribution.push(
                json!({"binding":source.binding.id,"current_original_mapping":mapping_current}),
            );
        }
        let authority = crate::digest(&serde_json::to_vec(&(
            review,
            native_proof,
            canonical_member,
            proofs,
        ))?);
        Ok((
            StatementScope {
                bindings: review.sources.clone(),
                authority,
                full_customer: review.full_customer,
                native_account: member.account,
                native_source: g.binding.source.clone(),
                native_origin: g.binding.native_origin.clone(),
            },
            review.payee.clone(),
            attribution,
        ))
    }
    pub(crate) fn joined_statement(
        &self,
        g: &Grant,
        actor: &StatementActor,
        query: &StatementQuery,
    ) -> Result<Value> {
        let (scope, payee, attribution) = self.statement_scope(g, actor)?;
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        let statement = ledger.joined_statement(&scope, query)?;
        let earnings = match &payee {
            Some(party) => Some(
                json!({"party":party,"unit":{"kind":"millisatoshis"},"statement":ledger.earnings_statement(party,query.after_earning.unwrap_or(0),query.after_payout.unwrap_or(0),query.limit.unwrap_or(50))?}),
            ),
            None if query.after_earning.is_some() || query.after_payout.is_some() => {
                return Err(Error::Denied);
            }
            None => None,
        };
        let (after, after_payee, after_attribution) = self.statement_scope(g, actor)?;
        if after.authority != scope.authority
            || after_payee != payee
            || after_attribution != attribution
        {
            return Err(Error::Denied);
        }
        let result = json!({"statement":statement,"payee":earnings,"source_attribution":attribution,"attribution_disclosure":"A false current_original_mapping marks a separately reviewed historical original source after retirement or supersession; it grants no new spend authority.","payee_disclosure":"Payee earnings remain separate and use their own next_earning and next_payout cursors."});
        if serde_json::to_vec(&result)?.len() > pay_ledger::shared::BODY_MAX - 2048 {
            return Err(Error::Denied);
        }
        Ok(result)
    }
}
