# ML Crate Architecture

Browser-first ML inference library with WebGPU compute.

## Crate Structure

```
crates/ml/
├── Cargo.toml
├── docs/
│   ├── ARCHITECTURE.md        # This file
│   ├── COMPUTE-KERNELS.md     # WGSL shader implementations
│   ├── RUNTIME-INTEGRATION.md # ComputeProvider integration
│   └── BROWSER-PROVIDER.md    # NIP-90 DVM service
└── src/
    ├── lib.rs
    ├── tensor/
    │   ├── mod.rs
    │   ├── dtype.rs      # DType enum
    │   ├── shape.rs      # Shape with GPU-friendly layout
    │   ├── storage.rs    # TensorStorage (GPU/CPU)
    │   └── tensor.rs     # Tensor struct
    ├── device/
    │   ├── mod.rs
    │   ├── webgpu.rs     # WebGpuDevice
    │   └── cpu.rs        # CpuDevice fallback
    ├── shaders/
    │   ├── mod.rs
    │   ├── gemm.wgsl
    │   ├── softmax.wgsl
    │   ├── rmsnorm.wgsl
    │   ├── activations.wgsl
    │   ├── rope.wgsl
    │   ├── attention.wgsl
    │   └── sinkhorn.wgsl
    ├── ops/
    │   ├── mod.rs
    │   ├── matmul.rs
    │   ├── softmax.rs
    │   ├── rmsnorm.rs
    │   ├── activations.rs
    │   ├── rope.rs
    │   └── attention.rs
    ├── model/
    │   ├── mod.rs
    │   ├── safetensors.rs  # Weight loader
    │   ├── weights.rs      # WeightManager with LRU
    │   └── config.rs       # Model configuration
    ├── llm/
    │   ├── mod.rs
    │   ├── tokenizer.rs    # tokenizers-wasm wrapper
    │   ├── kv_cache.rs     # KV cache management
    │   ├── sampling.rs     # Temperature, top-p, etc.
    │   └── generate.rs     # Text generation loop
    └── provider/
        ├── mod.rs
        ├── webgpu_provider.rs  # ComputeProvider impl
        └── browser_dvm.rs      # BrowserDvmService
```

## Core Types

### DType

Data type enumeration for tensor elements:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DType {
    F32 = 0,   // 32-bit float (default)
    F16 = 1,   // 16-bit float (half precision)
    BF16 = 2,  // Brain float 16
    I32 = 3,   // 32-bit signed integer
    U8 = 4,    // 8-bit unsigned (for quantized)
    Q4 = 5,    // 4-bit quantized (packed)
}

impl DType {
    pub fn size_bytes(&self) -> usize {
        match self {
            DType::F32 | DType::I32 => 4,
            DType::F16 | DType::BF16 => 2,
            DType::U8 => 1,
            DType::Q4 => 1,  // 2 values packed per byte
        }
    }

