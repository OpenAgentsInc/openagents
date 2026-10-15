//! Float32 device-resident operations for bounded encoder and digit graphs.
use metal::{
    Buffer, CommandBuffer, CommandQueue, CompileOptions, ComputeCommandEncoder,
    ComputePipelineState, Device, MTLResourceOptions, MTLSize,
};
use std::{collections::BTreeMap, ffi::c_void};

pub struct ResidentMetal {
    device: Device,
    queue: CommandQueue,
    pipelines: BTreeMap<&'static str, ComputePipelineState>,
}
pub struct ResidentBuffer {
    raw: Buffer,
    bytes: usize,
    _owner: std::marker::PhantomData<std::rc::Rc<()>>,
}
pub struct ResidentBatch<'a> {
    runtime: &'a ResidentMetal,
    encoder: ComputeCommandEncoder,
    command: CommandBuffer,
}
fn size(x: usize, y: usize, z: usize) -> MTLSize {
    MTLSize::new(x as u64, y as u64, z as u64)
}
fn dims(n: usize, h: usize) -> Result<(), String> {
    if !(1..=512).contains(&n) || !(1..=16384).contains(&h) {
        Err("Unsupported native Metal dimensions".into())
    } else {
        Ok(())
    }
}
impl ResidentMetal {
    pub fn new() -> Result<Self, String> {
        let device = Device::system_default().ok_or("Metal is unavailable")?;
        let options = CompileOptions::new();
        options.set_fast_math_enabled(false);
        let library =
            device.new_library_with_source(include_str!("kernels/small_models.metal"), &options)?;
        let mut pipelines = BTreeMap::new();
        for name in [
            "small_embed",
            "small_norm",
            "small_matmul",
            "small_rope",
            "small_attention",
            "small_geglu",
            "small_add",
            "small_bias",
            "small_digit_conv_pool",
            "small_digit_head",
        ] {
            let function = library.get_function(name, None)?;
            let pipeline = device.new_compute_pipeline_state_with_function(&function)?;
            if pipeline.max_total_threads_per_threadgroup() < 256 {
                return Err("Native Metal small models require 256 threads per group".into());
            }
            if name == "small_attention" && pipeline.thread_execution_width() != 32 {
                return Err("Native Metal attention requires a 32-thread SIMD group".into());
            }
            pipelines.insert(name, pipeline);
        }
        let queue = device.new_command_queue();
        Ok(Self {
            device,
            queue,
            pipelines,
        })
    }
    pub fn device_name(&self) -> &str {
        self.device.name()
    }
    pub fn buffer(&self, bytes: usize) -> ResidentBuffer {
        let bytes = bytes.max(16);
        let raw = self
            .device
            .new_buffer(bytes as u64, MTLResourceOptions::StorageModeShared);
        // SAFETY: the shared allocation contains exactly `bytes` writable bytes.
        unsafe {
            std::ptr::write_bytes(raw.contents().cast::<u8>(), 0, bytes);
        }
        ResidentBuffer {
            raw,
            bytes,
            _owner: std::marker::PhantomData,
        }
    }
    pub fn bytes(&self, values: &[u8]) -> Result<ResidentBuffer, String> {
        let buffer = self.buffer(values.len());
        buffer.write_bytes(values)?;
        Ok(buffer)
    }
    pub fn f32s(&self, values: &[f32]) -> Result<ResidentBuffer, String> {
        let buffer = self.buffer(std::mem::size_of_val(values));
        buffer.write_f32(values)?;
        Ok(buffer)
    }
    pub fn batch(&self) -> ResidentBatch<'_> {
        let (command, encoder) = objc::rc::autoreleasepool(|| {
            let command = self.queue.new_command_buffer().to_owned();
            // A serial encoder orders every dependent operation without host waits.
            let encoder = command.new_compute_command_encoder().to_owned();
            (command, encoder)
        });
        ResidentBatch {
            runtime: self,
            command,
            encoder,
        }
    }
}
impl ResidentBuffer {
    fn check(&self, bytes: usize) -> Result<(), String> {
        if bytes <= self.bytes {
            Ok(())
        } else {
            Err("Native Metal buffer is too small".into())
        }
    }
    fn f32s(&self, count: usize) -> Result<(), String> {
        self.check(count.checked_mul(4).ok_or("Metal buffer size overflow")?)
    }
    pub fn write_bytes(&self, values: &[u8]) -> Result<(), String> {
        self.check(values.len())?;
        // SAFETY: checked range in a shared allocation; the caller owns the idle model.
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr(),
                self.raw.contents().cast::<u8>(),
                values.len(),
            );
        }
        Ok(())
    }
    pub fn write_f32(&self, values: &[f32]) -> Result<(), String> {
        // SAFETY: floats have no padding; the byte slice covers the complete input.
        self.write_bytes(unsafe {
            std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
        })
    }
    pub fn write_i32(&self, values: &[i32]) -> Result<(), String> {
        // SAFETY: integers have no padding; the byte slice covers the complete input.
        self.write_bytes(unsafe {
            std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
        })
    }
    pub fn read_f32(&self, count: usize) -> Result<Vec<f32>, String> {
        self.f32s(count)?;
        let mut out = vec![0.0; count];
        // SAFETY: checked shared range; the model waits for completion before reading.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.raw.contents().cast::<f32>(),
                out.as_mut_ptr(),
                count,
            );
        }
        Ok(out)
    }
}
impl ResidentBatch<'_> {
    fn dispatch(
        &self,
        name: &str,
        buffers: &[&ResidentBuffer],
        args: &[u32],
        floats: &[f32],
        groups: MTLSize,
        threads: MTLSize,
    ) -> Result<(), String> {
        let pipeline = self
            .runtime
            .pipelines
            .get(name)
            .ok_or("Metal operation is unavailable")?;
        self.encoder.set_compute_pipeline_state(pipeline);
        for (i, buffer) in buffers.iter().enumerate() {
            self.encoder.set_buffer(i as u64, Some(&buffer.raw), 0);
        }
        if !args.is_empty() {
            self.encoder.set_bytes(
                8,
                std::mem::size_of_val(args) as u64,
                args.as_ptr().cast::<c_void>(),
            );
        }
        if !floats.is_empty() {
            self.encoder.set_bytes(
                9,
                std::mem::size_of_val(floats) as u64,
                floats.as_ptr().cast::<c_void>(),
            );
        }
        self.encoder.dispatch_thread_groups(groups, threads);
        Ok(())
    }
    pub fn commit_wait(self) -> Result<(), String> {
        self.encoder.end_encoding();
        self.command.commit();
        self.command.wait_until_completed();
        if self.command.status() == metal::MTLCommandBufferStatus::Completed {
            Ok(())
        } else {
            Err(format!(
                "Native Metal command failed: {:?}",
                self.command.status()
            ))
        }
    }
    pub fn embed(
        &self,
        table: &ResidentBuffer,
        ids: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        h: usize,
        vocab: usize,
        zero: u8,
        scale: f32,
    ) -> Result<(), String> {
        dims(n, h)?;
        if !(1..=1_000_000).contains(&vocab) {
            return Err("Unsupported vocabulary".into());
        }
        table.check(vocab * h)?;
        ids.f32s(n)?;
        out.f32s(n * h)?;
        self.dispatch(
            "small_embed",
            &[table, ids, out],
            &[n as u32, h as u32, vocab as u32, zero as u32],
            &[scale],
            size((n * h).div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn norm(
        &self,
        input: &ResidentBuffer,
        weight: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        h: usize,
        epsilon: f32,
    ) -> Result<(), String> {
        dims(n, h)?;
        input.f32s(n * h)?;
        weight.f32s(h)?;
        out.f32s(n * h)?;
        self.dispatch(
            "small_norm",
            &[input, weight, out],
            &[n as u32, h as u32],
            &[epsilon],
            size(n, 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn matmul(
        &self,
        a: &ResidentBuffer,
        b: &ResidentBuffer,
        out: &ResidentBuffer,
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<(), String> {
        dims(m, k)?;
        dims(1, n)?;
        a.f32s(m * k)?;
        b.f32s(k * n)?;
        out.f32s(m * n)?;
        self.dispatch(
            "small_matmul",
            &[a, b, out],
            &[m as u32, k as u32, n as u32],
            &[],
            size(n.div_ceil(16), m.div_ceil(16), 1),
            size(16, 16, 1),
        )
    }
    pub fn rope(
        &self,
        input: &ResidentBuffer,
        freq: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        heads: usize,
        d: usize,
    ) -> Result<(), String> {
        dims(n, heads.checked_mul(d).ok_or("Metal head size overflow")?)?;
        if d % 2 != 0 || d == 0 {
            return Err("Unsupported rotary dimensions".into());
        }
        input.f32s(3 * n * heads * d)?;
        freq.f32s(d / 2)?;
        out.f32s(3 * n * heads * d)?;
        self.dispatch(
            "small_rope",
            &[input, freq, out],
            &[n as u32, heads as u32, d as u32],
            &[],
            size((n * heads * d).div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn attention(
        &self,
        qkv: &ResidentBuffer,
        mask: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        heads: usize,
        d: usize,
        radius: Option<usize>,
        queries: usize,
    ) -> Result<(), String> {
        dims(n, heads.checked_mul(d).ok_or("Metal head size overflow")?)?;
        if queries == 0 || queries > n || d == 0 || d > 256 || radius.is_some_and(|r| r > 512) {
            return Err("Unsupported attention dimensions".into());
        }
        qkv.f32s(3 * n * heads * d)?;
        mask.f32s(n)?;
        out.f32s(queries * heads * d)?;
        self.dispatch(
            "small_attention",
            &[qkv, mask, out],
            &[
                n as u32,
                heads as u32,
                d as u32,
                radius.map_or(u32::MAX, |r| r as u32),
            ],
            &[],
            size(queries, heads, 1),
            size(256, 1, 1),
        )
    }
    pub fn geglu(
        &self,
        input: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        f: usize,
    ) -> Result<(), String> {
        dims(n, f)?;
        input.f32s(2 * n * f)?;
        out.f32s(n * f)?;
        self.dispatch(
            "small_geglu",
            &[input, out],
            &[n as u32, f as u32],
            &[],
            size((n * f).div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn add(
        &self,
        input: &ResidentBuffer,
        out: &ResidentBuffer,
        count: usize,
    ) -> Result<(), String> {
        if count == 0 || count > 512 * 16384 {
            return Err("Unsupported addition size".into());
        }
        input.f32s(count)?;
        out.f32s(count)?;
        self.dispatch(
            "small_add",
            &[input, out],
            &[count as u32],
            &[],
            size(count.div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn bias(
        &self,
        out: &ResidentBuffer,
        bias: &ResidentBuffer,
        count: usize,
    ) -> Result<(), String> {
        dims(1, count)?;
        out.f32s(count)?;
        bias.f32s(count)?;
        self.dispatch(
            "small_bias",
            &[out, bias],
            &[count as u32],
            &[],
            size(count.div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn digit_conv_pool(
        &self,
        input: &ResidentBuffer,
        w: &ResidentBuffer,
        bias: &ResidentBuffer,
        out: &ResidentBuffer,
        channels: usize,
        width: usize,
        outputs: usize,
        pool: usize,
    ) -> Result<(), String> {
        if !(1..=64).contains(&channels)
            || !(1..=64).contains(&outputs)
            || !(1..=128).contains(&width)
            || !(1..=4).contains(&pool)
            || pool > width
        {
            return Err("Unsupported digit dimensions".into());
        }
        let next = width / pool;
        input.f32s(channels * width * width)?;
        w.f32s(outputs * channels * 25)?;
        bias.f32s(outputs)?;
        out.f32s(outputs * next * next)?;
        self.dispatch(
            "small_digit_conv_pool",
            &[input, w, bias, out],
            &[channels as u32, width as u32, outputs as u32, pool as u32],
            &[],
            size((outputs * next * next).div_ceil(256), 1, 1),
            size(256, 1, 1),
        )
    }
    pub fn digit_head(
        &self,
        input: &ResidentBuffer,
        w: &ResidentBuffer,
        bias: &ResidentBuffer,
        out: &ResidentBuffer,
    ) -> Result<(), String> {
        input.f32s(256)?;
        w.f32s(2560)?;
        bias.f32s(10)?;
        out.f32s(10)?;
        self.dispatch(
            "small_digit_head",
            &[input, w, bias, out],
            &[],
            &[],
            size(1, 1, 1),
            size(256, 1, 1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an Apple GPU"]
    fn tiled_float32_matrices_match_scalar_at_all_tile_edges() {
        let gpu = ResidentMetal::new().unwrap();
        for (m, k, n) in [(1, 1, 1), (3, 17, 22), (17, 19, 31), (33, 65, 17)] {
            let a: Vec<_> = (0..m * k)
                .map(|i| ((i * 7 % 31) as f32 - 15.0) / 17.0)
                .collect();
            let b: Vec<_> = (0..k * n)
                .map(|i| ((i * 13 % 37) as f32 - 18.0) / 19.0)
                .collect();
            let left = gpu.f32s(&a).unwrap();
            let right = gpu.f32s(&b).unwrap();
            let out = gpu.buffer(m * n * 4);
            let s = gpu.batch();
            s.matmul(&left, &right, &out, m, k, n).unwrap();
            s.commit_wait().unwrap();
            let values = out.read_f32(m * n).unwrap();
            for row in 0..m {
                for col in 0..n {
                    let expected: f64 = (0..k)
                        .map(|i| f64::from(a[row * k + i]) * f64::from(b[i * n + col]))
                        .sum();
                    assert!((f64::from(values[row * n + col]) - expected).abs() < 1e-4);
                }
            }
        }
    }
    #[test]
    #[ignore = "requires an Apple GPU"]
    fn normalization_supports_in_place_and_rejects_short_buffers() {
        let gpu = ResidentMetal::new().unwrap();
        let h = 67;
        let n = 3;
        let input: Vec<_> = (0..n * h)
            .map(|i| ((i * 11 % 43) as f32 - 21.0) / 17.0)
            .collect();
        let weights: Vec<_> = (0..h).map(|i| 0.7 + i as f32 / 100.0).collect();
        let x = gpu.f32s(&input).unwrap();
        let w = gpu.f32s(&weights).unwrap();
        let s = gpu.batch();
        assert!(s.norm(&x, &w, &gpu.buffer(16), n, h, 1e-5).is_err());
        s.norm(&x, &w, &x, n, h, 1e-5).unwrap();
        s.commit_wait().unwrap();
        for (row, actual) in input.chunks(h).zip(x.read_f32(n * h).unwrap().chunks(h)) {
            let mean = row.iter().map(|v| f64::from(*v)).sum::<f64>() / h as f64;
            let variance = row
                .iter()
                .map(|v| (f64::from(*v) - mean).powi(2))
                .sum::<f64>()
                / h as f64;
            for ((x, w), a) in row.iter().zip(&weights).zip(actual) {
                let expected = (f64::from(*x) - mean) / (variance + 1e-5).sqrt() * f64::from(*w);
                assert!((f64::from(*a) - expected).abs() < 1e-5);
            }
        }
    }
}
