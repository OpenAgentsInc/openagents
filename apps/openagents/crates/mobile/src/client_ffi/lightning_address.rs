//! The account's Lightning addresses as a shared Rust Native wallet sheet.
use super::{CoreAccount, CoreError, CoreResult};
use crate::wallet::{Action, CoreWallet};
use openagents_account::{
    Controller,
    openagents::lightning_address::{AddressState, LightningAddress},
};
use rust_native::{Activation, Axis, Element, Node, TextRole, View};
use std::sync::{Arc, Mutex};

fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}
const UNREACHABLE: &str = "Your Lightning address couldn't be reached. Try again.";
#[derive(Default)]
struct State {
    revision: u64,
    generation: Option<u64>,
    busy: bool,
    loaded: bool,
    configured: bool,
    name: Option<String>,
    records: Vec<LightningAddress>,
    notice: Option<String>,
}
#[derive(uniffi::Object)]
pub struct CoreLightningAddress {
    controller: Arc<Controller>,
    wallet: Arc<CoreWallet>,
    instance: String,
    state: Arc<Mutex<State>>,
}

#[uniffi::export]
impl CoreAccount {
    pub fn lightning_address(
        &self,
        wallet: Arc<CoreWallet>,
    ) -> CoreResult<Arc<CoreLightningAddress>> {
        wallet.open();
        Ok(Arc::new(CoreLightningAddress {
            controller: self.controller.clone(),
            wallet,
            instance: format!("lightning-address-{}", uuid::Uuid::new_v4()),
            state: Arc::new(Mutex::new(State {
                revision: 1,
                ..State::default()
            })),
        }))
    }
}
impl CoreLightningAddress {
    fn view(&self) -> View<Action> {
        let state = self.state.lock().unwrap();
        let current = self
            .controller
            .lease()
            .is_some_and(|lease| Some(lease.generation) == state.generation);
        address_view(
            &self.instance,
            &state,
            current,
            self.wallet.node().is_some(),
        )
    }
}
fn address_view(instance: &str, state: &State, current: bool, wallet_ready: bool) -> View<Action> {
    let mut nodes = vec![text("heading", "Lightning address", TextRole::Heading)];
    if !current || !state.loaded {
        nodes.push(text("loading", "Loading your address…", TextRole::Body));
    } else {
        for record in &state.records {
            let address = format!("{}@{}", record.name, record.domain);
            nodes.push(text(
                &format!("address-{}", record.name),
                if record.state == AddressState::Active {
                    &address
                } else {
                    &record.name
                },
                TextRole::Body,
            ));
            if record.state == AddressState::Active {
                nodes.push(button(
                    &format!("copy:{}", record.name),
                    "Copy address",
                    !state.busy,
                ));
                nodes.push(button(
                    &format!("share:{}", record.name),
                    "Share address",
                    !state.busy,
                ));
                nodes.push(button(
                    &format!("disable:{}", record.name),
                    "Disable",
                    !state.busy,
                ));
            } else {
                nodes.push(text(
                    &format!("state-{}", record.name),
                    if record.state == AddressState::Pending {
                        "Waiting to enable this address."
                    } else {
                        "This address is disabled. Its name stays yours."
                    },
                    TextRole::Body,
                ));
            }
        }
        if !state.configured {
            nodes.push(text(
                "unavailable",
                "Receiving at your name isn't available yet. Try again later.",
                TextRole::Body,
            ));
        } else if let Some(name) = &state.name {
            if !state
                .records
                .iter()
                .any(|record| record.name == *name && record.state == AddressState::Active)
            {
                nodes.push(text(
                    "name",
                    &format!("Use {name}@openagents.com to receive payments."),
                    TextRole::Body,
                ));
                nodes.push(button(
                    "enable",
                    "Enable Lightning address",
                    !state.busy && wallet_ready,
                ));
            }
        } else {
            nodes.push(text(
                "name-required",
                "Claim a name in your public profile first.",
                TextRole::Body,
            ));
        }
    }
    if let Some(notice) = &state.notice {
        nodes.push(text("notice", notice, TextRole::Body));
    }
    nodes.push(button(
        "refresh",
        if state.busy {
            "Please wait…"
        } else {
            "Refresh"
        },
        !state.busy,
    ));
    View::new_v3(
        instance,
        state.revision,
        node(
            "lightning-address",
            Element::Stack {
                axis: Axis::Vertical,
                children: nodes,
            },
        ),
    )
}