    pub fn wgsl_type(&self) -> &'static str {
        match self {
            DType::F32 => "f32",
            DType::F16 | DType::BF16 => "f16",  // with f16 extension
            DType::I32 => "i32",
            DType::U8 => "u32",  // unpacked in shader
            DType::Q4 => "u32",  // unpacked in shader
        }
    }
}
```

### Shape

GPU-friendly shape representation (up to 4 dimensions):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Shape {
    /// Dimensions [d0, d1, d2, d3]. Unused dimensions are 1.
    /// For 2D [M, N]: dims = [M, N, 1, 1]
    /// For 3D [B, M, N]: dims = [B, M, N, 1]
    dims: [u32; 4],
    /// Number of actual dimensions (1-4)
    ndim: u32,
}

impl Shape {
    pub fn new(dims: &[usize]) -> Self {
        assert!(dims.len() <= 4, "Maximum 4 dimensions supported");
        let mut shape_dims = [1u32; 4];
        for (i, &d) in dims.iter().enumerate() {
            shape_dims[i] = d as u32;
        }
        Self {
            dims: shape_dims,
            ndim: dims.len() as u32,
        }
    }

    pub fn numel(&self) -> usize {
        self.dims.iter().map(|&d| d as usize).product()
    }

    pub fn strides(&self) -> [u32; 4] {
        let mut strides = [1u32; 4];
        for i in (0..3).rev() {
            strides[i] = strides[i + 1] * self.dims[i + 1];
        }
        strides
    }

    // Convenience constructors
    pub fn scalar() -> Self { Self::new(&[]) }
    pub fn vec(n: usize) -> Self { Self::new(&[n]) }
    pub fn matrix(m: usize, n: usize) -> Self { Self::new(&[m, n]) }
    pub fn tensor3(a: usize, b: usize, c: usize) -> Self { Self::new(&[a, b, c]) }
    pub fn tensor4(a: usize, b: usize, c: usize, d: usize) -> Self { Self::new(&[a, b, c, d]) }
}

// GPU uniform representation
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ShapeUniform {
    pub dims: [u32; 4],
    pub strides: [u32; 4],
}

impl From<&Shape> for ShapeUniform {
    fn from(shape: &Shape) -> Self {
        Self {
            dims: shape.dims,
            strides: shape.strides(),
        }
    }
}
```

### TensorStorage

Storage backend abstraction:

```rust
pub enum TensorStorage {
    /// GPU buffer storage
    Gpu(GpuStorage),
    /// CPU memory storage (for fallback or staging)
    Cpu(CpuStorage),
}

pub struct GpuStorage {
    pub buffer: wgpu::Buffer,
    pub size_bytes: usize,
}

pub struct CpuStorage {
    pub data: Vec<u8>,
}

impl TensorStorage {
    pub fn size_bytes(&self) -> usize {
        match self {
            TensorStorage::Gpu(g) => g.size_bytes,
            TensorStorage::Cpu(c) => c.data.len(),
        }
    }

    pub fn is_gpu(&self) -> bool {
        matches!(self, TensorStorage::Gpu(_))
    }
}
```

### Tensor

Main tensor type:

```rust
pub struct Tensor {
    /// Shape of the tensor
    shape: Shape,
    /// Data type of elements
    dtype: DType,
    /// Underlying storage (GPU or CPU)
    storage: TensorStorage,
    /// Device this tensor lives on
    device: DeviceId,
    /// Optional debug name
    name: Option<String>,
}

impl Tensor {
    /// Create new zero-initialized GPU tensor
    pub fn zeros(device: &WebGpuDevice, shape: Shape, dtype: DType) -> Self {
        let size_bytes = shape.numel() * dtype.size_bytes();
        let buffer = device.create_buffer(size_bytes, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        Self {
            shape,
            dtype,
            storage: TensorStorage::Gpu(GpuStorage { buffer, size_bytes }),
            device: device.id(),
            name: None,
        }
    }

    /// Create tensor from CPU data
    pub fn from_slice<T: bytemuck::Pod>(device: &WebGpuDevice, data: &[T], shape: Shape, dtype: DType) -> Self {
        let bytes = bytemuck::cast_slice(data);
        let buffer = device.create_buffer_init(bytes, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        Self {
            shape,
            dtype,
            storage: TensorStorage::Gpu(GpuStorage { buffer, size_bytes: bytes.len() }),
            device: device.id(),
            name: None,
        }
    }

    /// Read tensor data back to CPU
    pub async fn to_vec<T: bytemuck::Pod>(&self, device: &WebGpuDevice) -> Vec<T> {
        match &self.storage {
            TensorStorage::Gpu(g) => device.read_buffer(&g.buffer).await,
            TensorStorage::Cpu(c) => bytemuck::cast_slice(&c.data).to_vec(),
        }
    }

    /// Get underlying GPU buffer (panics if CPU storage)
    pub fn buffer(&self) -> &wgpu::Buffer {
        match &self.storage {
            TensorStorage::Gpu(g) => &g.buffer,
            TensorStorage::Cpu(_) => panic!("Tensor is on CPU, not GPU"),
        }
    }

    // Accessors
    pub fn shape(&self) -> &Shape { &self.shape }
    pub fn dtype(&self) -> DType { self.dtype }
    pub fn numel(&self) -> usize { self.shape.numel() }
    pub fn size_bytes(&self) -> usize { self.storage.size_bytes() }
}
```

