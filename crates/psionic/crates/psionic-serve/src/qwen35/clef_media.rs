//! Clef image embeddings and explicit MRoPE positions on the CPU decoder.

use super::*;
use crate::clef::media::ClefMediaProjection;

impl CpuGgufQwen35TextGenerationService {
    /// Streams image-conditioned hidden rows without evaluating the LM head.
    pub fn stream_media_hidden_rows(
        &self,
        tokens: &[TokenId],
        media: &ClefMediaProjection,
        sink: &mut dyn FnMut(usize, &[f32]) -> Result<(), ReferenceTextGenerationError>,
    ) -> Result<(), ReferenceTextGenerationError> {
        media
            .validate(tokens, self.model.descriptor.config.hidden_size)
            .map_err(|error| {
                ReferenceTextGenerationError::Runtime(crate::RuntimeError::Backend(error))
            })?;
        if tokens.is_empty() {
            return Err(ReferenceTextGenerationError::EmptyPrompt);
        }
        let mut state = self.model.initial_state(tokens.len());
        let mut rows = media.embeddings.iter().peekable();
        for (index, token) in tokens.iter().enumerate() {
            let embedding = if rows.peek().is_some_and(|(position, _)| *position == index) {
                rows.next().map(|(_, values)| values.as_slice())
            } else {
                None
            };
            let output = self.model.forward_token_inner(
                &mut state,
                *token,
                embedding,
                media.positions[index],
                false,
            )?;
            sink(index, &output.final_hidden)?;
        }
        Ok(())
    }
}
