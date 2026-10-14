//! Bounded Parquet files decoded in small Arrow batches.

use std::path::Path;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Float64Array, Int32Array, Int64Array, LargeListArray,
    LargeStringArray, ListArray, RecordBatch, StringArray, StringViewArray, UInt32Array,
    UInt64Array,
};
use bytes::Bytes;
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::{Error, Result, require};

pub(crate) const MAX_FILE_BYTES: usize = 64 << 20;
pub(crate) const MAX_ROWS: usize = 1_000_000;

pub(crate) fn batches(path: &Path, wanted: &[&str]) -> Result<Vec<RecordBatch>> {
    let bytes = crate::dataset::read_file(path, MAX_FILE_BYTES)?;
    require(
        bytes.len() >= 12 && &bytes[..4] == b"PAR1" && &bytes[bytes.len() - 4..] == b"PAR1",
        "Choose an ordinary Parquet recording table.",
    )?;
    let footer_len =
        u32::from_le_bytes(bytes[bytes.len() - 8..bytes.len() - 4].try_into().unwrap()) as usize;
    require(
        footer_len <= 1 << 20 && footer_len <= bytes.len() - 8,
        "This recording's table metadata is too large.",
    )?;
    let bytes = Bytes::from(bytes);
    let builder = ParquetRecordBatchReaderBuilder::try_new(bytes.clone())?;
    let metadata = builder.metadata();
    require(
        metadata.num_row_groups() <= 1_024
            && metadata.file_metadata().num_rows() >= 0
            && metadata.file_metadata().num_rows() as u64 <= MAX_ROWS as u64
            && metadata.file_metadata().schema_descr().num_columns() <= 128,
        "This recording exceeds the table size limits.",
    )?;
    let mut decoded_bytes = 0_u64;
    for group in metadata.row_groups() {
        require(
            group.num_rows() >= 0
                && group.num_rows() as u64 <= MAX_ROWS as u64
                && group.total_byte_size() >= 0,
            "This recording has an invalid table size.",
        )?;
        decoded_bytes = decoded_bytes
            .checked_add(group.total_byte_size() as u64)
            .ok_or_else(|| Error::Invalid("This recording is too large to decode.".into()))?;
        require(
            decoded_bytes <= 256 << 20,
            "This recording is too large to decode.",
        )?;
    }
    crate::page_bounds::check(&bytes, bytes.len() - 8 - footer_len, metadata)?;
    let roots: Vec<usize> = builder
        .schema()
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            (wanted.is_empty() || wanted.contains(&field.name().as_str())).then_some(index)
        })
        .collect();
    require(
        wanted.is_empty() || !roots.is_empty(),
        "The recording's table is missing the expected columns.",
    )?;
    let projection = ProjectionMask::roots(builder.parquet_schema(), roots);
    let reader = builder
        .with_projection(projection)
        .with_batch_size(512)
        .build()?;
    let mut batches = Vec::new();
    let mut memory = 0_usize;
    let mut rows = 0_usize;
    for batch in reader {
        let batch = batch?;
        memory = memory
            .checked_add(batch.get_array_memory_size())
            .ok_or_else(|| Error::Invalid("This recording is too large to decode.".into()))?;
        rows += batch.num_rows();
        require(
            memory <= 256 << 20 && rows <= MAX_ROWS,
            "This recording is too large to decode.",
        )?;
        batches.push(batch);
    }
    Ok(batches)
}

pub(crate) fn column<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a dyn Array> {
    batch
        .column_by_name(name)
        .map(|a| a.as_ref())
        .ok_or_else(|| Error::Invalid(format!("The recording has no {name} column.")))
}

pub(crate) fn integer(array: &dyn Array, row: usize) -> Result<u64> {
    require(
        row < array.len() && !array.is_null(row),
        "The recording has a missing index.",
    )?;
    let value = if let Some(a) = array.as_any().downcast_ref::<Int64Array>() {
        u64::try_from(a.value(row)).ok()
    } else if let Some(a) = array.as_any().downcast_ref::<Int32Array>() {
        u64::try_from(a.value(row)).ok()
    } else if let Some(a) = array.as_any().downcast_ref::<UInt64Array>() {
        Some(a.value(row))
    } else {
        array
            .as_any()
            .downcast_ref::<UInt32Array>()
            .map(|a| u64::from(a.value(row)))
    };
    value.ok_or_else(|| Error::Invalid("Use nonnegative integer indexes in the recording.".into()))
}

pub(crate) fn number(array: &dyn Array, row: usize) -> Result<f64> {
    require(
        row < array.len() && !array.is_null(row),
        "The recording has a missing value.",
    )?;
    let value = if let Some(a) = array.as_any().downcast_ref::<Float64Array>() {
        a.value(row)
    } else if let Some(a) = array.as_any().downcast_ref::<Float32Array>() {
        f64::from(a.value(row))
    } else {
        return Err(Error::Invalid(
            "Use floating-point joint values and timestamps.".into(),
        ));
    };
    require(
        value.is_finite(),
        "The recording contains an invalid number.",
    )?;
    Ok(value)
}

pub(crate) fn vector(array: &dyn Array, row: usize, width: usize) -> Result<Vec<f64>> {
    require(
        row < array.len() && !array.is_null(row) && (1..=64).contains(&width),
        "The recording has a missing joint vector.",
    )?;
    let values = if let Some(a) = array.as_any().downcast_ref::<ListArray>() {
        a.value(row)
    } else if let Some(a) = array.as_any().downcast_ref::<LargeListArray>() {
        a.value(row)
    } else if let Some(a) = array.as_any().downcast_ref::<FixedSizeListArray>() {
        a.value(row)
    } else {
        return Err(Error::Invalid(
            "Use a list of joint values for each recorded frame.".into(),
        ));
    };
    require(
        values.len() == width,
        "The recorded joint count doesn't match its feature list.",
    )?;
    (0..width).map(|i| number(values.as_ref(), i)).collect()
}

pub(crate) fn string(array: &dyn Array, row: usize) -> Result<String> {
    require(
        row < array.len() && !array.is_null(row),
        "The recording has a missing task description.",
    )?;
    let value = if let Some(a) = array.as_any().downcast_ref::<StringArray>() {
        a.value(row)
    } else if let Some(a) = array.as_any().downcast_ref::<LargeStringArray>() {
        a.value(row)
    } else if let Some(a) = array.as_any().downcast_ref::<StringViewArray>() {
        a.value(row)
    } else {
        return Err(Error::Invalid(
            "Use text for recorded task descriptions.".into(),
        ));
    };
    require(
        value.len() <= 16_384,
        "Use a shorter recorded task description.",
    )?;
    Ok(value.into())
}