## Device Abstraction

### DeviceId

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceId {
    Cpu,
    Gpu(u32),  // GPU index
}
```

### WebGpuDevice

Core GPU device wrapper:

```rust
pub struct WebGpuDevice {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Cached compute pipelines by shader hash
    pipeline_cache: parking_lot::RwLock<HashMap<u64, Arc<wgpu::ComputePipeline>>>,
    /// Device capabilities
    limits: wgpu::Limits,
    id: DeviceId,
}

impl WebGpuDevice {
    /// Initialize WebGPU device (browser or native)
    pub async fn new() -> Result<Self, DeviceError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(DeviceError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("ML Compute Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(DeviceError::RequestDevice)?;

        Ok(Self {
            limits: device.limits(),
            device,
            queue,
            pipeline_cache: parking_lot::RwLock::new(HashMap::new()),
            id: DeviceId::Gpu(0),
        })
    }

    /// Create buffer with given usage
    pub fn create_buffer(&self, size: usize, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size as u64,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Create buffer initialized with data
    pub fn create_buffer_init(&self, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: data,
            usage,
        })
    }

    /// Read buffer contents back to CPU (async)
    pub async fn read_buffer<T: bytemuck::Pod>(&self, buffer: &wgpu::Buffer) -> Vec<T> {
        let size = buffer.size() as usize;
        let staging = self.create_buffer(size, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);

        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size as u64);
        self.queue.submit([encoder.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        rx.await.unwrap().unwrap();

        let data = slice.get_mapped_range();
        bytemuck::cast_slice(&data).to_vec()
    }

    /// Get or create compute pipeline for shader
    pub fn get_pipeline(&self, shader_source: &str, entry_point: &str) -> Arc<wgpu::ComputePipeline> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        shader_source.hash(&mut hasher);
        entry_point.hash(&mut hasher);
        let key = hasher.finish();

        // Check cache
        if let Some(pipeline) = self.pipeline_cache.read().get(&key) {
            return Arc::clone(pipeline);
        }

        // Create new pipeline
        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = Arc::new(self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,  // Auto layout
            module: &shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        }));

        self.pipeline_cache.write().insert(key, Arc::clone(&pipeline));
        pipeline
    }

    /// Execute compute pass
    pub fn dispatch(&self, pipeline: &wgpu::ComputePipeline, bind_group: &wgpu::BindGroup, workgroups: [u32; 3]) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }
        self.queue.submit([encoder.finish()]);
    }

    pub fn id(&self) -> DeviceId { self.id }
    pub fn limits(&self) -> &wgpu::Limits { &self.limits }
}
```

## Memory Management

### Buffer Pool

Reuse GPU buffers to reduce allocation overhead:

```rust
pub struct BufferPool {
    /// Free buffers organized by size bucket
    free_buffers: parking_lot::Mutex<HashMap<usize, Vec<wgpu::Buffer>>>,
    /// Size buckets (powers of 2)
    bucket_sizes: Vec<usize>,
}

impl BufferPool {
    pub fn new() -> Self {
        // Size buckets from 4KB to 128MB
        let bucket_sizes: Vec<usize> = (12..=27).map(|p| 1 << p).collect();
        Self {
            free_buffers: parking_lot::Mutex::new(HashMap::new()),
            bucket_sizes,
        }
    }

    fn bucket_for_size(&self, size: usize) -> usize {
        *self.bucket_sizes.iter().find(|&&b| b >= size).unwrap_or(&size)
    }

