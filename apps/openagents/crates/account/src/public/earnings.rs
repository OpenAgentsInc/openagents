//! The account owner chooses which individual earnings appear on their page.
use super::*;
use nostr::earnings::{Category, State};

pub(super) fn append(children: &mut Vec<Node<String>>, state: &FormState, generation: Option<u64>) {
    if generation.is_none() || generation != state.earnings_generation || state.earnings.is_empty()
    {
        return;
    }
    let text = |key: String, value: String| Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Text {
            value,
            role: TextRole::Body,
        },
    };
    children.push(text("earnings-title".into(), "Earnings".into()));
    children.push(text(
        "earnings-help".into(),
        "Choose what to show on your public profile.".into(),
    ));
    for row in state.earnings.iter().take(40) {
        let receipt = &row.receipt;
        let category = match receipt.category {
            Category::Pylon => "Pylon",
            Category::Plugin => "Plugin sales",
            Category::Tip => "Tips",
        };
        let status = match receipt.state {
            State::Pending => "Pending",
            State::Earned => "Earned",
            State::Paid => "Paid",
            State::Reversed => "Reversed",
        };
        let amount = bitcoin_amount::Format::Bip177.show_msat(receipt.amount_msat);
        children.push(text(
            format!("earning-{}", receipt.source),
            format!("{category} · {amount} · {status}"),
        ));
        if let Some(plugin) = &receipt.plugin {
            children.push(text(
                format!("earning-plugin-{}", receipt.source),
                format!("Plugin: {plugin}"),
            ));
        }
        if let Some(release) = &receipt.release {
            children.push(text(
                format!("earning-release-{}", receipt.source),
                format!("Release: {release}"),
            ));
        }
        children.push(Node {
            key: format!("earning-action-{}", receipt.source).into(),
            style: Default::default(),
            element: Element::Button {
                label: if row.published {
                    "Hide from profile"
                } else {
                    "Show on profile"
                }
                .into(),
                enabled: !state.busy,
                icon: None,
                shortcut: None,
                intent: format!(
                    "earning:{}:{}",
                    if row.published { "hide" } else { "show" },
                    receipt.source
                ),
            },
        });
    }
}
impl ProfileForm {
    pub(super) async fn set_earning(&self, intent: &str) -> Result<(), String> {
        let (action, source) = intent.split_once(':').ok_or("Choose an action.")?;
        if !matches!(action, "show" | "hide") {
            return Err("Choose an action.".into());
        }
        let _gate = self.io.lock().await;
        let (lease, identity) = self.signed_in()?;
        let row = {
            let mut state = self.state.lock().unwrap();
            if state.earnings_generation != Some(lease.generation) {
                return Err(copy::NOT_READY.into());
            }
            let row = state
                .earnings
                .iter()
                .find(|r| r.receipt.source == source)
                .cloned()
                .ok_or("Refresh your earnings and try again.")?;
            if row.receipt.party != identity.pubkey_hex() {
                return Err(copy::NOT_READY.into());
            }
            state.busy = true;
            self.changed(&mut state);
            row
        };
        let result = async {
            let client = lease.client();
            let session = lease.session();
            let party = identity.pubkey_hex();
            if action == "hide" {
                client
                    .hide_earning(session, &party, source)
                    .await
                    .map(|_| ())
            } else {
                client
                    .publish_earning(session, &party, &row.source)
                    .await
                    .map(|_| ())
            }
            .map_err(|_| "We couldn't change your profile earnings. Try again.".to_owned())?;
            client
                .profile_earnings(session, &party)
                .await
                .map(|r| r.rows)
                .map_err(|_| "We couldn't refresh your earnings. Try again.".to_owned())
        }
        .await;
        let mut state = self.state.lock().unwrap();
        state.busy = false;
        if !lease.is_current() {
            state.earnings.clear();
            state.earnings_generation = None;
            self.changed(&mut state);
            return Err(copy::NOT_READY.into());
        }
        match result {
            Ok(rows) => {
                state.earnings = rows;
                state.notice = Some("Your profile earnings were updated.".into());
                self.changed(&mut state);
                Ok(())
            }
            Err(message) => {
                state.notice = Some(message.clone());
                self.changed(&mut state);
                Err(message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_sources_stay_out_of_views_and_account_switches_hide_the_rows() {
        let payment_hash = "77".repeat(32);
        let row = openagents::profile_earnings::Earning {
            source: openagents::profile_earnings::EarningsSource::Tip {
                payment_hash: payment_hash.clone(),
            },
            receipt: nostr::earnings::Receipt {
                v: nostr::earnings::SCHEMA.into(),
                party: "11".repeat(32),
                source: "22".repeat(32),
                category: Category::Tip,
                state: State::Paid,
                amount_msat: 12001,
                network: "bitcoin".into(),
                recorded_at: 1,
                evidence: None,
                plugin: None,
                release: None,
            },
            published: false,
        };
        let state = FormState {
            earnings: vec![row],
            earnings_generation: Some(1),
            ..FormState::default()
        };
        let mut children = Vec::new();
        append(&mut children, &state, Some(1));
        let encoded = serde_json::to_string(&children).unwrap();
        assert!(encoded.contains("Show on profile") && encoded.contains("12,001 msat"));
        assert!(!encoded.contains(&payment_hash));
        let mut switched = Vec::new();
        append(&mut switched, &state, Some(2));
        assert!(switched.is_empty());
        let mut signed_out = Vec::new();
        append(&mut signed_out, &state, None);
        assert!(signed_out.is_empty());
    }
}
