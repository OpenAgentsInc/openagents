//! Native CNN execution and preprocessing for cropped handwritten digits.

use super::{ModelFamily, floats, identity, load_engine, pool};
use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result, Tensor, TensorData, Tensors,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[cfg(target_arch = "x86_64")]
mod simd;

#[cfg(feature = "native-cuda")]
mod cuda;
#[cfg(feature = "native-metal")]
mod metal;

pub struct MnistEngine {
    buffers: crate::buffers::Buffers,
    #[cfg(feature = "native-cuda")]
    cuda: Option<cuda::Digit>,
    #[cfg(feature = "native-metal")]
    metal: Option<metal::Digit>,
    accelerator: Option<std::sync::Mutex<crate::dense_accelerator::DenseAccelerator>>,
    conv1: Vec<f32>,
    bias1: Vec<f32>,
    conv2: Vec<f32>,
    bias2: Vec<f32>,
    fc: Vec<f32>,
    bias_fc: Vec<f32>,
    pool: rayon::ThreadPool,
    identity: ExecutionIdentity,
}

impl MnistEngine {
    pub(super) fn load_verified(root: &Path, threads: usize) -> Result<Self> {
        let pool = pool(threads)?;
        let bytes = std::fs::read(root.join("native.safetensors"))?;
        let tensors =
            safetensors::SafeTensors::deserialize(&bytes).map_err(|e| Error(e.to_string()))?;
        Ok(Self {
            buffers: crate::buffers::Buffers::default(),
            #[cfg(feature = "native-cuda")]
            cuda: None,
            #[cfg(feature = "native-metal")]
            metal: None,
            accelerator: None,
            conv1: floats(&tensors, "conv1.weight", &[8, 1, 5, 5])?,
            bias1: floats(&tensors, "conv1.bias", &[8])?,
            conv2: floats(&tensors, "conv2.weight", &[16, 8, 5, 5])?,
            bias2: floats(&tensors, "conv2.bias", &[16])?,
            fc: floats(&tensors, "fc.weight", &[256, 10])?,
            bias_fc: floats(&tensors, "fc.bias", &[10])?,
            pool,
            identity: identity("psionic-mnist-cpu", threads),
        })
    }

    pub(super) fn load_on(
        root: &Path,
        threads: usize,
        device: crate::DeviceChoice,
    ) -> Result<Self> {
        let mut model = Self::load_verified(root, threads)?;
        #[cfg(feature = "native-cuda")]
        if device == crate::DeviceChoice::Cuda {
            let digit = cuda::Digit::load(&mut model)?;
            model.identity.provider = "psionic-cuda-digit".into();
            model.identity.device = digit.device.clone();
            model.identity.cpu_fallback = Some(false);
            model.cuda = Some(digit);
            return Ok(model);
        }
        #[cfg(feature = "native-metal")]
        if device == crate::DeviceChoice::Metal {
            let gpu = metal::Digit::load(&mut model)?;
            model.identity.provider = "psionic-metal-digit".into();
            model.identity.device = gpu.device.clone();
            model.identity.cpu_fallback = Some(false);
            model.metal = Some(gpu);
            return Ok(model);
        }
        if device != crate::DeviceChoice::Cpu {
            let transpose = |values: &[f32], channels: usize| -> Vec<f32> {
                (0..values.len())
                    .map(|i| values[(i % channels) * (values.len() / channels) + i / channels])
                    .collect()
            };
            let first = transpose(&model.conv1, 8);
            let second = transpose(&model.conv2, 16);
            let accelerator = crate::dense_accelerator::DenseAccelerator::load(
                device,
                &[
                    (1, &first, 25, 8),
                    (2, &second, 200, 16),
                    (3, &model.fc, 256, 10),
                ],
            )?;
            model.identity.provider = accelerator.provider.clone();
            model.identity.device = accelerator.device.clone();
            model.identity.cpu_fallback = Some(true);
            model.accelerator = Some(std::sync::Mutex::new(accelerator));
        }
        Ok(model)
    }