    pub fn acquire(&self, device: &WebGpuDevice, size: usize, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        let bucket = self.bucket_for_size(size);
        let mut free = self.free_buffers.lock();

        if let Some(buffers) = free.get_mut(&bucket) {
            if let Some(buffer) = buffers.pop() {
                return buffer;
            }
        }

        device.create_buffer(bucket, usage)
    }

    pub fn release(&self, buffer: wgpu::Buffer) {
        let size = buffer.size() as usize;
        let bucket = self.bucket_for_size(size);
        self.free_buffers.lock().entry(bucket).or_default().push(buffer);
    }
}
```

### Weight Manager

LRU cache for model weights:

```rust
pub struct WeightManager {
    /// Loaded weights by name
    weights: parking_lot::RwLock<HashMap<String, Arc<Tensor>>>,
    /// LRU order (most recent at back)
    lru_order: parking_lot::Mutex<Vec<String>>,
    /// Maximum memory usage
    max_memory: usize,
    /// Current memory usage
    current_memory: AtomicUsize,
}

impl WeightManager {
    pub fn new(max_memory_mb: usize) -> Self {
        Self {
            weights: parking_lot::RwLock::new(HashMap::new()),
            lru_order: parking_lot::Mutex::new(Vec::new()),
            max_memory: max_memory_mb * 1024 * 1024,
            current_memory: AtomicUsize::new(0),
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<Tensor>> {
        let weights = self.weights.read();
        if let Some(tensor) = weights.get(name) {
            // Update LRU order
            let mut order = self.lru_order.lock();
            order.retain(|n| n != name);
            order.push(name.to_string());
            return Some(Arc::clone(tensor));
        }
        None
    }

    pub fn insert(&self, name: String, tensor: Tensor) {
        let size = tensor.size_bytes();

        // Evict if necessary
        while self.current_memory.load(Ordering::Relaxed) + size > self.max_memory {
            if !self.evict_one() {
                break;
            }
        }

        let tensor = Arc::new(tensor);
        self.weights.write().insert(name.clone(), tensor);
        self.current_memory.fetch_add(size, Ordering::Relaxed);
        self.lru_order.lock().push(name);
    }

    fn evict_one(&self) -> bool {
        let mut order = self.lru_order.lock();
        if let Some(name) = order.first().cloned() {
            order.remove(0);
            drop(order);

            if let Some(tensor) = self.weights.write().remove(&name) {
                self.current_memory.fetch_sub(tensor.size_bytes(), Ordering::Relaxed);
                return true;
            }
        }
        false
    }
}
```

## Safetensors Loading

### SafetensorsLoader

Stream model weights via HTTP range requests:

```rust
pub struct SafetensorsLoader {
    /// Base URL for model files
    base_url: String,
    /// Cached metadata
    metadata: Option<SafetensorsMetadata>,
}

#[derive(Debug, Clone)]
pub struct TensorMetadata {
    pub dtype: DType,
    pub shape: Vec<usize>,
    pub data_offsets: (usize, usize),  // Start, end byte offsets
}

#[derive(Debug, Clone)]
pub struct SafetensorsMetadata {
    pub header_size: usize,
    pub tensors: HashMap<String, TensorMetadata>,
}

impl SafetensorsLoader {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            metadata: None,
        }
    }