fn text(key: &str, value: &str, role: TextRole) -> Node<Action> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}
fn button(key: &str, label: &str, enabled: bool) -> Node<Action> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent: Action(key.into()),
        },
    )
}

#[uniffi::export]
impl CoreLightningAddress {
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self.view().validate().map_err(|_| failed(UNREACHABLE))?;
        serde_json::to_string(view.view()).map_err(|_| failed(UNREACHABLE))
    }
    pub async fn refresh(&self) -> CoreResult<()> {
        let lease = self
            .controller
            .lease()
            .ok_or_else(|| failed("Log in again to continue."))?;
        {
            let mut state = self.state.lock().unwrap();
            if state.busy {
                return Ok(());
            }
            state.busy = true;
            state.revision += 1;
        }
        let read = lease.clone();
        let result = openagents_client::runtime::shared()
            .spawn(async move {
                let names = read
                    .client()
                    .account_name(read.session())
                    .await
                    .map_err(|_| UNREACHABLE.to_string())?;
                let addresses = read
                    .client()
                    .lightning_addresses(read.session())
                    .await
                    .map_err(|_| UNREACHABLE.to_string())?;
                Ok::<_, String>((names.name, addresses))
            })
            .await
            .unwrap_or_else(|_| Err(UNREACHABLE.to_string()));
        let mut state = self.state.lock().unwrap();
        state.busy = false;
        state.revision += 1;
        if !lease.is_current() {
            state.records.clear();
            state.loaded = false;
            state.generation = None;
            return Err(failed("Your account changed. Open the address again."));
        }
        match result {
            Ok((name, addresses)) => {
                state.name = name;
                state.configured = addresses.configured;
                state.records = addresses.addresses;
                state.loaded = true;
                state.generation = Some(lease.generation);
                state.notice = None;
                Ok(())
            }
            Err(_) => {
                state.notice = Some(UNREACHABLE.into());
                Err(failed(UNREACHABLE))
            }
        }
    }
    /// Returns a copy effect only after validating the displayed action.
    pub async fn activate(
        &self,
        instance: String,
        revision: u64,
        key: String,
    ) -> CoreResult<String> {
        let action = self
            .view()
            .validate()
            .map_err(|_| failed(UNREACHABLE))?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| failed("This changed. Try again."))?
            .clone();
        if action.0 == "refresh" {
            self.refresh().await?;
            return Ok("{}".into());
        }
        let lease = self
            .controller
            .lease()
            .ok_or_else(|| failed("Log in again to continue."))?;
        let (name, expected) = {
            let state = self.state.lock().unwrap();
            if state.generation != Some(lease.generation) || state.busy {
                return Err(failed("This changed. Try again."));
            }
            if let Some(name) = action
                .0
                .strip_prefix("copy:")
                .or_else(|| action.0.strip_prefix("share:"))
            {
                let record = state
                    .records
                    .iter()
                    .find(|record| record.name == name && record.state == AddressState::Active)
                    .ok_or_else(|| failed("This changed. Try again."))?;
                let effect = if action.0.starts_with("share:") {
                    "share"
                } else {
                    "copy"
                };
                return Ok(
                    serde_json::json!({(effect):format!("{}@{}",record.name,record.domain)})
                        .to_string(),
                );
            }
            let name = action
                .0
                .strip_prefix("disable:")
                .map(str::to_owned)
                .or_else(|| state.name.clone())
                .ok_or_else(|| failed("Claim a name in your public profile first."))?;
            let revision = state
                .records
                .iter()
                .find(|record| record.name == name)
                .map_or(0, |record| record.revision);
            (name, revision)
        };
        let node = self
            .wallet
            .node()
            .ok_or_else(|| failed("Open your wallet first."))?;
        {
            let mut state = self.state.lock().unwrap();
            if state.busy {
                return Err(failed("Please wait for this request to finish."));
            }
            state.busy = true;
            state.revision += 1;
        }
        let disable = action.0.starts_with("disable:");
        let work = lease.clone();
        let result = openagents_client::runtime::shared()
            .spawn(async move {
                if disable {
                    work.client()
                        .disable_lightning_address(work.session(), &name, expected)
                        .await
                        .map_err(|_| UNREACHABLE.to_string())?;
                } else {
                    let challenge = work
                        .client()
                        .lightning_challenge(work.session(), expected)
                        .await
                        .map_err(|_| UNREACHABLE.to_string())?;
                    if challenge.name != name
                        || challenge.domain != "openagents.com"
                        || !work.is_current()
                    {
                        return Err("The address request changed. Refresh and try again.".into());
                    }
                    let message = challenge.message.clone();
                    let signing = node.clone();
                    let proof = tokio::task::spawn_blocking(move || {
                        signing.sign_lightning_challenge(&message)
                    })
                    .await
                    .map_err(|_| UNREACHABLE.to_string())??;
                    if !work.is_current() {
                        return Err("Your account changed. Open the address again.".into());
                    }
                    work.client()
                        .enroll_lightning_address(
                            work.session(),
                            &challenge.id,
                            &proof.pubkey,
                            &proof.signature,
                        )
                        .await
                        .map_err(|_| UNREACHABLE.to_string())?;
                    tokio::task::spawn_blocking(move || node.register_lightning_address(&name))
                        .await
                        .map_err(|_| UNREACHABLE.to_string())??;
                }
                Ok::<_, String>(())
            })
            .await
            .unwrap_or_else(|_| Err(UNREACHABLE.to_string()));
        {
            let mut state = self.state.lock().unwrap();
            state.busy = false;
            state.revision += 1;
            state.notice = result.as_ref().err().cloned();
        }
        result.map_err(failed)?;
        self.refresh().await?;
        Ok("{}".into())
    }
}

