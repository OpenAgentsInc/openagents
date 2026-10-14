//! Check compact-Thrift Parquet page sizes before a codec allocates memory.
//! See apache/thrift's compact protocol and apache/parquet-format's PageHeader.

use crate::{Result, require};

const MAX_PAGE_BYTES: u64 = 4 << 20;
const MAX_HEADER_BYTES: usize = 64 << 10;
const MAX_PAGE_VALUES: i64 = 1_000_000;

pub(crate) fn check(
    bytes: &[u8],
    footer_start: usize,
    metadata: &parquet::file::metadata::ParquetMetaData,
) -> Result<()> {
    let mut total_decoded = 0_u64;
    for group in metadata.row_groups() {
        for column in group.columns() {
            require(
                column.compressed_size() >= 0
                    && column.data_page_offset() >= 4
                    && column
                        .dictionary_page_offset()
                        .is_none_or(|offset| offset >= 4),
                "Use valid recording page offsets and lengths.",
            )?;
            let (start, length) = column.byte_range();
            let end = start.checked_add(length).ok_or_else(|| {
                crate::Error::Invalid("This recording has an invalid page range.".into())
            })?;
            require(
                start >= 4 && end <= footer_start as u64,
                "Keep recording pages inside their table file.",
            )?;
            let mut at = start as usize;
            let end = end as usize;
            while at < end {
                let mut header = Compact {
                    bytes: &bytes[at..end.min(at + MAX_HEADER_BYTES)],
                    at: 0,
                    values: 0,
                };
                let fields = header.structure(0, true)?;
                let uncompressed = fields.get(&2).copied().unwrap_or(-1);
                let compressed = fields.get(&3).copied().unwrap_or(-1);
                require(
                    uncompressed >= 0
                        && compressed >= 0
                        && uncompressed as u64 <= MAX_PAGE_BYTES
                        && compressed as u64 <= MAX_PAGE_BYTES,
                    "This recording has a page larger than 4 MB.",
                )?;
                total_decoded =
                    total_decoded
                        .checked_add(uncompressed as u64)
                        .ok_or_else(|| {
                            crate::Error::Invalid("This recording is too large to decode.".into())
                        })?;
                require(
                    total_decoded <= 256 << 20,
                    "This recording is too large to decode.",
                )?;
                let next = at
                    .checked_add(header.at)
                    .and_then(|n| n.checked_add(compressed as usize));
                require(
                    next.is_some_and(|n| n > at && n <= end),
                    "The recording contains a truncated page.",
                )?;
                at = next.unwrap();
            }
        }
    }
    Ok(())
}

struct Compact<'a> {
    bytes: &'a [u8],
    at: usize,
    values: usize,
}

impl Compact<'_> {
    fn byte(&mut self) -> Result<u8> {
        require(
            self.at < self.bytes.len(),
            "The recording has a truncated or oversized page header.",
        )?;
        let value = self.bytes[self.at];
        self.at += 1;
        Ok(value)
    }
    fn unsigned(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        for shift in (0..70).step_by(7) {
            let byte = self.byte()?;
            require(
                shift != 63 || byte <= 1,
                "The recording has an invalid page header number.",
            )?;
            value |= u64::from(byte & 127) << shift;
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        Err(crate::Error::Invalid(
            "The recording has an invalid page header number.".into(),
        ))
    }
    fn signed(&mut self) -> Result<i64> {
        let value = self.unsigned()?;
        Ok((value >> 1) as i64 ^ -((value & 1) as i64))
    }
    fn skip_bytes(&mut self, count: usize) -> Result<()> {
        require(
            count <= self.bytes.len().saturating_sub(self.at),
            "The recording has a truncated or oversized page header.",
        )?;
        self.at += count;
        Ok(())
    }
    fn structure(
        &mut self,
        depth: usize,
        root: bool,
    ) -> Result<std::collections::BTreeMap<i64, i64>> {
        require(depth <= 16, "Use fewer nested recording header fields.")?;
        let mut previous = 0_i64;
        let mut fields = std::collections::BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let header = self.byte()?;
            if header == 0 {
                return Ok(fields);
            }
            let kind = header & 15;
            let delta = i64::from(header >> 4);
            let field = if delta == 0 {
                self.signed()?
            } else {
                previous + delta
            };
            require(
                field > 0 && field <= i64::from(i16::MAX) && seen.insert(field),
                "Use different valid recording header fields.",
            )?;
            previous = field;
            if matches!(kind, 4..=6) {
                let value = self.signed()?;
                if root {
                    fields.insert(field, value);
                }
                // Field one in DataPageHeader, DictionaryPageHeader, and
                // DataPageHeaderV2 is a value count, checked before decoding.
                if depth == 1 && field == 1 {
                    require(
                        (0..=MAX_PAGE_VALUES).contains(&value),
                        "This recording has too many values in one page.",
                    )?;
                }
            } else {
                self.skip(kind, depth, true)?;
            }
        }
    }
    fn skip(&mut self, kind: u8, depth: usize, field: bool) -> Result<()> {
        self.values += 1;
        require(
            self.values <= MAX_HEADER_BYTES && depth <= 16,
            "This recording's page header is too large.",
        )?;
        match kind {
            1 | 2 => {
                if !field {
                    self.byte()?;
                }
            }
            3 => {
                self.byte()?;
            }
            4..=6 => {
                self.unsigned()?;
            }
            7 => self.skip_bytes(8)?,
            8 => {
                let length = self.unsigned()?;
                require(
                    length <= MAX_HEADER_BYTES as u64,
                    "Use shorter recording header values.",
                )?;
                self.skip_bytes(length as usize)?;
            }
            9 | 10 => {
                let header = self.byte()?;
                let length = if header >> 4 == 15 {
                    self.unsigned()?
                } else {
                    u64::from(header >> 4)
                };
                require(
                    length <= MAX_HEADER_BYTES as u64,
                    "Use shorter recording header lists.",
                )?;
                for _ in 0..length {
                    self.skip(header & 15, depth + 1, false)?;
                }
            }
            11 => {
                let length = self.unsigned()?;
                require(
                    length <= MAX_HEADER_BYTES as u64,
                    "Use smaller recording header maps.",
                )?;
                if length > 0 {
                    let kinds = self.byte()?;
                    for _ in 0..length {
                        self.skip(kinds >> 4, depth + 1, false)?;
                        self.skip(kinds & 15, depth + 1, false)?;
                    }
                }
            }
            12 => {
                self.structure(depth + 1, false)?;
            }
            _ => {
                return Err(crate::Error::Invalid(
                    "This recording uses an unsupported page header type.".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_header_integer_overflow_and_nesting_are_refused() {
        let mut overflow = Compact {
            bytes: &[255; 10],
            at: 0,
            values: 0,
        };
        assert!(overflow.unsigned().is_err());
        let mut deep = Compact {
            bytes: &[0x1c; 40],
            at: 0,
            values: 0,
        };
        assert!(deep.structure(0, true).is_err());
        let mut truncated = Compact {
            bytes: &[0x25, 0x80],
            at: 0,
            values: 0,
        };
        assert!(truncated.structure(0, true).is_err());
    }
}
