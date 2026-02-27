use crate::app_state::{PaneLoadState, RenderState};
use crate::pane_system::{CreditDeskPaneAction, CreditSettlementLedgerPaneAction};
use crate::runtime_lanes::AcCreditCommand;
use crate::runtime_lanes::{AcLaneSnapshot, RuntimeCommandResponse, RuntimeCommandStatus};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: AcLaneSnapshot) {
    state.ac_lane = snapshot;
    sync_credit_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.network_requests.apply_authority_response(response);
    state.provider_runtime.last_result = Some(summary.to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        let error = response.error.as_ref().map_or_else(
            || "AC lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.credit_desk.last_error = Some(error.clone());
        state.credit_desk.load_state = PaneLoadState::Error;
        state.credit_settlement_ledger.last_error = Some(error);
        state.credit_settlement_ledger.load_state = PaneLoadState::Error;
    }
}

fn sync_credit_pane_snapshots(state: &mut RenderState) {
    state.credit_desk.intent_event_id = state.ac_lane.intent_event_id.clone();
    state.credit_desk.offer_event_id = state.ac_lane.offer_event_id.clone();
    state.credit_desk.envelope_event_id = state.ac_lane.envelope_event_id.clone();
    state.credit_desk.spend_event_id = state.ac_lane.spend_auth_event_id.clone();
    if state.credit_desk.intent_event_id.is_some() {
        state.credit_desk.load_state = PaneLoadState::Ready;
    }

    state.credit_settlement_ledger.settlement_event_id = state.ac_lane.settlement_event_id.clone();
    state.credit_settlement_ledger.default_event_id = state.ac_lane.default_event_id.clone();
    if state.credit_settlement_ledger.settlement_event_id.is_some()
        || state.credit_settlement_ledger.default_event_id.is_some()
    {
        state.credit_settlement_ledger.load_state = PaneLoadState::Ready;
    }
}

pub(super) fn run_credit_desk_action(
    state: &mut RenderState,
    action: CreditDeskPaneAction,
) -> bool {
    match action {
        CreditDeskPaneAction::PublishIntent => {
            let scope = state.credit_desk.scope.trim().to_string();
            let skill_scope_id = super::super::skill_scope_from_scope(&scope);
            match state.queue_ac_command(AcCreditCommand::PublishCreditIntent {
                scope,
                request_type: "credit.intent".to_string(),
                payload: "{\"source\":\"credit_desk\"}".to_string(),
                skill_scope_id,
                credit_envelope_ref: state.credit_desk.envelope_event_id.clone(),
                requested_sats: state.credit_desk.requested_sats.max(1),
                timeout_seconds: 60,
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued credit intent command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::PublishOffer => {
            let Some(intent_event_id) = state
                .credit_desk
                .intent_event_id
                .clone()
                .or_else(|| state.ac_lane.intent_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish intent before creating an offer".to_string());
                state.credit_desk.load_state = PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditOffer {
                intent_event_id,
                offered_sats: state.credit_desk.offered_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued credit offer command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::PublishEnvelope => {
            let Some(offer_event_id) = state
                .credit_desk
                .offer_event_id
                .clone()
                .or_else(|| state.ac_lane.offer_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish offer before creating an envelope".to_string());
                state.credit_desk.load_state = PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditEnvelope {
                offer_event_id,
                cap_sats: state.credit_desk.envelope_cap_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued envelope command #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        CreditDeskPaneAction::AuthorizeSpend => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_desk.last_error =
                    Some("Publish envelope before authorizing spend".to_string());
                state.credit_desk.load_state = PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditSpendAuth {
                envelope_event_id,
                job_id: state.credit_desk.spend_job_id.clone(),
                spend_sats: state.credit_desk.spend_sats.max(1),
            }) {
                Ok(command_seq) => {
                    state.credit_desk.last_error = None;
                    state.credit_desk.load_state = PaneLoadState::Ready;
                    state.credit_desk.last_action =
                        Some(format!("Queued spend authorization #{command_seq}"));
                }
                Err(error) => {
                    state.credit_desk.last_error = Some(error);
                    state.credit_desk.load_state = PaneLoadState::Error;
                }
            }
            true
        }
    }
}

pub(super) fn run_credit_settlement_ledger_action(
    state: &mut RenderState,
    action: CreditSettlementLedgerPaneAction,
) -> bool {
    match action {
        CreditSettlementLedgerPaneAction::VerifySettlement => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_settlement_ledger.last_error =
                    Some("No credit envelope available for settlement".to_string());
                state.credit_settlement_ledger.load_state = PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditSettlement {
                envelope_event_id,
                result_event_id: state.credit_settlement_ledger.result_event_id.clone(),
                payment_pointer: state.credit_settlement_ledger.payment_pointer.clone(),
            }) {
                Ok(command_seq) => {
                    state.credit_settlement_ledger.last_error = None;
                    state.credit_settlement_ledger.load_state = PaneLoadState::Ready;
                    state.credit_settlement_ledger.last_action =
                        Some(format!("Queued settlement verification #{command_seq}"));
                }
                Err(error) => {
                    state.credit_settlement_ledger.last_error = Some(error);
                    state.credit_settlement_ledger.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        CreditSettlementLedgerPaneAction::EmitDefaultNotice => {
            let Some(envelope_event_id) = state
                .credit_desk
                .envelope_event_id
                .clone()
                .or_else(|| state.ac_lane.envelope_event_id.clone())
            else {
                state.credit_settlement_ledger.last_error =
                    Some("No credit envelope available for default notice".to_string());
                state.credit_settlement_ledger.load_state = PaneLoadState::Error;
                return true;
            };
            match state.queue_ac_command(AcCreditCommand::PublishCreditDefault {
                envelope_event_id,
                reason: state.credit_settlement_ledger.default_reason.clone(),
            }) {
                Ok(command_seq) => {
                    state.credit_settlement_ledger.last_error = None;
                    state.credit_settlement_ledger.load_state = PaneLoadState::Ready;
                    state.credit_settlement_ledger.last_action =
                        Some(format!("Queued default notice #{command_seq}"));
                }
                Err(error) => {
                    state.credit_settlement_ledger.last_error = Some(error);
                    state.credit_settlement_ledger.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        CreditSettlementLedgerPaneAction::EmitReputationLabel => {
            let label = if state.credit_settlement_ledger.settlement_event_id.is_some() {
                "reputation:positive:settled"
            } else if state.credit_settlement_ledger.default_event_id.is_some() {
                "reputation:negative:default"
            } else {
                "reputation:neutral:pending"
            };
            state.credit_settlement_ledger.last_error = None;
            state.credit_settlement_ledger.load_state = PaneLoadState::Ready;
            state.credit_settlement_ledger.last_action =
                Some(format!("Emitted NIP-32 label {label}"));
            true
        }
    }
}
