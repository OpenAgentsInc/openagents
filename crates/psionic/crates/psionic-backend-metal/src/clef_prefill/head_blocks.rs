//! Checked operations for the experimental device-resident field decoder.

use super::*;

/// Geometry and exact temporary allocation sizes for one field stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefFieldPlan {
    pub rows: usize,
    pub width: usize,
    pub heads: usize,
    pub feedforward: usize,
    pub row_bytes: usize,
    pub feedforward_bytes: usize,
    pub workspace_bytes: usize,
}

impl ClefFieldPlan {
    pub fn new(
        rows: usize,
        width: usize,
        heads: usize,
        feedforward: usize,
    ) -> Result<Self, String> {
        if !(1..=64).contains(&rows)
            || !(1..=4096).contains(&width)
            || !(1..=64).contains(&heads)
            || width % heads != 0
            || !(1..=16384).contains(&feedforward)
        {
            return Err(String::from("unsupported field decoder dimensions"));
        }
        let row_bytes = rows
            .checked_mul(width)
            .and_then(|x| x.checked_mul(4))
            .ok_or("field row size overflow")?
            .max(16);
        let feedforward_bytes = rows
            .checked_mul(feedforward)
            .and_then(|x| x.checked_mul(4))
            .ok_or("field feed-forward size overflow")?
            .max(16);
        // Fields, normalized fields, Q, K, V, context, and projected update.
        let workspace_bytes = row_bytes
            .checked_mul(7)
            .and_then(|x| x.checked_add(feedforward_bytes))
            .ok_or("field workspace size overflow")?;
        Ok(Self {
            rows,
            width,
            heads,
            feedforward,
            row_bytes,
            feedforward_bytes,
            workspace_bytes,
        })
    }

    /// Additional resident vectors; matrices already belong to the head.
    pub fn vector_bytes(&self, layers: usize) -> Result<usize, String> {
        if !(1..=16).contains(&layers) {
            return Err(String::from("unsupported field decoder layer count"));
        }
        // Three affine norms, four self-attention biases, three cross-attention
        // biases, and the two feed-forward biases. Each allocation is at least 16B.
        let width_bytes = self
            .width
            .checked_mul(4)
            .ok_or("field vector overflow")?
            .max(16);
        let ff_bytes = self
            .feedforward
            .checked_mul(4)
            .ok_or("field vector overflow")?
            .max(16);
        width_bytes
            .checked_mul(14)
            .and_then(|x| x.checked_add(ff_bytes))
            .and_then(|x| x.checked_mul(layers))
            .ok_or_else(|| String::from("field vectors overflow"))
    }
}

fn overlaps(a: &ClefMetalBuffer, a_bytes: usize, b: &ClefMetalBuffer, b_bytes: usize) -> bool {
    a.raw.contents() == b.raw.contents()
        && a.offset_bytes < b.offset_bytes + b_bytes
        && b.offset_bytes < a.offset_bytes + a_bytes
}

impl ClefMetalBatch<'_> {
    /// Adds an update in place. Exact aliases are safe; partial aliases are refused.
    pub fn add_f32(
        &self,
        target: &ClefMetalBuffer,
        update: &ClefMetalBuffer,
        count: usize,
    ) -> Result<(), String> {
        let bytes = count.checked_mul(4).ok_or("residual size overflow")?;
        target.check(0, bytes, "residual target")?;
        update.check(0, bytes, "residual update")?;
        if overlaps(target, bytes, update, bytes) && target.offset_bytes != update.offset_bytes {
            return Err(String::from("residual ranges partially overlap"));
        }
        let count_u32 = int(count, "residual count")?;
        if count == 0 {
            return Ok(());
        }
        self.pipeline("clef_add_f32")?;
        self.bind(0, target, 0);
        self.bind(1, update, 0);
        self.bytes(2, &count_u32);
        self.threads(count, 256);
        Ok(())
    }

    /// Noncausal self-attention for at most 64 field rows, with f32 softmax.
    pub fn field_self_attention(
        &self,
        q: &ClefMetalBuffer,
        k: &ClefMetalBuffer,
        v: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        rows: usize,
        heads: usize,
        width: usize,
    ) -> Result<(), String> {
        let plan = ClefFieldPlan::new(rows, width, heads, width)?;
        let bytes = rows * width * 4;
        for source in [q, k, v] {
            source.check(0, bytes, "field attention input")?;
            if overlaps(source, bytes, out, bytes) {
                return Err(String::from("field attention output overlaps an input"));
            }
        }
        out.check(0, bytes, "field attention output")?;
        self.pipeline("clef_field_self_attention")?;
        for (index, buffer) in [q, k, v, out].into_iter().enumerate() {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(
            4,
            &[plan.rows as u32, plan.heads as u32, plan.width as u32, 0],
        );
        self.groups(size(rows * heads, 1, 1), size(64, 1, 1));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
