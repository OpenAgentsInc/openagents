//! Adapts the Gemma 4 pipeline step to the model-independent stage message.
//!
//! The Gemma 4 split (`DistributedGemma4TextGenerationService` at the front,
//! `distributed_worker_step` behind `/psionic/internal/gemma4/pipeline/step`)
//! sends one token's hidden state and forwarded key and value per step. This
//! adapter maps that request onto a [`StageMessage`] with a residual section
//! and source KV sections, so a Gemma stage can use the same codec and
//! [`StageInbox`] admission as DeepSeek-V4.1 stages: duplicate, stale,
//! missing, reordered, and cross-request steps are refused before the worker
//! touches its cache. The Gemma routes and their JSON bodies are unchanged.

use psionic_models::stage_state::{
    CodecLimits, RowRule, Section, SectionKind, SectionSpec, StageContract, StageInbox,
    StageMessage, StageRefusal, StepKind, WireDtype,
};
use sha2::{Digest, Sha256};

use crate::DistributedGemma4RemoteStepRequest;

/// The shape of one Gemma 4 split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gemma4StageAdapter {
    /// The model both halves serve.
    pub model_id: String,
    /// The first layer the remote half runs.
    pub split_layer: u32,
    /// Total decoder layers.
    pub total_layers: u32,
    /// Hidden size.
    pub hidden: u32,
    /// Forwarded key values per step; zero when none are forwarded.
    pub forwarded_key: u32,
    /// Forwarded value values per step; zero when none are forwarded.
    pub forwarded_value: u32,
}

