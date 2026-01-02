use crate::storage::WgpuStorage;
use candle_core::backend::BackendDevice;
use candle_core::cpu_backend::CpuDevice;
use candle_core::{CpuStorage, DeviceLocation, DType, Error, Result, Shape};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct WgpuContext {
    #[allow(dead_code)]
    device: wgpu::Device,
    #[allow(dead_code)]
    queue: wgpu::Queue,
    #[allow(dead_code)]
    limits: wgpu::Limits,
    pipelines: Mutex<HashMap<String, wgpu::ComputePipeline>>,
}

#[derive(Clone)]
pub struct WgpuDevice {
    context: Option<Arc<WgpuContext>>,
}

impl std::fmt::Debug for WgpuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuDevice")
            .field("has_context", &self.context.is_some())
            .finish()
    }
}

impl WgpuDevice {
    pub fn new() -> Result<Self> {
        let context = Some(Arc::new(init_wgpu()?));
        Ok(Self { context })
    }

    pub fn new_cpu_fallback() -> Self {
        Self { context: None }
    }

    pub fn has_wgpu(&self) -> bool {
        self.context.is_some()
    }

    pub fn as_candle_device(&self) -> candle_core::Device {
        candle_core::Device::Cpu
    }

    pub fn wgpu_device(&self) -> Option<&wgpu::Device> {
        self.context.as_ref().map(|ctx| &ctx.device)
    }

    pub fn wgpu_queue(&self) -> Option<&wgpu::Queue> {
        self.context.as_ref().map(|ctx| &ctx.queue)
    }

    pub(crate) fn pipeline<F>(&self, key: &str, build: F) -> Option<wgpu::ComputePipeline>
    where
        F: FnOnce(&wgpu::Device) -> wgpu::ComputePipeline,
    {
        let context = self.context.as_ref()?;
        if let Ok(cache) = context.pipelines.lock() {
            if let Some(pipeline) = cache.get(key) {
                return Some(pipeline.clone());
            }
        }

        let pipeline = build(&context.device);
        if let Ok(mut cache) = context.pipelines.lock() {
            cache
                .entry(key.to_string())
                .or_insert_with(|| pipeline.clone());
        }
        Some(pipeline)
    }
}

impl BackendDevice for WgpuDevice {
    type Storage = WgpuStorage;

    fn new(_: usize) -> Result<Self> {
        Self::new()
    }

    fn location(&self) -> DeviceLocation {
        DeviceLocation::Cpu
    }

    fn same_device(&self, other: &Self) -> bool {
        match (&self.context, &other.context) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }

    fn zeros_impl(&self, shape: &Shape, dtype: DType) -> Result<Self::Storage> {
        let cpu = <CpuDevice as BackendDevice>::zeros_impl(&CpuDevice, shape, dtype)?;
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    fn ones_impl(&self, shape: &Shape, dtype: DType) -> Result<Self::Storage> {
        let cpu = <CpuDevice as BackendDevice>::ones_impl(&CpuDevice, shape, dtype)?;
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    unsafe fn alloc_uninit(&self, shape: &Shape, dtype: DType) -> Result<Self::Storage> {
        let cpu = unsafe { <CpuDevice as BackendDevice>::alloc_uninit(&CpuDevice, shape, dtype)? };
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    fn storage_from_slice<T: candle_core::WithDType>(&self, data: &[T]) -> Result<Self::Storage> {
        let cpu = <CpuDevice as BackendDevice>::storage_from_slice(&CpuDevice, data)?;
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    fn storage_from_cpu_storage(&self, storage: &CpuStorage) -> Result<Self::Storage> {
        Ok(WgpuStorage::from_cpu(self.clone(), storage.clone()))
    }

    fn storage_from_cpu_storage_owned(&self, storage: CpuStorage) -> Result<Self::Storage> {
        Ok(WgpuStorage::from_cpu(self.clone(), storage))
    }

    fn rand_uniform(&self, shape: &Shape, dtype: DType, lo: f64, up: f64) -> Result<Self::Storage> {
        let cpu = <CpuDevice as BackendDevice>::rand_uniform(&CpuDevice, shape, dtype, lo, up)?;
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    fn rand_normal(
        &self,
        shape: &Shape,
        dtype: DType,
        mean: f64,
        std: f64,
    ) -> Result<Self::Storage> {
        let cpu = <CpuDevice as BackendDevice>::rand_normal(&CpuDevice, shape, dtype, mean, std)?;
        Ok(WgpuStorage::from_cpu(self.clone(), cpu))
    }

    fn set_seed(&self, seed: u64) -> Result<()> {
        <CpuDevice as BackendDevice>::set_seed(&CpuDevice, seed)
    }

    fn synchronize(&self) -> Result<()> {
        Ok(())
    }
}

fn init_wgpu() -> Result<WgpuContext> {
    let backends = if cfg!(target_arch = "wasm32") {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    } else {
        wgpu::Backends::all()
    };

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok_or_else(|| Error::Msg("No WebGPU adapter available".to_string()))?;

    let limits = adapter.limits();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("candle-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: limits.clone(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .map_err(|e| Error::Msg(format!("Failed to create WebGPU device: {e:?}")))?;

    Ok(WgpuContext {
        device,
        queue,
        limits,
        pipelines: Mutex::new(HashMap::new()),
    })
}
