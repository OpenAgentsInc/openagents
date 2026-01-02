use crate::device::WgpuDevice;
use crate::ops;
use candle_core::backend::BackendStorage;
use candle_core::op::{BinaryOpT, CmpOp, ReduceOp, UnaryOpT};
use candle_core::{CpuStorage, DType, Layout, Result};

#[derive(Debug, Clone)]
pub struct WgpuStorage {
    cpu: CpuStorage,
    device: WgpuDevice,
}

impl WgpuStorage {
    pub fn from_cpu(device: WgpuDevice, cpu: CpuStorage) -> Self {
        Self { cpu, device }
    }

    pub fn cpu_storage(&self) -> &CpuStorage {
        &self.cpu
    }
}

impl BackendStorage for WgpuStorage {
    type Device = WgpuDevice;

    fn try_clone(&self, layout: &Layout) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::try_clone(&self.cpu, layout)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn dtype(&self) -> DType {
        <CpuStorage as BackendStorage>::dtype(&self.cpu)
    }

    fn device(&self) -> &Self::Device {
        &self.device
    }

    fn to_cpu_storage(&self) -> Result<CpuStorage> {
        Ok(self.cpu.clone())
    }

    fn affine(&self, layout: &Layout, mul: f64, add: f64) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::affine(&self.cpu, layout, mul, add)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn powf(&self, layout: &Layout, alpha: f64) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::powf(&self.cpu, layout, alpha)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn elu(&self, layout: &Layout, alpha: f64) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::elu(&self.cpu, layout, alpha)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn reduce_op(&self, op: ReduceOp, layout: &Layout, dims: &[usize]) -> Result<Self> {
        if let Some(op) = reduce_op_from_reduce(op) {
            if self.device.has_wgpu() && dims_cover_all(dims, layout.dims().len()) {
                if let Some(slice) = contiguous_f32_slice(&self.cpu, layout)? {
                    if let Ok(value) = ops::reduce(&self.device, op, slice) {
                        return Ok(Self::from_cpu(self.device.clone(), CpuStorage::F32(vec![value])));
                    }
                }
            }
        }
        let cpu = <CpuStorage as BackendStorage>::reduce_op(&self.cpu, op, layout, dims)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn cmp(&self, op: CmpOp, rhs: &Self, l: &Layout, r: &Layout) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::cmp(&self.cpu, op, &rhs.cpu, l, r)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn to_dtype(&self, layout: &Layout, dtype: DType) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::to_dtype(&self.cpu, layout, dtype)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn unary_impl<B: UnaryOpT>(&self, layout: &Layout) -> Result<Self> {
        if let Some(op) = unary_op_from_name(B::NAME) {
            if self.device.has_wgpu() {
                if let Some(slice) = contiguous_f32_slice(&self.cpu, layout)? {
                    if let Ok(values) = ops::unary(&self.device, op, slice) {
                        return Ok(Self::from_cpu(self.device.clone(), CpuStorage::F32(values)));
                    }
                }
            }
        }
        let cpu = <CpuStorage as BackendStorage>::unary_impl::<B>(&self.cpu, layout)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn binary_impl<B: BinaryOpT>(&self, rhs: &Self, l: &Layout, r: &Layout) -> Result<Self> {
        if let Some(op) = binary_op_from_name(B::NAME) {
            if self.device.has_wgpu() {
                if let Some(lhs_slice) = contiguous_f32_slice(&self.cpu, l)? {
                    if let Some(rhs_slice) = contiguous_f32_slice(&rhs.cpu, r)? {
                        if let Ok(values) = ops::binary(&self.device, op, lhs_slice, rhs_slice) {
                            return Ok(Self::from_cpu(
                                self.device.clone(),
                                CpuStorage::F32(values),
                            ));
                        }
                    }
                }
            }
        }
        let cpu = <CpuStorage as BackendStorage>::binary_impl::<B>(&self.cpu, &rhs.cpu, l, r)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn where_cond(
        &self,
        cond_l: &Layout,
        t: &Self,
        t_l: &Layout,
        f: &Self,
        f_l: &Layout,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::where_cond(&self.cpu, cond_l, &t.cpu, t_l, &f.cpu, f_l)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn conv1d(
        &self,
        l: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &candle_core::conv::ParamsConv1D,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::conv1d(&self.cpu, l, &kernel.cpu, kernel_l, params)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn conv_transpose1d(
        &self,
        l: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &candle_core::conv::ParamsConvTranspose1D,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::conv_transpose1d(
            &self.cpu,
            l,
            &kernel.cpu,
            kernel_l,
            params,
        )?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn conv2d(
        &self,
        l: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &candle_core::conv::ParamsConv2D,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::conv2d(&self.cpu, l, &kernel.cpu, kernel_l, params)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn conv_transpose2d(
        &self,
        l: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &candle_core::conv::ParamsConvTranspose2D,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::conv_transpose2d(
            &self.cpu,
            l,
            &kernel.cpu,
            kernel_l,
            params,
        )?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn avg_pool2d(&self, l: &Layout, k: (usize, usize), s: (usize, usize)) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::avg_pool2d(&self.cpu, l, k, s)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn max_pool2d(&self, l: &Layout, k: (usize, usize), s: (usize, usize)) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::max_pool2d(&self.cpu, l, k, s)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn upsample_nearest1d(&self, l: &Layout, size: usize) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::upsample_nearest1d(&self.cpu, l, size)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn upsample_nearest2d(&self, l: &Layout, size: usize, size_w: usize) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::upsample_nearest2d(&self.cpu, l, size, size_w)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn gather(&self, l: &Layout, ids: &Self, ids_l: &Layout, dim: usize) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::gather(&self.cpu, l, &ids.cpu, ids_l, dim)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }
    fn scatter_add(
        &self,
        l: &Layout,
        ids: &Self,
        ids_l: &Layout,
        src: &Self,
        src_l: &Layout,
        dim: usize,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::scatter_add(
            &self.cpu,
            l,
            &ids.cpu,
            ids_l,
            &src.cpu,
            src_l,
            dim,
        )?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn index_select(&self, ids: &Self, l: &Layout, ids_l: &Layout, dim: usize) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::index_select(&self.cpu, &ids.cpu, l, ids_l, dim)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn index_add(
        &self,
        l: &Layout,
        ids: &Self,
        ids_l: &Layout,
        src: &Self,
        src_l: &Layout,
        dim: usize,
    ) -> Result<Self> {
        let cpu = <CpuStorage as BackendStorage>::index_add(
            &self.cpu,
            l,
            &ids.cpu,
            ids_l,
            &src.cpu,
            src_l,
            dim,
        )?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn matmul(
        &self,
        rhs: &Self,
        dims: (usize, usize, usize, usize),
        l: &Layout,
        r: &Layout,
    ) -> Result<Self> {
        if self.device.has_wgpu() {
            let (b, m, n, k) = dims;
            if b == 1 {
                if let (Some(lhs_slice), Some(rhs_slice)) =
                    (contiguous_f32_slice(&self.cpu, l)?, contiguous_f32_slice(&rhs.cpu, r)?)
                {
                    if let Ok(values) = ops::matmul(&self.device, lhs_slice, rhs_slice, m, n, k) {
                        return Ok(Self::from_cpu(self.device.clone(), CpuStorage::F32(values)));
                    }
                }
            }
        }
        let cpu = <CpuStorage as BackendStorage>::matmul(&self.cpu, &rhs.cpu, dims, l, r)?;
        Ok(Self::from_cpu(self.device.clone(), cpu))
    }

    fn copy_strided_src(&self, dst: &mut Self, offset: usize, l: &Layout) -> Result<()> {
        <CpuStorage as BackendStorage>::copy_strided_src(&self.cpu, &mut dst.cpu, offset, l)
    }

    fn copy2d(
        &self,
        dst: &mut Self,
        d1: usize,
        d2: usize,
        src_stride1: usize,
        dst_stride1: usize,
        src_offset: usize,
        dst_offset: usize,
    ) -> Result<()> {
        <CpuStorage as BackendStorage>::copy2d(
            &self.cpu,
            &mut dst.cpu,
            d1,
            d2,
            src_stride1,
            dst_stride1,
            src_offset,
            dst_offset,
        )
    }

}

fn unary_op_from_name(name: &str) -> Option<ops::UnaryOp> {
    match name {
        "exp" => Some(ops::UnaryOp::Exp),
        "log" => Some(ops::UnaryOp::Log),
        "relu" => Some(ops::UnaryOp::Relu),
        "silu" => Some(ops::UnaryOp::Silu),
        "gelu" => Some(ops::UnaryOp::Gelu),
        "tanh" => Some(ops::UnaryOp::Tanh),
        _ => None,
    }
}

fn binary_op_from_name(name: &str) -> Option<ops::BinaryOp> {
    match name {
        "add" => Some(ops::BinaryOp::Add),
        "sub" => Some(ops::BinaryOp::Sub),
        "mul" => Some(ops::BinaryOp::Mul),
        "div" => Some(ops::BinaryOp::Div),
        "minimum" => Some(ops::BinaryOp::Minimum),
        "maximum" => Some(ops::BinaryOp::Maximum),
        _ => None,
    }
}

fn reduce_op_from_reduce(op: ReduceOp) -> Option<ops::ReduceOp> {
    match op {
        ReduceOp::Sum => Some(ops::ReduceOp::Sum),
        ReduceOp::Max => Some(ops::ReduceOp::Max),
        ReduceOp::Min => Some(ops::ReduceOp::Min),
        ReduceOp::ArgMax | ReduceOp::ArgMin => None,
    }
}

fn dims_cover_all(dims: &[usize], rank: usize) -> bool {
    if dims.len() != rank {
        return false;
    }
    let mut seen = vec![false; rank];
    for &dim in dims {
        if dim >= rank || seen[dim] {
            return false;
        }
        seen[dim] = true;
    }
    true
}

fn contiguous_f32_slice<'a>(storage: &'a CpuStorage, layout: &Layout) -> Result<Option<&'a [f32]>> {
    let (start, end) = match layout.contiguous_offsets() {
        Some(bounds) => bounds,
        None => return Ok(None),
    };

    match storage {
        CpuStorage::F32(data) => {
            if end <= data.len() {
                Ok(Some(&data[start..end]))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}
