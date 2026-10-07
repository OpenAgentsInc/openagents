//! Canonical money owns funds; the native journal retains prices, caps, and history.
use crate::{money, serve::ServeState};
use pay_ledger::shared::{
    Binding, Client, ClientConfig, Intent, Liability, Operation as Rpc, Outcome, ProjectionPage,
};
use receipts::shared_spend::Reference;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use tenancy::money::{Ledger, Mutation, Operation, budgets::Admission};
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub(crate) fn unsupported(state: &ServeState, caller: &crate::serve::Caller) -> bool {
    caller.workspace.as_ref().is_some_and(|workspace| {
        tenancy::money::shared::required(&state.config.registry, workspace)
            .map_or(true, |m| m.is_some())
    })
}
pub(crate) fn configure(directory: &Path, ledger: &mut Ledger) -> Result<(), String> {
    for workspace in ledger.shared_workspaces() {
        let path = directory.join("shared-spend").join(format!(
            "{:x}.client.json",
            Sha256::digest(workspace.as_bytes())
        ));
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(error)?;
        let meta = f.metadata().map_err(error)?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.mode() & 0o077 != 0
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.len() > 16384
        {
            return Err("Protected shared client configuration is required.".into());
        }
        let mut bytes = Vec::new();
        f.take(16385).read_to_end(&mut bytes).map_err(error)?;
        let config: ClientConfig = serde_json::from_slice(&bytes).map_err(error)?;
        let marker = tenancy::money::shared::required(directory, &workspace)?
            .ok_or("Retained shared custody is required.")?;
        if ledger.shared_mode(&workspace) != Some(&marker) {
            return Err("Native shared custody differs from its persistent marker.".into());
        }
        ledger.configure_shared(&workspace, config)?;
    }
    Ok(())
}
pub(crate) fn prepare_reserve(
    ledger: &mut Ledger,
    workspace: &str,
    request: &str,
    attempt: u32,
    terms: &str,
    priced: &money::Priced,
    binding: (&str, &str),
    budget: Option<Admission>,
    actor: pay_ledger::shared::GatewayActor,
    dispatched: &mut bool,
) -> Result<(money::Hold, tenancy::money::shared::Prepared), money::Refusal> {
    let key = format!("{request}#{attempt}");
    if ledger.has_attempt(&key) {
        return Err(money::Refusal::Duplicate);
    }
    let price = &priced.price;
    let offer = priced.offer.as_ref().ok_or_else(|| {
        money::Refusal::Price(
            "Shared Gateway spending requires a qualified native decision offer.".into(),
        )
    })?;
    offer.check(priced).map_err(money::Refusal::Price)?;
    if price.policy != money::POLICY || price.model != binding.0 || price.capacity != binding.1 {
        return Err(money::Refusal::Price(
            "Original native price differs from admission.".into(),
        ));
    }
    let maximum = ledger
        .shared_native_reservation(workspace, price, &priced.maximum_usage)
        .map_err(money::Refusal::Price)?;
    if let Some(b) = &budget {
        if let Some(blocked) = ledger
            .check_budget(workspace, b, maximum)
            .map_err(money::Refusal::BudgetUnavailable)?
        {
            return Err(money::Refusal::Budget(blocked));
        }
    }
    let client = ledger
        .shared_client(workspace)
        .ok_or_else(|| {
            money::Refusal::Ledger("Original shared controller configuration is required.".into())
        })?
        .clone();
    let current: Binding = serde_json::from_value(
        client
            .call(Rpc::Binding {})
            .map_err(|e| money::Refusal::Authorization(error(e)))?,
    )
    .map_err(|e| money::Refusal::Ledger(error(e)))?;
    if Some(&current.mode()) != ledger.shared_mode(workspace) {
        return Err(money::Refusal::Authorization(
            "Shared mapping changed; new spending requires reviewed activation.".into(),
        ));
    }
    let id = Intent::stable_id(&current, &key);
    let mut after = 0;
    let mut projection_head = None;
    loop {
        let page: ProjectionPage = serde_json::from_value(
            client
                .call(Rpc::SourceOutcomes {
                    after,
                    through: projection_head,
                })
                .map_err(|e| money::Refusal::Ledger(error(e)))?,
        )
        .map_err(|e| money::Refusal::Ledger(error(e)))?;
        if projection_head.is_some_and(|head| head != page.through) || page.entries.len() > 128 {
            return Err(money::Refusal::Ledger(
                "Original native projection snapshot changed.".into(),
            ));
        }
        projection_head = Some(page.through);
        if page
            .entries
            .iter()
            .any(|out| out.intent != id && !ledger.has_attempt(&out.native_attempt))
        {
            return Err(money::Refusal::Ledger("Original canonical liabilities require native projection before another reservation.".into()));
        }
        match page.next {
            Some(next) if next > after && next <= page.through => after = next,
            Some(_) => {
                return Err(money::Refusal::Ledger(
                    "Invalid native projection cursor.".into(),
                ));
            }
            None => break,
        }
    }
    let original = client
        .call(Rpc::Observe { id: id.clone() })
        .ok()
        .and_then(|v| serde_json::from_value::<Outcome>(v).ok());
    let intent = Intent {
        id,
        binding: current.clone(),
        native_attempt: key.clone(),
        quote: format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&(price, &priced.maximum_usage))
                    .map_err(|e| money::Refusal::Price(error(e)))?
            )
        ),
        execution: key.clone(),
        terms: terms.into(),
        maximum_units: maximum,
        fee_cap_msat: 0,
        invoice: None,
        liability: Liability::NativeService {
            resource: "openagents.gateway.systemone.v1".into(),
        },
        admitted_at: original
            .as_ref()
            .map_or(crate::accounts::unix_now(), |o| o.intent.admitted_at),
    };
    let reference = Reference {
        intent: intent.id.clone(),
        digest: intent.digest(),
        mode: current.mode(),
    };
    *dispatched = true; // A lost RPC reply retains liability; no second allocation or quota release.
    client
        .call(Rpc::Reserve {
            intent,
            projection_head: projection_head.unwrap_or(0),
            actor,
        })
        .map_err(|e| money::Refusal::Ledger(error(e)))?;
    let operation = if let Some(budget) = budget {
        Operation::ReserveScoped {
            attempt: key.clone(),
            request_digest: terms.into(),
            price: price.clone(),
            maximum_usage: priced.maximum_usage.clone(),
            budget,
        }
    } else {
        Operation::Reserve {
            attempt: key.clone(),
            request_digest: terms.into(),
            price: price.clone(),
            maximum_usage: priced.maximum_usage.clone(),
        }
    };
    let prepared = ledger
        .prepare_shared(Mutation {
            workspace: workspace.into(),
            source: format!("gateway:{key}:reserve"),
            audit: terms.into(),
            operation: Operation::SharedProject {
                reference,
                operation: Box::new(operation),
            },
        })
        .map_err(money::Refusal::Ledger)?;
    Ok((
        money::Hold {
            offer: priced.offer.clone(),
            workspace: workspace.into(),
            attempt: key,
            price: price.clone(),
        },
        prepared,
    ))
}
pub(crate) struct Effect {
    pub client: Client,
    pub reference: Reference,
    pub actor: pay_ledger::shared::GatewayActor,
}
pub(crate) fn effect(
    ledger: &Ledger,
    hold: &money::Hold,
    actor: pay_ledger::shared::GatewayActor,
) -> Option<Effect> {
    let (client, reference) = retained(ledger, hold)?;
    Some(Effect {
        client,
        reference,
        actor,
    })
}
fn retained(ledger: &Ledger, hold: &money::Hold) -> Option<(Client, Reference)> {
    let reference = ledger
        .hold(&hold.workspace, &hold.attempt)?
        .shared
        .clone()?;
    Some((ledger.shared_client(&hold.workspace)?.clone(), reference))
}
pub(crate) fn project(
    ledger: &mut Ledger,
    hold: &money::Hold,
    operation: Operation,
    source: &str,
    audit: &str,
) -> Result<(), String> {
    let Some((client, reference)) = retained(ledger, hold) else {
        return Err("Original shared controller is required.".into());
    };
    let rpc = match &operation {
        Operation::Settle { usage, .. } => Rpc::Settle {
            id: reference.intent.clone(),
            units: hold.price.quote(usage)?,
            evidence: audit.into(),
        },
        Operation::Release { .. } => Rpc::ReleaseUndispatched {
            id: reference.intent.clone(),
            evidence: audit.into(),
        },
        Operation::Unknown { .. } => Rpc::Unknown {
            id: reference.intent.clone(),
        },
        _ => return Err("Unsupported shared projection.".into()),
    };
    client.call(rpc).map_err(error)?;
    ledger.shared_project(&hold.workspace, source, audit, reference, operation)?;
    Ok(())
}