    fn convolution(
        &self,
        input: &[f32],
        channels: usize,
        width: usize,
        weights: &[f32],
        bias: &[f32],
        key: usize,
    ) -> Result<Vec<f32>> {
        let Some(accelerator) = &self.accelerator else {
            return Ok(conv_relu(
                input,
                channels,
                width,
                weights,
                bias,
                false,
                if key == 2 { width / 3 * 3 } else { width },
                &self.buffers,
            ));
        };
        let k = channels * 25;
        let mut columns = vec![0.0; width * width * k];
        for y in 0..width {
            for x in 0..width {
                for c in 0..channels {
                    for ky in 0..5 {
                        for kx in 0..5 {
                            let iy = y as isize + ky as isize - 2;
                            let ix = x as isize + kx as isize - 2;
                            if iy >= 0 && ix >= 0 && (iy as usize) < width && (ix as usize) < width
                            {
                                columns[(y * width + x) * k + (c * 5 + ky) * 5 + kx] =
                                    input[c * width * width + iy as usize * width + ix as usize];
                            }
                        }
                    }
                }
            }
        }
        let result = accelerator
            .lock()
            .map_err(|_| Error("Cannot access native accelerator".into()))?
            .matmul(key, &columns, width * width, k, bias.len())?;
        let mut output = vec![0.0; result.len()];
        for c in 0..bias.len() {
            for i in 0..width * width {
                output[c * width * width + i] = (result[i * bias.len() + c] + bias[c]).max(0.0);
            }
        }
        Ok(output)
    }

    fn pixels(inputs: &Tensors) -> Result<&[f32]> {
        if inputs.len() != 1 {
            return Err(Error("The digit model requires one image".into()));
        }
        let input = inputs
            .get("Input3")
            .ok_or_else(|| Error("Missing digit image".into()))?;
        if input.shape != [1, 1, 28, 28] {
            return Err(Error(
                "The digit model expects a 28 by 28 grayscale image".into(),
            ));
        }
        input.floats()
    }

    fn forward(&self, inputs: &Tensors) -> Result<Tensors> {
        let pixels = Self::pixels(inputs)?;
        #[cfg(target_arch = "x86_64")]
        let fused = self.accelerator.is_none()
            && std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma");
        #[cfg(not(target_arch = "x86_64"))]
        let fused = false;
        let first = if fused {
            #[cfg(target_arch = "x86_64")]
            // SAFETY: both CPU features were checked and this is the pinned 28-to-14 stage.
            unsafe {
                simd::conv_relu(
                    pixels,
                    1,
                    28,
                    &self.conv1,
                    &self.bias1,
                    false,
                    28,
                    &self.buffers,
                    true,
                )
            }
            #[cfg(not(target_arch = "x86_64"))]
            unreachable!()
        } else {
            let first = self.convolution(pixels, 1, 28, &self.conv1, &self.bias1, 1)?;
            let pooled = max_pool_cached(&first, 8, 28, 2, &self.buffers);
            self.buffers.give(first);
            pooled
        };
        let second = self.convolution(&first, 8, 14, &self.conv2, &self.bias2, 2)?;
        self.buffers.give(first);
        let pooled = max_pool_cached(&second, 16, 14, 3, &self.buffers);
        self.buffers.give(second);
        let second = pooled;
        let mut logits = if let Some(accelerator) = &self.accelerator {
            accelerator
                .lock()
                .map_err(|_| Error("Cannot access native accelerator".into()))?
                .matmul(3, &second, 1, 256, 10)?
        } else {
            native_head(&second, &self.fc)
        };
        self.buffers.give(second);
        for (out, bias) in logits.iter_mut().zip(&self.bias_fc) {
            *out += bias;
        }
        Ok(BTreeMap::from([(
            "Plus214_Output_0".into(),
            Tensor {
                shape: vec![1, 10],
                data: TensorData::F32(logits),
            },
        )]))
    }
}

