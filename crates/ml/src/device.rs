use crate::error::Result;

#[derive(Debug, Clone)]
pub enum MlDevice {
    #[cfg(feature = "native")]
    Candle(candle_core::Device),

    #[cfg(feature = "browser")]
    WebGpu(candle_wgpu::WgpuDevice),

    Cpu,
}

impl MlDevice {
    pub async fn best_available() -> Result<Self> {
        #[cfg(feature = "native")]
        {
            if candle_core::utils::cuda_is_available() {
                if let Ok(device) = candle_core::Device::new_cuda(0) {
                    return Ok(MlDevice::Candle(device));
                }
            }

            if candle_core::utils::metal_is_available() {
                if let Ok(device) = candle_core::Device::new_metal(0) {
                    return Ok(MlDevice::Candle(device));
                }
            }
        }

        #[cfg(feature = "browser")]
        {
            if let Ok(device) = candle_wgpu::WgpuDevice::new() {
                return Ok(MlDevice::WebGpu(device));
            }
        }

        Ok(MlDevice::Cpu)
    }

    pub fn candle_device(&self) -> candle_core::Device {
        match self {
            #[cfg(feature = "native")]
            MlDevice::Candle(device) => device.clone(),
            #[cfg(feature = "browser")]
            MlDevice::WebGpu(_) => candle_core::Device::Cpu,
            MlDevice::Cpu => candle_core::Device::Cpu,
        }
    }

    #[cfg(feature = "browser")]
    pub fn wgpu_device(&self) -> Option<&candle_wgpu::WgpuDevice> {
        match self {
            MlDevice::WebGpu(device) => Some(device),
            _ => None,
        }
    }

    pub fn is_gpu_accelerated(&self) -> bool {
        match self {
            #[cfg(feature = "native")]
            MlDevice::Candle(device) => device.is_cuda() || device.is_metal(),
            #[cfg(feature = "browser")]
            MlDevice::WebGpu(device) => device.has_wgpu(),
            MlDevice::Cpu => false,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            #[cfg(feature = "native")]
            MlDevice::Candle(device) => {
                if device.is_cuda() {
                    "candle-cuda"
                } else if device.is_metal() {
                    "candle-metal"
                } else {
                    "candle-cpu"
                }
            }
            #[cfg(feature = "browser")]
            MlDevice::WebGpu(_) => "candle-wgpu",
            MlDevice::Cpu => "cpu",
        }
    }
}

impl Default for MlDevice {
    fn default() -> Self {
        MlDevice::Cpu
    }
}

impl From<candle_core::Device> for MlDevice {
    fn from(_device: candle_core::Device) -> Self {
        #[cfg(feature = "native")]
        {
            return MlDevice::Candle(_device);
        }

        #[allow(unreachable_code)]
        MlDevice::Cpu
    }
}

impl From<MlDevice> for candle_core::Device {
    fn from(device: MlDevice) -> Self {
        device.candle_device()
    }
}

impl From<&MlDevice> for candle_core::Device {
    fn from(device: &MlDevice) -> Self {
        device.candle_device()
    }
}
