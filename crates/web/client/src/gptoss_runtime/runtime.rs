struct GpuAllocTracker {
    bytes: usize,
    buffers: usize,
}

impl GpuAllocTracker {
    fn reset(&mut self) {
        self.bytes = 0;
        self.buffers = 0;
    }

    fn add_buffers(&mut self, bytes: usize, buffers: usize) {
        self.bytes = self.bytes.saturating_add(bytes);
        self.buffers = self.buffers.saturating_add(buffers);
    }
}

pub(crate) struct GptOssRuntime {
    pub(crate) gguf_source: GgufSource,
    pub(crate) gguf_label: String,
    pub(crate) gpu: GpuContext,
    pub(crate) index: Option<GgufIndex>,
}

impl GptOssRuntime {
    pub(crate) fn new(gguf_source: GgufSource, gpu: GpuContext) -> Self {
        let gguf_label = gguf_source.label();
        Self {
            gguf_source,
            gguf_label,
            gpu,
            index: None,
        }
    }

    pub(crate) async fn load_index(
        &mut self,
        initial_bytes: u64,
        max_attempts: usize,
    ) -> Result<&GgufIndex, String> {
        let index =
            fetch_and_parse_index_source(&self.gguf_source, initial_bytes, max_attempts).await?;
        self.index = Some(index);
        Ok(self.index.as_ref().expect("index set"))
    }

    pub(crate) async fn read_tensor_slice(
        &self,
        tensor: &GgufTensor,
        len: usize,
    ) -> Result<Vec<u8>, String> {
        let bytes =
            fetch_range_source(&self.gguf_source, tensor.absolute_offset, len as u64).await?;
        Ok(bytes)
    }
}

