use std::collections::BTreeSet;

use openagents_kernel_core::pylon_training::{
    PylonTrainingReputationLabel, PylonTrainingReputationNamespace,
    PylonTrainingReputationProjection, PylonTrainingReputationRecord,
    reputation_projection_for_label,
};

use crate::Event;
use crate::nip32::{KIND_LABEL, Label, LabelEvent, LabelTarget};

pub const PYLON_TRAINING_NIP32_NAMESPACES: [&str; 4] = [
    "trn/contributor",
    "trn/validator",
    "trn/build",
    "trn/checkpoint",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingObservedLabel {
    pub projection: PylonTrainingReputationProjection,
    pub subject_pubkey: Option<String>,
    pub event_ref: Option<String>,
    pub address_ref: Option<String>,
}

pub fn pylon_training_reputation_targets(
    record: &PylonTrainingReputationRecord,
) -> Result<Vec<LabelTarget>, String> {
    record.validate()?;
    let mut targets = Vec::new();
    if let Some(pubkey) = record.subject_pubkey.as_ref() {
        targets.push(LabelTarget::pubkey(pubkey.clone(), None::<String>));
    }
    if let Some(event_ref) = record.event_ref.as_ref() {
        targets.push(LabelTarget::event(event_ref.clone(), None::<String>));
    }
    if let Some(address_ref) = record.address_ref.as_ref() {
        targets.push(LabelTarget::address(address_ref.clone(), None::<String>));
    }
    Ok(targets)
}

pub fn pylon_training_reputation_to_label_event(
    record: &PylonTrainingReputationRecord,
) -> Result<LabelEvent, String> {
    Ok(LabelEvent::new(
        vec![Label::new(record.label.label(), record.namespace.label())],
        pylon_training_reputation_targets(record)?,
    ))
}

pub fn pylon_training_reputation_projection(
    namespace: &str,
    label: &str,
    created_at_unix: u64,
    now_unix: u64,
) -> Option<PylonTrainingReputationProjection> {
    let namespace = PylonTrainingReputationNamespace::parse(namespace)?;
    let label = PylonTrainingReputationLabel::parse(label)?;
    reputation_projection_for_label(namespace, label, created_at_unix, now_unix).ok()
}

pub fn pylon_training_single_namespace(event: &Event) -> Option<&str> {
    let namespaces = event
        .tags
        .iter()
        .filter_map(|tag| {
            (tag.first().is_some_and(|value| value == "L"))
                .then(|| tag.get(1).map(String::as_str))
                .flatten()
        })
        .collect::<BTreeSet<_>>();
    (namespaces.len() == 1)
        .then(|| namespaces.iter().next().copied())
        .flatten()
}

pub fn pylon_training_observed_labels_from_event(
    event: &Event,
    subject_pubkey: &str,
    event_refs: &BTreeSet<String>,
    address_refs: &BTreeSet<String>,
    now_unix: u64,
) -> Vec<PylonTrainingObservedLabel> {
    if event.kind != KIND_LABEL as u16 {
        return Vec::new();
    }

    let subject_pubkey_ref = event.tags.iter().find_map(|tag| {
        (tag.first().is_some_and(|value| value == "p")
            && tag.get(1).is_some_and(|value| value == subject_pubkey))
        .then(|| tag.get(1).cloned())
        .flatten()
    });
    let event_ref = event.tags.iter().find_map(|tag| {
        (tag.first().is_some_and(|value| value == "e")
            && tag.get(1).is_some_and(|value| event_refs.contains(value)))
        .then(|| tag.get(1).cloned())
        .flatten()
    });
    let address_ref = event.tags.iter().find_map(|tag| {
        (tag.first().is_some_and(|value| value == "a")
            && tag.get(1).is_some_and(|value| address_refs.contains(value)))
        .then(|| tag.get(1).cloned())
        .flatten()
    });
    if subject_pubkey_ref.is_none() && event_ref.is_none() && address_ref.is_none() {
        return Vec::new();
    }

    let mut observed = Vec::new();
    for tag in &event.tags {
        if !tag.first().is_some_and(|value| value == "l") {
            continue;
        }
        let Some(label_value) = tag.get(1).map(String::as_str) else {
            continue;
        };
        let namespace = tag
            .get(2)
            .map(|value| value.as_str())
            .or_else(|| pylon_training_single_namespace(event));
        let Some(namespace) = namespace else {
            continue;
        };
        let Some(projection) = pylon_training_reputation_projection(
            namespace,
            label_value,
            event.created_at,
            now_unix,
        ) else {
            continue;
        };
        observed.push(PylonTrainingObservedLabel {
            projection,
            subject_pubkey: subject_pubkey_ref.clone(),
            event_ref: event_ref.clone(),
            address_ref: address_ref.clone(),
        });
    }
    observed
}

#[cfg(test)]
mod tests {
    use super::{
        PYLON_TRAINING_NIP32_NAMESPACES, pylon_training_observed_labels_from_event,
        pylon_training_reputation_projection, pylon_training_reputation_targets,
        pylon_training_reputation_to_label_event,
    };
    use crate::nip32::{KIND_LABEL, Label, LabelEvent, LabelTarget};
    use openagents_kernel_core::pylon_training::{
        PylonTrainingReputationLabel, PylonTrainingReputationNamespace,
        PylonTrainingReputationRecord, PylonTrainingSchedulerEffect, scheduler_effect_for_label,
    };
    use std::collections::BTreeSet;

    #[test]
    fn pylon_training_reputation_records_convert_to_nip32_events() {
        let record = PylonTrainingReputationRecord::new(
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Good,
            Some("pubkey-hex".to_string()),
            Some("event-id".to_string()),
            Some("30078:pubkey:d-tag".to_string()),
        )
        .expect("record should build");
        let targets =
            pylon_training_reputation_targets(&record).expect("label targets should build");
        assert_eq!(targets.len(), 3);
        let event =
            pylon_training_reputation_to_label_event(&record).expect("label event should build");
        assert_eq!(event.labels.len(), 1);
        assert_eq!(
            event.labels[0].namespace.as_deref(),
            Some("trn/contributor")
        );
        assert_eq!(event.labels[0].value, "good");
        assert_eq!(event.targets.len(), 3);
        assert_eq!(
            scheduler_effect_for_label(
                PylonTrainingReputationNamespace::Contributor,
                PylonTrainingReputationLabel::Good,
                2,
            )
            .expect("effect should resolve"),
            PylonTrainingSchedulerEffect::SoftPositive
        );
        assert_eq!(PYLON_TRAINING_NIP32_NAMESPACES[0], "trn/contributor");
    }

    #[test]
    fn pylon_training_reputation_projection_applies_decay_and_gating() {
        let projected = pylon_training_reputation_projection(
            "trn/checkpoint",
            "warning",
            1_700_000_000,
            1_700_000_000 + (31 * 86_400),
        )
        .expect("projection should resolve");
        assert_eq!(
            projected.namespace,
            PylonTrainingReputationNamespace::Checkpoint
        );
        assert_eq!(projected.label, PylonTrainingReputationLabel::Warning);
        assert_eq!(projected.age_days, 31);
        assert_eq!(
            projected.scheduler_effect,
            PylonTrainingSchedulerEffect::Ignored
        );
        assert!(!projected.hard_gate);
    }

    #[test]
    fn pylon_training_observed_labels_resolve_single_namespace_and_targets() {
        let subject_pubkey = "11".repeat(32);
        let mut label_event = LabelEvent::new(
            vec![Label::new("poor", "trn/validator")],
            vec![
                LabelTarget::pubkey(subject_pubkey.clone(), None::<String>),
                LabelTarget::event("event.alpha".to_string(), None::<String>),
                LabelTarget::address("39512:22:challenge.alpha".to_string(), None::<String>),
            ],
        );
        label_event.set_content("validator drift");
        let event = crate::Event {
            id: "label.validator.poor.alpha".to_string(),
            pubkey: "22".repeat(32),
            created_at: 1_700_000_000,
            kind: KIND_LABEL as u16,
            tags: label_event.to_tags(),
            content: label_event.content,
            sig: "00".repeat(64),
        };
        let event_refs = BTreeSet::from(["event.alpha".to_string()]);
        let address_refs = BTreeSet::from(["39512:22:challenge.alpha".to_string()]);
        let observed = pylon_training_observed_labels_from_event(
            &event,
            subject_pubkey.as_str(),
            &event_refs,
            &address_refs,
            1_700_000_000 + 86_400,
        );
        assert_eq!(observed.len(), 1);
        assert_eq!(
            observed[0].projection.namespace,
            PylonTrainingReputationNamespace::Validator
        );
        assert_eq!(
            observed[0].projection.label,
            PylonTrainingReputationLabel::Poor
        );
        assert_eq!(
            observed[0].projection.scheduler_effect,
            PylonTrainingSchedulerEffect::SoftNegative
        );
        assert!(!observed[0].projection.hard_gate);
        assert_eq!(
            observed[0].subject_pubkey.as_deref(),
            Some(subject_pubkey.as_str())
        );
        assert_eq!(observed[0].event_ref.as_deref(), Some("event.alpha"));
        assert_eq!(
            observed[0].address_ref.as_deref(),
            Some("39512:22:challenge.alpha")
        );
    }
}
