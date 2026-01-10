struct LayerKvCache {
    k: Vec<f32>,
    v: Vec<f32>,
    start: usize,
    len: usize,
    capacity: usize,
    stride: usize,
    gpu_k: Option<wgpu::Buffer>,
    gpu_v: Option<wgpu::Buffer>,
    gpu_bytes: usize,
    cpu_enabled: bool,
}

impl LayerKvCache {
    fn new() -> Self {
        Self {
            k: Vec::new(),
            v: Vec::new(),
            start: 0,
            len: 0,
            capacity: 0,
            stride: 0,
            gpu_k: None,
            gpu_v: None,
            gpu_bytes: 0,
            cpu_enabled: true,
        }
    }

    fn token_count(&self) -> usize {
        self.len
    }

    fn ensure_capacity(
        &mut self,
        max_len: usize,
        stride: usize,
        gpu: &GpuContext,
        store_cpu: bool,
    ) -> Result<(), String> {
        if max_len == 0 {
            return Err("kv cache max length is zero".to_string());
        }
        if stride == 0 {
            return Err("kv cache stride is zero".to_string());
        }
        if self.capacity == 0
            || self.capacity != max_len
            || self.stride != stride
            || self.cpu_enabled != store_cpu
        {
            self.capacity = max_len;
            self.stride = stride;
            self.start = 0;
            self.len = 0;
            self.cpu_enabled = store_cpu;
            if store_cpu {
                self.k = vec![0.0f32; max_len * stride];
                self.v = vec![0.0f32; max_len * stride];
            } else {
                self.k.clear();
                self.v.clear();
            }
            self.gpu_k = None;
            self.gpu_v = None;
            self.gpu_bytes = 0;
        }

        if self.gpu_k.is_none() || self.gpu_v.is_none() {
            let bytes = max_len
                .checked_mul(stride)
                .ok_or_else(|| "kv cache byte overflow".to_string())?
                .checked_mul(std::mem::size_of::<f32>())
                .ok_or_else(|| "kv cache byte overflow".to_string())?;
            let limits = gpu.device.limits();
            ensure_buffer_limit(
                "kv cache",
                bytes,
                limits.max_storage_buffer_binding_size.into(),
                limits.max_buffer_size,
            )?;
            let k_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kv_cache_k"),
                size: bytes as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let v_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kv_cache_v"),
                size: bytes as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.gpu_k = Some(k_buf);
            self.gpu_v = Some(v_buf);
            self.gpu_bytes = bytes * 2;
        }

        Ok(())
    }

    fn append(
        &mut self,
        k: &[f32],
        v: &[f32],
        kv_heads: usize,
        head_dim: usize,
        max_len: usize,
        gpu: &GpuContext,
        store_cpu: bool,
    ) -> Result<(), String> {
        let expected = kv_heads
            .checked_mul(head_dim)
            .ok_or_else(|| "kv append overflow".to_string())?;
        if k.len() != expected || v.len() != expected {
            return Err(format!(
                "kv append shape mismatch k={} v={} expected={expected}",
                k.len(),
                v.len()
            ));
        }
        self.ensure_capacity(max_len, expected, gpu, store_cpu)?;

        let slot = if self.len < self.capacity {
            let slot = (self.start + self.len) % self.capacity;
            self.len = self.len.saturating_add(1).min(self.capacity);
            slot
        } else {
            let slot = self.start;
            self.start = (self.start + 1) % self.capacity;
            slot
        };

        let base = slot
            .checked_mul(self.stride)
            .ok_or_else(|| "kv append base overflow".to_string())?;
        let end = base + self.stride;
        if self.cpu_enabled {
            self.k[base..end].copy_from_slice(k);
            self.v[base..end].copy_from_slice(v);
        }

        let byte_offset = base
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| "kv append offset overflow".to_string())? as u64;
        if let (Some(k_buf), Some(v_buf)) = (self.gpu_k.as_ref(), self.gpu_v.as_ref()) {
            gpu.queue.write_buffer(k_buf, byte_offset, cast_slice(k));
            gpu.queue.write_buffer(v_buf, byte_offset, cast_slice(v));
        }