    /// Parse safetensors header (first 8 bytes = header size, then JSON)
    pub async fn load_metadata(&mut self) -> Result<&SafetensorsMetadata, LoadError> {
        if self.metadata.is_some() {
            return Ok(self.metadata.as_ref().unwrap());
        }

        // Fetch first 8 bytes to get header size
        let header_size_bytes = self.fetch_range(0, 8).await?;
        let header_size = u64::from_le_bytes(header_size_bytes.try_into().unwrap()) as usize;

        // Fetch header JSON
        let header_bytes = self.fetch_range(8, 8 + header_size).await?;
        let header: serde_json::Value = serde_json::from_slice(&header_bytes)?;

        let mut tensors = HashMap::new();
        if let serde_json::Value::Object(map) = header {
            for (name, info) in map {
                if name == "__metadata__" { continue; }

                let dtype = match info["dtype"].as_str().unwrap_or("F32") {
                    "F32" => DType::F32,
                    "F16" => DType::F16,
                    "BF16" => DType::BF16,
                    "I32" => DType::I32,
                    _ => DType::F32,
                };

                let shape: Vec<usize> = info["shape"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize)
                    .collect();

                let offsets = info["data_offsets"].as_array().unwrap();
                let start = offsets[0].as_u64().unwrap() as usize;
                let end = offsets[1].as_u64().unwrap() as usize;

                tensors.insert(name, TensorMetadata {
                    dtype,
                    shape,
                    data_offsets: (start + 8 + header_size, end + 8 + header_size),
                });
            }
        }

        self.metadata = Some(SafetensorsMetadata { header_size, tensors });
        Ok(self.metadata.as_ref().unwrap())
    }

    /// Load a specific tensor by name
    pub async fn load_tensor(&mut self, device: &WebGpuDevice, name: &str) -> Result<Tensor, LoadError> {
        let metadata = self.load_metadata().await?;
        let tensor_meta = metadata.tensors.get(name)
            .ok_or_else(|| LoadError::TensorNotFound(name.to_string()))?;

        let (start, end) = tensor_meta.data_offsets;
        let data = self.fetch_range(start, end).await?;

        let shape = Shape::new(&tensor_meta.shape);
        Ok(Tensor::from_slice(device, &data, shape, tensor_meta.dtype))
    }

    /// Fetch byte range from URL
    async fn fetch_range(&self, start: usize, end: usize) -> Result<Vec<u8>, LoadError> {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use wasm_bindgen_futures::JsFuture;
            use web_sys::{Request, RequestInit, Response};

            let opts = RequestInit::new();
            opts.set_method("GET");

            let headers = web_sys::Headers::new().unwrap();
            headers.set("Range", &format!("bytes={}-{}", start, end - 1)).unwrap();
            opts.set_headers(&headers);

            let request = Request::new_with_str_and_init(&self.base_url, &opts).unwrap();
            let window = web_sys::window().unwrap();
            let resp: Response = JsFuture::from(window.fetch_with_request(&request))
                .await
                .unwrap()
                .dyn_into()
                .unwrap();

            let buffer = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
            let array = js_sys::Uint8Array::new(&buffer);
            Ok(array.to_vec())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let client = reqwest::Client::new();
            let resp = client
                .get(&self.base_url)
                .header("Range", format!("bytes={}-{}", start, end - 1))
                .send()
                .await?;
            Ok(resp.bytes().await?.to_vec())
        }
    }
}
```

## Browser Constraints

| Constraint | Limit | Notes |
|------------|-------|-------|
| Max workgroup size | 256 total invocations | 16x16x1 or 256x1x1 |
| Max workgroups per dimension | 65535 | Split large dispatches |
| Max storage buffer | 128 MB | Most browsers |
| Max bind groups | 4 | Plan layouts carefully |
| No tokio | - | Use spawn_local |
| No std::time::Instant | - | Use web_time::Instant |
| Single thread | - | Main thread only in WASM |

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum MlError {
    #[error("Device error: {0}")]
    Device(#[from] DeviceError),

    #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch { expected: Shape, actual: Shape },

    #[error("DType mismatch: expected {expected:?}, got {actual:?}")]
    DTypeMismatch { expected: DType, actual: DType },

    #[error("Model load error: {0}")]
    Load(#[from] LoadError),

    #[error("Compute error: {0}")]
    Compute(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("No WebGPU adapter available")]
    NoAdapter,

    #[error("Failed to request device: {0}")]
    RequestDevice(wgpu::RequestDeviceError),
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Tensor not found: {0}")]
    TensorNotFound(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),
}
```