impl ModelEngine for MnistEngine {
    fn execution_report(&self) -> serde_json::Value {
        #[cfg(feature = "native-metal")]
        if let Some(metal) = &self.metal {
            return metal.report();
        }
        #[cfg(feature = "native-cuda")]
        if let Some(cuda) = &self.cuda {
            return cuda.report();
        }
        self.accelerator
            .as_ref()
            .and_then(|a| a.lock().ok())
            .map_or_else(|| {
                #[cfg(target_arch="x86_64")]
                let simd=std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma");
                #[cfg(not(target_arch="x86_64"))]
                let simd=false;
                serde_json::json!({"placement":"CPU","convolution":if simd {"AVX2/FMA with four channels and sixteen spatial pixels per group"} else {"portable scalar"},"requested_thread_budget":self.identity.threads,"convolution_parallelism":if simd {1} else {self.identity.threads},"first_stage_pooling":if simd {"fused with convolution"} else {"separate"},"cached_activation_bytes":self.buffers.cached_bytes()})
            }, |a| {
                let mut report = a.report();
                report["cpu_operations"] = serde_json::json!(
                    "im2col, bias, ReLU, max pooling, input and output preparation"
                );
                report
            })
    }

    fn identity(&self) -> &ExecutionIdentity {
        &self.identity
    }
    fn run(&mut self, inputs: &Tensors) -> Result<Tensors> {
        #[cfg(feature = "native-metal")]
        if let Some(metal) = &mut self.metal {
            return metal.run(Self::pixels(inputs)?);
        }
        #[cfg(feature = "native-cuda")]
        if let Some(cuda) = &mut self.cuda {
            return cuda.run(Self::pixels(inputs)?);
        }
        #[cfg(target_arch = "x86_64")]
        if self.accelerator.is_none()
            && std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
        {
            return self.forward(inputs);
        }
        self.pool.install(|| self.forward(inputs))
    }
}

fn conv_relu(
    input: &[f32],
    channels: usize,
    width: usize,
    weights: &[f32],
    bias: &[f32],
    parallel: bool,
    rows: usize,
    buffers: &crate::buffers::Buffers,
) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        // SAFETY: both required CPU features were checked; the pinned model supplies fixed dimensions.
        return unsafe {
            simd::conv_relu(
                input, channels, width, weights, bias, parallel, rows, buffers, false,
            )
        };
    }
    conv_relu_scalar(input, channels, width, weights, bias)
}

fn conv_relu_scalar(
    input: &[f32],
    channels: usize,
    width: usize,
    weights: &[f32],
    bias: &[f32],
) -> Vec<f32> {
    let mut result = vec![0.0; bias.len() * width * width];
    result
        .par_chunks_mut(width * width)
        .enumerate()
        .for_each(|(out_channel, plane)| {
            for y in 0..width {
                for x in 0..width {
                    let mut sum = 0.0_f32;
                    for channel in 0..channels {
                        for ky in 0..5 {
                            for kx in 0..5 {
                                let iy = y as isize + ky as isize - 2;
                                let ix = x as isize + kx as isize - 2;
                                if iy >= 0
                                    && ix >= 0
                                    && (iy as usize) < width
                                    && (ix as usize) < width
                                {
                                    sum += input[channel * width * width
                                        + iy as usize * width
                                        + ix as usize]
                                        * weights[((out_channel * channels + channel) * 5 + ky)
                                            * 5
                                            + kx];
                                }
                            }
                        }
                    }
                    plane[y * width + x] = (sum + bias[out_channel]).max(0.0);
                }
            }
        });
    result
}

#[cfg(test)]
fn max_pool(input: &[f32], channels: usize, width: usize, stride: usize) -> Vec<f32> {
    max_pool_cached(
        input,
        channels,
        width,
        stride,
        &crate::buffers::Buffers::default(),
    )
}