        Ok(())
    }

    fn memory_bytes(&self) -> usize {
        self.gpu_bytes
    }
}

struct KvCache {
    layers: Vec<LayerKvCache>,
    seq_len: usize,
    max_len: usize,
}

impl KvCache {
    fn new(layer_count: usize, max_len: usize) -> Self {
        let mut layers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            layers.push(LayerKvCache::new());
        }
        Self {
            layers,
            seq_len: 0,
            max_len,
        }
    }

    fn layer_mut(&mut self, layer: usize) -> Result<&mut LayerKvCache, String> {
        self.layers
            .get_mut(layer)
            .ok_or_else(|| format!("kv cache missing layer {layer}"))
    }

    fn total_bytes(&self) -> usize {
        self.layers.iter().map(LayerKvCache::memory_bytes).sum()
    }
}

#[derive(Clone, Debug)]
struct CacheStats {
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
    bytes: usize,
    entries: usize,
}

#[derive(Clone, Debug)]
struct TensorCacheEntry {
    data: Vec<f32>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct TensorCache {
    entries: HashMap<String, TensorCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl TensorCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, name: &str) -> Option<Vec<f32>> {
        let hit = self.entries.get(name).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(name);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, name: String, data: Vec<f32>) {
        let bytes = data.len() * std::mem::size_of::<f32>();
        if bytes > TENSOR_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > TENSOR_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&name) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &name);
        }
        while self.total_bytes.saturating_add(bytes) > TENSOR_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(name.clone());
        self.entries.insert(
            name,
            TensorCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, name: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == name) {
            self.order.remove(pos);
            self.order.push_back(name.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct TokenCacheEntry {
    data: Vec<f32>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct TokenCache {
    entries: HashMap<u32, TokenCacheEntry>,
    order: VecDeque<u32>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl TokenCache {
    fn new() -> Self {
        Self {
            entries: HashMap::default(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, token_id: u32) -> Option<Vec<f32>> {
        let hit = self.entries.get(&token_id).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(token_id);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, token_id: u32, data: Vec<f32>) {
        let bytes = data.len() * std::mem::size_of::<f32>();
        if bytes > TOKEN_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > TOKEN_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&token_id) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| *v != token_id);
        }
        while self.total_bytes.saturating_add(bytes) > TOKEN_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(token_id);
        self.entries.insert(
            token_id,
            TokenCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, token_id: u32) {
        if let Some(pos) = self.order.iter().position(|v| *v == token_id) {
            self.order.remove(pos);
            self.order.push_back(token_id);
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct QuantCacheEntry {
    data: Vec<u8>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct QuantCache {
    entries: HashMap<String, QuantCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl QuantCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, name: &str) -> Option<Vec<u8>> {
        let hit = self.entries.get(name).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(name);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, name: String, data: Vec<u8>) {
        let bytes = data.len();
        if bytes > Q8_0_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > Q8_0_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&name) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &name);
        }
        while self.total_bytes.saturating_add(bytes) > Q8_0_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(name.clone());
        self.entries.insert(
            name,
            QuantCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, name: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == name) {
            self.order.remove(pos);
            self.order.push_back(name.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct ExpertCacheEntry {
    data: Vec<u8>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct ExpertCache {
    entries: HashMap<String, ExpertCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl ExpertCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        let hit = self.entries.get(key).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, key: String, data: Vec<u8>) {
        let bytes = data.len();
        if bytes > EXPERT_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > EXPERT_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &key);
        }
        while self.total_bytes.saturating_add(bytes) > EXPERT_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            ExpertCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == key) {
            self.order.remove(pos);
            self.order.push_back(key.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

struct RuntimeCaches {
    token_embd: TokenCache,
    tensors: TensorCache,
    quant: QuantCache,
    experts: ExpertCache,
    moe_disabled: bool,
}

impl RuntimeCaches {
    fn new() -> Self {
        Self {
            token_embd: TokenCache::new(),
            tensors: TensorCache::new(),
            quant: QuantCache::new(),
            experts: ExpertCache::new(),
            moe_disabled: false,
        }
    }
}

#[derive(Default)]