fn node(key: &str, element: Element<Action>) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(address_state: &str) -> State {
        State {
            revision: 7,
            loaded: true,
            configured: true,
            name: Some("alice".into()),
            records: vec![
                serde_json::from_value(serde_json::json!({
                    "name":"alice","account":"account-a","nostr_pubkey":"11".repeat(32),
                    "spark_pubkey":"02abcd","domain":"openagents.com","revision":1,
                    "state":address_state,"created":1,"updated":1,"last_challenge":"challenge"
                }))
                .unwrap(),
            ],
            ..State::default()
        }
    }
    fn press(view: &View<Action>, instance: &str, revision: u64, key: &str) -> bool {
        view.clone()
            .validate()
            .unwrap()
            .activate(&Activation {
                instance: instance.into(),
                revision,
                node: key.into(),
            })
            .is_ok()
    }
    #[test]
    fn only_active_addresses_copy_and_displayed_actions_are_fenced() {
        let active = address_view("sheet", &state("active"), true, true);
        assert!(press(&active, "sheet", 7, "copy:alice"));
        assert!(press(&active, "sheet", 7, "share:alice"));
        assert!(!press(&active, "old-sheet", 7, "copy:alice"));
        assert!(!press(&active, "sheet", 6, "disable:alice"));
        assert!(!press(&active, "sheet", 7, "enable"));
        let pending = address_view("sheet", &state("pending"), true, true);
        assert!(!press(&pending, "sheet", 7, "copy:alice"));
        assert!(press(&pending, "sheet", 7, "enable"));
        let opening = address_view("sheet", &state("pending"), true, false);
        assert!(!press(&opening, "sheet", 7, "enable"));
        let signed_out = address_view("sheet", &state("active"), false, true);
        assert!(
            !serde_json::to_string(&signed_out)
                .unwrap()
                .contains("alice@openagents.com")
        );
        assert!(!press(&signed_out, "sheet", 7, "copy:alice"));
    }
    #[test]
    fn busy_controls_refuse_a_second_request_and_copy_stays_plain() {
        let mut state = state("pending");
        state.busy = true;
        let view = address_view("sheet", &state, true, true);
        assert!(!press(&view, "sheet", 7, "enable"));
        assert!(!press(&view, "sheet", 7, "refresh"));
        fn check(value: &serde_json::Value) {
            if let Some(properties) = value.get("props") {
                for key in ["value", "label"] {
                    if let Some(text) = properties[key].as_str() {
                        assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
                    }
                }
            }
            match value {
                serde_json::Value::Array(values) => values.iter().for_each(check),
                serde_json::Value::Object(values) => values.values().for_each(check),
                _ => {}
            }
        }
        check(&serde_json::to_value(view).unwrap());
    }
}