fn native_head(features: &[f32], weights: &[f32]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        // SAFETY: features were checked and the pinned model supplies a 256-by-10 dense matrix.
        return unsafe { simd::head(features, weights) };
    }
    let mut logits = vec![0.0; 10];
    for (i, value) in features.iter().enumerate() {
        for (j, out) in logits.iter_mut().enumerate() {
            *out += value * weights[i * 10 + j];
        }
    }
    logits
}

fn max_pool_cached(
    input: &[f32],
    channels: usize,
    width: usize,
    stride: usize,
    buffers: &crate::buffers::Buffers,
) -> Vec<f32> {
    let height = input.len() / (channels * width);
    let next = width / stride;
    let mut output = buffers.take(channels * next * next);
    #[cfg(target_arch = "x86_64")]
    if ((stride == 2 && width == 28) || (stride == 3 && width == 14))
        && std::arch::is_x86_feature_detected!("avx2")
        && std::arch::is_x86_feature_detected!("fma")
    {
        // SAFETY: features and complete pinned spatial shapes were checked.
        unsafe {
            simd::pool(input, channels, width, height, stride, &mut output);
        }
        return output;
    }

    for channel in 0..channels {
        for y in 0..next {
            for x in 0..next {
                let mut max = f32::NEG_INFINITY;
                for ky in 0..stride {
                    for kx in 0..stride {
                        max = max.max(
                            input[channel * height * width
                                + (y * stride + ky) * width
                                + x * stride
                                + kx],
                        );
                    }
                }
                output[channel * next * next + y * next + x] = max;
            }
        }
    }
    output
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DigitPrediction {
    pub digit: usize,
    pub logits: Vec<f32>,
    pub probabilities: Vec<f32>,
}

pub struct MnistModel {
    engine: Box<dyn ModelEngine>,
}

impl MnistModel {
    pub fn load(
        root: &Path,
        kind: EngineKind,
        threads: usize,
        library: Option<&Path>,
        checksum: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            engine: load_engine(ModelFamily::Mnist, kind, root, threads, library, checksum)?,
        })
    }
    pub fn identity(&self) -> &ExecutionIdentity {
        self.engine.identity()
    }
    pub fn prepare(pixels: &[u8]) -> Result<Tensors> {
        if pixels.len() != 28 * 28 {
            return Err(Error(
                "Provide 784 grayscale pixels for the digit model".into(),
            ));
        }
        Ok(BTreeMap::from([(
            "Input3".into(),
            Tensor {
                shape: vec![1, 1, 28, 28],
                data: TensorData::F32(pixels.iter().map(|v| f32::from(*v) / 255.0).collect()),
            },
        )]))
    }
    pub fn classify(&mut self, pixels: &[u8]) -> Result<DigitPrediction> {
        let outputs = self.engine.run(&Self::prepare(pixels)?)?;
        let tensor = outputs
            .get("Plus214_Output_0")
            .ok_or_else(|| Error("Missing digit model output".into()))?;
        if tensor.shape != [1, 10] {
            return Err(Error("Unexpected digit output dimensions".into()));
        }
        let logits = tensor.floats()?.to_vec();
        let mut digit = 0;
        for i in 1..10 {
            if logits[i] > logits[digit] {
                digit = i;
            }
        }
        let max = logits[digit];
        let mut probabilities: Vec<f32> = logits.iter().map(|v| (*v - max).exp()).collect();
        let sum: f32 = probabilities.iter().sum();
        for p in &mut probabilities {
            *p /= sum;
        }
        Ok(DigitPrediction {
            digit,
            logits,
            probabilities,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_convolution_matches_scalar_at_padding_and_row_tails() {
        for width in [3, 7, 14, 28] {
            for channels in [1, 8] {
                for out_channels in [3, 4, 8] {
                    let input: Vec<_> = (0..channels * width * width)
                        .map(|i| ((i * 13 % 37) as f32 - 18.0) / 19.0)
                        .collect();
                    let weights: Vec<_> = (0..out_channels * channels * 25)
                        .map(|i| ((i * 7 % 29) as f32 - 14.0) / 31.0)
                        .collect();
                    let bias: Vec<_> = (0..out_channels).map(|i| (i as f32 - 1.0) * 0.1).collect();
                    let actual = conv_relu(
                        &input,
                        channels,
                        width,
                        &weights,
                        &bias,
                        true,
                        width,
                        &crate::buffers::Buffers::default(),
                    );
                    let expected = conv_relu_scalar(&input, channels, width, &weights, &bias);
                    let distance = actual
                        .iter()
                        .zip(&expected)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0_f32, f32::max);
                    assert!(
                        distance < 1e-4,
                        "width={width}, channels={channels}, error={distance}"
                    );
                }
            }
        }
    }
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn fused_first_stage_matches_scalar_convolution_and_pooling() {
        if !std::arch::is_x86_feature_detected!("avx2")
            || !std::arch::is_x86_feature_detected!("fma")
        {
            return;
        }
        let input: Vec<_> = (0..784)
            .map(|i| ((i * 13 % 37) as f32 - 18.0) / 19.0)
            .collect();
        let weights: Vec<_> = (0..8 * 25)
            .map(|i| ((i * 7 % 29) as f32 - 14.0) / 31.0)
            .collect();
        let bias = vec![-0.1; 8];
        let buffers = crate::buffers::Buffers::default();
        let expected = max_pool(&conv_relu_scalar(&input, 1, 28, &weights, &bias), 8, 28, 2);
        for _ in 0..3 {
            // SAFETY: features and complete pinned dimensions were checked.
            let actual = unsafe {
                simd::conv_relu(&input, 1, 28, &weights, &bias, false, 28, &buffers, true)
            };
            assert_eq!(actual.len(), expected.len());
            assert!(
                actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| (a - b).abs() < 1e-5)
            );
            buffers.give(actual);
        }
    }

    #[test]
    fn cropped_convolution_preserves_every_pooled_value() {
        let width = 14;
        let channels = 8;
        let input: Vec<_> = (0..channels * width * width)
            .map(|i| ((i * 13 % 37) as f32 - 18.0) / 19.0)
            .collect();
        let weights: Vec<_> = (0..16 * channels * 25)
            .map(|i| ((i * 7 % 29) as f32 - 14.0) / 31.0)
            .collect();
        let bias = vec![0.1; 16];
        let buffers = crate::buffers::Buffers::default();
        let cropped = conv_relu(
            &input, channels, width, &weights, &bias, false, 12, &buffers,
        );
        let full = conv_relu_scalar(&input, channels, width, &weights, &bias);
        let expected = max_pool(&full, 16, width, 3);
        let actual = max_pool_cached(&cropped, 16, width, 3, &buffers);
        assert_eq!(actual.len(), 256);
        for (a, b) in actual.iter().zip(expected) {
            assert!((a - b).abs() < 1e-4, "{a} != {b}");
        }
    }

    #[test]
    fn pooling_uses_floor_dimensions_and_keeps_channels_separate() {
        let input: Vec<f32> = (0..32).map(|i| i as f32).collect();
        assert_eq!(
            max_pool(&input, 2, 4, 2),
            vec![5.0, 7.0, 13.0, 15.0, 21.0, 23.0, 29.0, 31.0]
        );
        assert_eq!(max_pool(&input[..16], 1, 4, 3), vec![10.0]);
    }
    #[test]
    fn image_preparation_scales_bytes_and_refuses_wrong_sizes() {
        assert!(MnistModel::prepare(&[0; 10]).is_err());
        let input = MnistModel::prepare(&[255; 784]).unwrap();
        assert_eq!(input["Input3"].floats().unwrap(), vec![1.0; 784]);
    }
}
