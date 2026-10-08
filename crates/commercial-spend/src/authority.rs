//! Native spending permission is independent of canonical attribution.
use crate::{Controller, Error, Grant, Native, Result, digest, now};
use pay_ledger::compute::{Need, credential_digest};
use receipts::purchase::{CommercialProduct, CommercialRef};
use serde_json::{Value, json};
use tenancy::{
    Accounts, Registry,
    accounts::commercial::{Product, Source},
};
impl Controller {
    pub(crate) fn gateway_actor(
        &self,
        grant: &Grant,
        actor: &pay_ledger::shared::GatewayActor,
    ) -> Result<Value> {
        let Native::Tenancy { directory, .. } = &grant.native else {
            return Err(Error::Denied);
        };
        if actor.credential.len() > 8192
            || actor.door.is_empty()
            || actor.door.len() > 256
            || grant.binding.source.product != CommercialProduct::Gateway
        {
            return Err(Error::Denied);
        }
        let workspace = grant
            .binding
            .source
            .workspace
            .as_deref()
            .ok_or(Error::Denied)?;
        let registry = Registry::open(directory).map_err(|_| Error::Denied)?;
        let accounts = Accounts::open(directory)?;
        accounts.read_locked::<_,Error>(|store|{
            let (member,principal)=if actor.credential.starts_with("sess_") {
                let sessions=tenancy::Sessions::open(directory).map_err(|_|Error::Denied)?;
                let book=sessions.store().map_err(|_|Error::Denied)?;
                let session=book.book.session_of_token(&actor.credential).ok_or(Error::Denied)?;
                if session.kind!=tenancy::SessionKind::User || session.standing(now())!=tenancy::SessionState::Active {return Err(Error::Denied);}
                (accounts.authorize(workspace,session.user.as_str()).map_err(|_|Error::Denied)?,format!("session:{}",session.id))
            }else{
                let key=tenancy::keys::authenticate(directory,registry.manifest(),&actor.credential).map_err(|_|Error::Denied)?;
                if key.scopes.as_ref().is_some_and(|s| !s.permits_action("inference") || !s.permits_action("shared-spend") || !s.permits_model(&actor.door)) {return Err(Error::Denied);}
                (accounts.authenticate_key(registry.manifest(),workspace,&actor.credential).map_err(|_|Error::Denied)?,format!("key:{}",key.key_id))
            };
            if member.account!=grant.binding.source.account {return Err(Error::Denied);}
            Ok(json!({"credential":digest(actor.credential.as_bytes()),"principal":principal,"account":member.account,"workspace":member.workspace,"epoch":member.epoch,"members_epoch":member.members_epoch,"revision":store.digest,"door":actor.door}))
        })
    }
    pub(crate) fn source_mapping_current(&self, grant: &Grant) -> Result<bool> {
        let s = &grant.binding.source;
        let source = Source {
            product: match s.product {
                CommercialProduct::Gateway => Product::Gateway,
                CommercialProduct::Plugin => Product::Plugin,
                CommercialProduct::Retail => Product::Retail,
            },
            issuer: s.issuer.clone(),
            account: s.account.clone(),
            workspace: s.workspace.clone(),
        };
        let Some(r) = self
            .commercial
            .selection(&source)
            .map_err(|_| Error::Denied)?
        else {
            return Ok(false);
        };
        let reference = CommercialRef {
            binding: r.binding,
            revision: r.revision,
            digest: r.digest,
            customer: r.customer,
            workspace: r.workspace,
            source: s.clone(),
        };
        Ok(reference == grant.binding.commercial)
    }
    pub(crate) fn current(&self, grant: &Grant, spend: bool) -> Result<Value> {
        self.current_authority(grant, spend, spend)
    }
    /// Original financial cleanup keeps native spend permission and protected
    /// review; retiring canonical linkage cannot erase the original obligation.
    pub(crate) fn current_cleanup(&self, grant: &Grant) -> Result<Value> {
        self.current_authority(grant, true, false)
    }
    fn current_authority(&self, grant: &Grant, spend: bool, mapping: bool) -> Result<Value> {
        self.policy.bytes(256 * 1024)?;
        self.ledger_custody.check()?;
        if spend && (now() < grant.reviewed_at || now() >= grant.valid_until) {
            return Err(Error::Denied);
        }
        let credential = self
            .native_credentials
            .get(&grant.binding.id)
            .ok_or(Error::Denied)?;
        let token = credential.token()?;
        let proof = match &grant.native {
            Native::Retail {
                principal,
                generation,
            } => {
                if grant.binding.source.product != CommercialProduct::Retail
                    || grant.binding.native_origin != self.config.origin
                {
                    return Err(Error::Denied);
                }
                let book = pay_ledger::Ledger::open_read_only(&self.config.ledger)?;
                let native = book.resolve_principal(
                    principal,
                    &credential_digest(&token),
                    if spend { Need::Spend } else { Need::Read },
                )?;
                if native.account != grant.binding.source.account
                    || native.generation != *generation
                {
                    return Err(Error::Denied);
                }
                json!({"kind":"retail","origin":book.origin()?,"principal":native.id,"generation":native.generation,"account":native.account,"spend":spend})
            }
            Native::Tenancy {
                directory,
                principal,
                member_epoch,
                members_epoch,
                tenant,
                ..
            } => {
                if grant.binding.source.product == CommercialProduct::Retail {
                    return Err(Error::Denied);
                }
                let held = self
                    .native_books
                    .get(&grant.binding.id)
                    .ok_or(Error::Denied)?;
                held.check()?;
                let registry = Registry::open(directory).map_err(|_| Error::Denied)?;
                let accounts = Accounts::open(directory).map_err(|_| Error::Denied)?;
                accounts.read_locked::<_,Error>(|store|{
     let workspace=grant.binding.source.workspace.as_deref().ok_or(Error::Denied)?;
     let key=tenancy::keys::authenticate(directory,registry.manifest(),&token).map_err(|_|Error::Denied)?;
     let member=accounts.authenticate_key(registry.manifest(),workspace,&token).map_err(|_|Error::Denied)?;
     if key.tenant!=*tenant||format!("key:{}",key.key_id)!=*principal||member.account!=grant.binding.source.account||member.epoch!=*member_epoch||member.members_epoch!=*members_epoch|| key.scopes.as_ref().is_some_and(|s|!s.permits_action("accounts")||(spend&&!s.permits_action("shared-spend"))){return Err(Error::Denied);}
     let origin=digest(&serde_json::to_vec(&("tenancy",tenant,&member.account,&member.workspace))?);
     if origin!=grant.binding.native_origin{return Err(Error::Denied);}
     Ok(json!({"kind":"tenancy","origin":origin,"principal":principal,"tenant":tenant,"member_epoch":member.epoch,"members_epoch":member.members_epoch,"revision":store.digest,"account":member.account,"workspace":member.workspace,"spend":spend}))
    })?
            }
        };
        credential.check()?;
        if mapping {
            let s = &grant.binding.source;
            let source = Source {
                product: match s.product {
                    CommercialProduct::Gateway => Product::Gateway,
                    CommercialProduct::Plugin => Product::Plugin,
                    CommercialProduct::Retail => Product::Retail,
                },
                issuer: s.issuer.clone(),
                account: s.account.clone(),
                workspace: s.workspace.clone(),
            };
            let revision = self
                .commercial
                .selection(&source)
                .map_err(|_| Error::Denied)?
                .ok_or(Error::Denied)?;
            let current = CommercialRef {
                binding: revision.binding,
                revision: revision.revision,
                digest: revision.digest,
                customer: revision.customer,
                workspace: revision.workspace,
                source: s.clone(),
            };
            if current != grant.binding.commercial {
                return Err(Error::Denied);
            }
        }
        self.policy.bytes(256 * 1024)?;
        self.ledger_custody.check()?;
        Ok(proof)
    }
}
