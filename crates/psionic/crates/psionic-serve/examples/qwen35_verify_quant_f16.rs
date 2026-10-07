use std::{env, path::PathBuf, process::ExitCode};

use psionic_backend_cpu::{decode_quantized_row_into, quantized_row_byte_len};
use psionic_backend_cuda::{CudaBackend, CudaCommandStatus, CudaCommandWait};
use psionic_catalog::LocalBlobOpenOptions;
use psionic_models::GgufBlobArtifact;
use psionic_runtime::DeviceDiscovery;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(model_path) = args.next().map(PathBuf::from) else {
        return Err(String::from(
            "usage: cargo run -p psionic-serve --example qwen35_verify_quant_f16 -- <model.gguf> <tensor[,tensor...]>",
        ));
    };
    let Some(tensor_arg) = args.next() else {
        return Err(String::from(
            "usage: cargo run -p psionic-serve --example qwen35_verify_quant_f16 -- <model.gguf> <tensor[,tensor...]>",
        ));
    };
    let tensor_names = tensor_arg
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if tensor_names.is_empty() {
        return Err(String::from("at least one tensor name is required"));
    }

    let artifact = GgufBlobArtifact::open_path(model_path, LocalBlobOpenOptions::default())
        .map_err(|error| format!("failed to open GGUF artifact: {error}"))?;
    let mut backend = CudaBackend::new();
    if backend.selected_device().is_none() {
        return Err(format!(
            "cuda backend is unavailable: {:?}",
            backend.health().status
        ));
    }

    let mut columns = None;
    let mut row_offset = 0usize;
    let mut expected = Vec::new();
    let mut transposed = Vec::new();
    let mut input_f32 = Vec::new();
    let mut total_rows = 0usize;

    for tensor_name in &tensor_names {
        let storage = artifact
            .paged_tensor(tensor_name)
            .map_err(|error| format!("failed to open tensor `{tensor_name}`: {error}"))?;
        let metadata = storage.metadata();
        let [rows, cols] = metadata.shape.dims() else {
            return Err(format!(
                "tensor `{tensor_name}` is not a matrix: {:?}",
                metadata.shape.dims()
            ));
        };
        let row_byte_len =
            quantized_row_byte_len(&metadata.shape, metadata.quantized_layout.ok_or_else(|| {
                format!("tensor `{tensor_name}` is not quantized")
            })?)
            .map_err(|error| format!("failed to resolve row byte length for `{tensor_name}`: {error}"))?;
        let mode = metadata.quantization;
        if columns.replace(*cols).is_some_and(|expected_cols| expected_cols != *cols) {
            return Err(format!(
                "tensor `{tensor_name}` width mismatch: expected {}, actual {}",
                columns.unwrap_or(*cols),
                cols
            ));
        }
        if input_f32.is_empty() {
            input_f32 = (0..*cols)
                .map(|index| ((index % 29) as f32 - 14.0) / 7.0)
                .map(f16_roundtrip)
                .collect();
        }
        if transposed.is_empty() {
            transposed.resize(
                tensor_names
                    .iter()
                    .map(|name| {
                        artifact
                            .paged_tensor(name)
                            .ok()
                            .and_then(|storage| storage.metadata().shape.dims().first().copied())
                            .unwrap_or(0)
                    })
                    .sum::<usize>()
                    .saturating_mul(*cols)
                    .saturating_mul(std::mem::size_of::<u16>()),
                0,
            );
        }
        let bytes = storage
            .bytes()
            .map_err(|error| format!("failed to read tensor `{tensor_name}` bytes: {error}"))?;
        for (local_row_index, row_bytes) in bytes.chunks_exact(row_byte_len).enumerate() {
            let mut decoded = Vec::new();
            decode_quantized_row_into(mode, row_bytes, &mut decoded)
                .map_err(|error| format!("failed to decode `{tensor_name}` row {local_row_index}: {error}"))?;
            if decoded.len() != *cols {
                return Err(format!(
                    "decoded `{tensor_name}` row {local_row_index} width mismatch: expected {cols}, actual {}",
                    decoded.len()
                ));
            }
            expected.push(dot(decoded.as_slice(), input_f32.as_slice()));
            let packed_row_index = row_offset + local_row_index;
            for (column_index, value) in decoded.iter().copied().enumerate() {
                let offset = column_index
                    .saturating_mul(
                        transposed.len()
                            / columns.unwrap_or(*cols)
                            / std::mem::size_of::<u16>(),
                    )
                    .saturating_add(packed_row_index)
                    .saturating_mul(std::mem::size_of::<u16>());
                transposed[offset..offset + std::mem::size_of::<u16>()]
                    .copy_from_slice(&f32_to_f16_bits(value).to_le_bytes());
            }
        }
        row_offset = row_offset.saturating_add(*rows);
        total_rows = total_rows.saturating_add(*rows);
    }

    let columns = columns.ok_or_else(|| String::from("no projection columns resolved"))?;
    let mut left = backend
        .f16_buffer(columns)
        .map_err(|error| format!("failed to allocate input f16 buffer: {error}"))?;
    left.write_bytes(f32_slice_to_f16_bytes(input_f32.as_slice()).as_slice())
        .map_err(|error| format!("failed to upload input f16 buffer: {error}"))?;
    let right = backend
        .byte_buffer(transposed.as_slice())
        .map_err(|error| format!("failed to upload transposed f16 buffer: {error}"))?;
    let output = backend
        .f32_buffer(total_rows)
        .map_err(|error| format!("failed to allocate output buffer: {error}"))?;

    let mut submission = backend
        .begin_submission()
        .map_err(|error| format!("failed to begin cuda submission: {error}"))?;
    submission
        .matmul_f16_to_f32(&left, &right, &output, 1, columns, total_rows)
        .map_err(|error| format!("failed to encode cuda matmul: {error}"))?;
    let report = submission
        .commit(CudaCommandWait::Completed)
        .map_err(|error| format!("failed to submit cuda matmul: {error}"))?;
    if report.status != CudaCommandStatus::Completed {
        return Err(format!("cuda matmul did not complete: {:?}", report.status));
    }
    let actual = output
        .read_f32()
        .map_err(|error| format!("failed to read cuda output: {error}"))?;
    let mut max_abs_diff = 0.0_f32;
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        max_abs_diff = max_abs_diff.max((actual - expected).abs());
    }
    println!("tensors={}", tensor_names.join(","));
    println!("rows={total_rows} cols={columns} max_abs_diff={max_abs_diff:.8}");
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).take(8).enumerate() {
        println!("row={index} actual={actual:.8} expected={expected:.8}");
    }
    Ok(())
}

