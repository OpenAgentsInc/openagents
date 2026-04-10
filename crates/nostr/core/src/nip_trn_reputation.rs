use openagents_kernel_core::pylon_training::PylonTrainingReputationRecord;

use crate::nip32::{Label, LabelEvent, LabelTarget};

pub fn pylon_training_reputation_to_label_event(
    record: &PylonTrainingReputationRecord,
) -> Result<LabelEvent, String> {
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
    Ok(LabelEvent::new(
        vec![Label::new(record.label.label(), record.namespace.label())],
        targets,
    ))
}

#[cfg(test)]
mod tests {
    use openagents_kernel_core::pylon_training::{
        scheduler_effect_for_label, PylonTrainingReputationLabel, PylonTrainingReputationNamespace,
        PylonTrainingSchedulerEffect,
    };

    use super::pylon_training_reputation_to_label_event;

    #[test]
    fn pylon_training_reputation_records_convert_to_nip32_events() {
        let record = openagents_kernel_core::pylon_training::PylonTrainingReputationRecord::new(
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Good,
            Some("pubkey-hex".to_string()),
            Some("event-id".to_string()),
            Some("30078:pubkey:d-tag".to_string()),
        )
        .expect("record should build");
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
    }
}