impl Gemma4StageAdapter {
    /// The digest both halves of this split agree on, in place of a group
    /// manifest.
    #[must_use]
    pub fn manifest_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic.gemma4.pipeline.v1\0");
        hasher.update(self.model_id.as_bytes());
        hasher.update([0]);
        for value in [
            self.split_layer,
            self.total_layers,
            self.hidden,
            self.forwarded_key,
            self.forwarded_value,
        ] {
            hasher.update(value.to_le_bytes());
        }
        hasher.finalize().into()
    }

    /// The remote half's inbound contract.
    #[must_use]
    pub fn contract(&self) -> StageContract {
        let mut sections = vec![SectionSpec {
            kind: SectionKind::Residual,
            dtype: WireDtype::F32,
            inner: vec![1, self.hidden],
            rows: RowRule::PerToken,
        }];
        let layer = self.split_layer;
        if self.forwarded_key > 0 {
            sections.push(SectionSpec {
                kind: SectionKind::SourceKv { layer },
                dtype: WireDtype::F32,
                inner: vec![self.forwarded_key],
                rows: RowRule::PerToken,
            });
        }
        if self.forwarded_value > 0 {
            sections.push(SectionSpec {
                kind: SectionKind::SourceValue { layer },
                dtype: WireDtype::F32,
                inner: vec![self.forwarded_value],
                rows: RowRule::PerToken,
            });
        }
        StageContract {
            stage: 1,
            layers: self.split_layer..self.total_layers,
            sections,
        }
    }

    /// An inbox for the remote half.
    #[must_use]
    pub fn inbox(&self, generation: u64, limits: CodecLimits) -> StageInbox {
        StageInbox::new(self.manifest_digest(), generation, self.contract(), limits)
    }

    /// The stage message for a Gemma step. Gemma steps one token at a time,
    /// so the step number is the position.
    ///
    /// # Errors
    ///
    /// Refuses a request for another model or split, or with tensors of the
    /// wrong length.
    pub fn to_message(
        &self,
        request: &DistributedGemma4RemoteStepRequest,
        generation: u64,
        attempt: u32,
    ) -> Result<StageMessage, StageRefusal> {
        if request.model_id != self.model_id
            || u32::try_from(request.split_layer).ok() != Some(self.split_layer)
        {
            return Err(StageRefusal::WrongManifest);
        }
        let layer = self.split_layer;
        let mut sections = vec![Section::floats(
            SectionKind::Residual,
            WireDtype::F32,
            vec![1, 1, self.hidden],
            0,
            &request.input_hidden,
        )?];
        for (kind, width, values) in [
            (
                SectionKind::SourceKv { layer },
                self.forwarded_key,
                &request.forwarded_key,
            ),
            (
                SectionKind::SourceValue { layer },
                self.forwarded_value,
                &request.forwarded_value,
            ),
        ] {
            if width > 0 || !values.is_empty() {
                sections.push(Section::floats(
                    kind,
                    WireDtype::F32,
                    vec![1, width],
                    0,
                    values,
                )?);
            }
        }
        Ok(StageMessage {
            manifest_digest: self.manifest_digest(),
            generation,
            request_id: request.request_id.clone(),
            attempt,
            stage: 1,
            layers: self.split_layer..self.total_layers,
            step: request.position as u64,
            kind: StepKind::Decode,
            position: request.position as u64,
            tokens: vec![request.token],
            sections,
        })
    }

    /// The Gemma step request an admitted message carries.
    ///
    /// # Errors
    ///
    /// Refuses a message for another split or without its sections.
    pub fn to_request(
        &self,
        message: &StageMessage,
    ) -> Result<DistributedGemma4RemoteStepRequest, StageRefusal> {
        if message.manifest_digest != self.manifest_digest() {
            return Err(StageRefusal::WrongManifest);
        }
        let floats = |kind: SectionKind| -> Result<Vec<f32>, StageRefusal> {
            message
                .section(kind)
                .map_or(Ok(Vec::new()), Section::to_f32)
        };
        let layer = self.split_layer;
        let input_hidden = message
            .section(SectionKind::Residual)
            .ok_or(StageRefusal::MissingSection(SectionKind::Residual))?
            .to_f32()?;
        let [token] = message.tokens.as_slice() else {
            return Err(StageRefusal::Step(String::from(
                "a Gemma step carries one token",
            )));
        };
        Ok(DistributedGemma4RemoteStepRequest {
            request_id: message.request_id.clone(),
            model_id: self.model_id.clone(),
            token: *token,
            position: usize::try_from(message.position)
                .map_err(|_| StageRefusal::Step(String::from("position exceeds usize")))?,
            split_layer: self.split_layer as usize,
            input_hidden,
            forwarded_key: floats(SectionKind::SourceKv { layer })?,
            forwarded_value: floats(SectionKind::SourceValue { layer })?,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use psionic_models::stage_state::{decode, encode};

    use super::*;

    fn adapter() -> Gemma4StageAdapter {
        Gemma4StageAdapter {
            model_id: String::from("gemma4-e4b"),
            split_layer: 21,
            total_layers: 42,
            hidden: 6,
            forwarded_key: 4,
            forwarded_value: 4,
        }
    }

    fn request(position: usize) -> DistributedGemma4RemoteStepRequest {
        DistributedGemma4RemoteStepRequest {
            request_id: String::from("req-1"),
            model_id: String::from("gemma4-e4b"),
            token: 17 + position as u32,
            position,
            split_layer: 21,
            input_hidden: (0..6).map(|i| i as f32 + position as f32).collect(),
            forwarded_key: vec![0.5; 4],
            forwarded_value: vec![-0.5; 4],
        }
    }

    #[test]
    fn a_gemma_step_round_trips_through_the_stage_codec() {
        let a = adapter();
        let limits = CodecLimits::default();
        let mut inbox = a.inbox(1, limits);
        for position in 0..3 {
            let original = request(position);
            let bytes = encode(&a.to_message(&original, 1, 0).unwrap(), &limits).unwrap();
            let admitted = inbox.admit_bytes(&bytes).unwrap();
            let back = a.to_request(&admitted).unwrap();
            assert_eq!(back.request_id, original.request_id);
            assert_eq!(back.model_id, original.model_id);
            assert_eq!(back.token, original.token);
            assert_eq!(back.position, original.position);
            assert_eq!(back.split_layer, original.split_layer);
            assert_eq!(back.input_hidden, original.input_hidden);
            assert_eq!(back.forwarded_key, original.forwarded_key);
            assert_eq!(back.forwarded_value, original.forwarded_value);
        }
        // Replaying position 1 is a duplicate; skipping to 5 is a gap.
        let replay = encode(&a.to_message(&request(1), 1, 0).unwrap(), &limits).unwrap();
        assert_eq!(
            inbox.admit_bytes(&replay),
            Err(StageRefusal::Duplicate { step: 1 })
        );
        let skip = encode(&a.to_message(&request(5), 1, 0).unwrap(), &limits).unwrap();
        assert_eq!(
            inbox.admit_bytes(&skip),
            Err(StageRefusal::OutOfOrder {
                expected: 3,
                got: 5
            })
        );
        // The Gemma reset route maps to ending the attempt.
        inbox.reset("req-1", 0);
        let late = encode(&a.to_message(&request(3), 1, 0).unwrap(), &limits).unwrap();
        assert!(matches!(
            inbox.admit_bytes(&late),
            Err(StageRefusal::StaleAttempt { .. })
        ));
        assert!(decode(&late, &limits).is_ok());
    }

    #[test]
    fn another_model_or_split_is_refused() {
        let a = adapter();
        let mut other = request(0);
        other.model_id = String::from("gemma4-31b");
        assert_eq!(a.to_message(&other, 1, 0), Err(StageRefusal::WrongManifest));
        let b = Gemma4StageAdapter {
            split_layer: 20,
            ..adapter()
        };
        let message = a.to_message(&request(0), 1, 0).unwrap();
        assert_ne!(a.manifest_digest(), b.manifest_digest());
        assert_eq!(
            b.inbox(1, CodecLimits::default()).check(&message),
            Err(StageRefusal::WrongManifest)
        );
        let mut short = request(0);
        short.input_hidden.pop();
        assert!(matches!(
            a.to_message(&short, 1, 0),
            Err(StageRefusal::Malformed(_))
        ));
    }
}
