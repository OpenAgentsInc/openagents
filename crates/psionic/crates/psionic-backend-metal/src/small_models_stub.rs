//! Explicit unavailable-device implementation for non-Apple targets.
#![allow(unused_variables)]
pub struct ResidentMetal {
    _private: (),
}
pub struct ResidentBuffer {
    _private: (),
}
pub struct ResidentBatch<'a> {
    _runtime: &'a ResidentMetal,
}
fn unavailable<T>() -> Result<T, String> {
    Err("Native Metal requires macOS".into())
}
impl ResidentMetal {
    pub fn new() -> Result<Self, String> {
        unavailable()
    }
    pub fn device_name(&self) -> &str {
        "unavailable"
    }
    pub fn buffer(&self, bytes: usize) -> ResidentBuffer {
        ResidentBuffer { _private: () }
    }
    pub fn bytes(&self, values: &[u8]) -> Result<ResidentBuffer, String> {
        unavailable()
    }
    pub fn f32s(&self, values: &[f32]) -> Result<ResidentBuffer, String> {
        unavailable()
    }
    pub fn batch(&self) -> ResidentBatch<'_> {
        ResidentBatch { _runtime: self }
    }
}
impl ResidentBuffer {
    pub fn write_bytes(&self, values: &[u8]) -> Result<(), String> {
        unavailable()
    }
    pub fn write_f32(&self, values: &[f32]) -> Result<(), String> {
        unavailable()
    }
    pub fn write_i32(&self, values: &[i32]) -> Result<(), String> {
        unavailable()
    }
    pub fn read_f32(&self, count: usize) -> Result<Vec<f32>, String> {
        unavailable()
    }
}
impl ResidentBatch<'_> {
    pub fn commit_wait(self) -> Result<(), String> {
        unavailable()
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
        unavailable()
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
        unavailable()
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
        unavailable()
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
        unavailable()
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
        unavailable()
    }
    pub fn geglu(
        &self,
        input: &ResidentBuffer,
        out: &ResidentBuffer,
        n: usize,
        f: usize,
    ) -> Result<(), String> {
        unavailable()
    }
    pub fn add(
        &self,
        input: &ResidentBuffer,
        out: &ResidentBuffer,
        count: usize,
    ) -> Result<(), String> {
        unavailable()
    }
    pub fn bias(
        &self,
        out: &ResidentBuffer,
        bias: &ResidentBuffer,
        count: usize,
    ) -> Result<(), String> {
        unavailable()
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
        unavailable()
    }
    pub fn digit_head(
        &self,
        input: &ResidentBuffer,
        w: &ResidentBuffer,
        bias: &ResidentBuffer,
        out: &ResidentBuffer,
    ) -> Result<(), String> {
        unavailable()
    }
}