fn f32_slice_to_f16_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(std::mem::size_of::<u16>()));
    for value in values {
        bytes.extend_from_slice(&f32_to_f16_bits(*value).to_le_bytes());
    }
    bytes
}

fn f16_roundtrip(value: f32) -> f32 {
    f16_bits_to_f32(f32_to_f16_bits(value))
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right.iter()).map(|(left, right)| left * right).sum()
}

fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;

    if exponent == 0xff {
        if mantissa == 0 {
            return sign | 0x7c00;
        }
        let payload = ((mantissa >> 13) as u16) | 1;
        return sign | 0x7c00 | payload;
    }

    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x0080_0000;
        let shift = (14 - half_exponent) as u32;
        let mut half_mantissa = (mantissa >> shift) as u16;
        let remainder_mask = (1_u32 << shift) - 1;
        let remainder = mantissa & remainder_mask;
        let halfway = 1_u32 << (shift - 1);
        if remainder > halfway || (remainder == halfway && (half_mantissa & 1) != 0) {
            half_mantissa = half_mantissa.wrapping_add(1);
        }
        return sign | half_mantissa;
    }

    let mut half = sign | (((half_exponent as u16) & 0x1f) << 10) | ((mantissa >> 13) as u16);
    let remainder = mantissa & 0x1fff;
    if remainder > 0x1000 || (remainder == 0x1000 && (half & 1) != 0) {
        half = half.wrapping_add(1);
    }
    half
}

fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits & 0x8000)) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = bits & 0x03ff;
    let value = if exponent == 0 {
        if mantissa == 0 {
            sign
        } else {
            let mut normalized = u32::from(mantissa);
            let mut shift = 0_u32;
            while (normalized & 0x0400) == 0 {
                normalized <<= 1;
                shift = shift.saturating_add(1);
            }
            normalized &= 0x03ff;
            sign | ((113_u32.saturating_sub(shift)) << 23) | (normalized << 13)
        }
    } else if exponent == 0x1f {
        sign | 0x7f80_0000 | (u32::from(mantissa) << 13)
    } else {
        sign | ((u32::from(exponent) + 112) << 23) | (u32::from(mantissa) << 13)
    };
    f32::from_bits(value)
}
