#![allow(dead_code)]

use crate::gguf_web::{fetch_and_parse_index, fetch_range, GgufIndex, GgufTensor};
use crate::state::GpuContext;

pub(crate) struct GptOssRuntime {
    pub(crate) gguf_url: String,
    pub(crate) gpu: GpuContext,
    pub(crate) index: Option<GgufIndex>,
}

impl GptOssRuntime {
    pub(crate) fn new(gguf_url: String, gpu: GpuContext) -> Self {
        Self {
            gguf_url,
            gpu,
            index: None,
        }
    }

    pub(crate) async fn load_index(
        &mut self,
        initial_bytes: u64,
        max_attempts: usize,
    ) -> Result<&GgufIndex, String> {
        let index = fetch_and_parse_index(&self.gguf_url, initial_bytes, max_attempts).await?;
        self.index = Some(index);
        Ok(self.index.as_ref().expect("index set"))
    }

    pub(crate) async fn read_tensor_slice(
        &self,
        tensor: &GgufTensor,
        len: usize,
    ) -> Result<Vec<u8>, String> {
        let bytes = fetch_range(&self.gguf_url, tensor.absolute_offset, len as u64).await?;
        Ok(bytes)
    }
}
