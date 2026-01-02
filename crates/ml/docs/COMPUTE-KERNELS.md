# WGSL Compute Kernels

Complete WGSL shader implementations for ML operations.

## Kernel Overview

| Kernel | Entry Point | Workgroup Size | Purpose |
|--------|-------------|----------------|---------|
| GEMM (tiled) | `main` | 16x16x1 | Matrix multiplication |
| Softmax | `main` | 256x1x1 | Attention scores |
| RMSNorm | `main` | 256x1x1 | Layer normalization |
| SiLU | `main` | 256x1x1 | Activation (LLaMA) |
| GELU | `main` | 256x1x1 | Activation (GPT) |
| RoPE | `main` | 64x1x1 | Rotary embeddings |
| Attention | `main` | 256x1x1 | Scaled dot-product |
| Sinkhorn-Knopp | 4 kernels | 256x1x1 | Doubly stochastic projection |

## Common Uniform Structures

```wgsl
// Shape and stride information for tensors
struct ShapeUniform {
    dims: vec4<u32>,      // [d0, d1, d2, d3]
    strides: vec4<u32>,   // [s0, s1, s2, s3]
}

// Matrix dimensions for GEMM
struct MatmulUniform {
    M: u32,  // Output rows
    N: u32,  // Output cols
    K: u32,  // Inner dimension
    _pad: u32,
}

// Normalization parameters
struct NormUniform {
    size: u32,    // Vector length
    eps: f32,     // Epsilon for numerical stability
    _pad0: u32,
    _pad1: u32,
}

// RoPE parameters
struct RopeUniform {
    seq_len: u32,
    head_dim: u32,
    base: f32,    // Typically 10000.0
    _pad: u32,
}

// Attention parameters
struct AttentionUniform {
    batch_size: u32,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
    scale: f32,      // 1/sqrt(head_dim)
    causal: u32,     // 0 or 1
    _pad0: u32,
    _pad1: u32,
}
```

## Element-wise Operations

### Add / Multiply / Scale

```wgsl
// elementwise.wgsl
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> size: u32;

@compute @workgroup_size(256)
fn add(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= size) { return; }
    output[idx] = a[idx] + b[idx];
}

@compute @workgroup_size(256)
fn mul(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= size) { return; }
    output[idx] = a[idx] * b[idx];
}

@compute @workgroup_size(256)
fn scale(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= size) { return; }
    // b[0] contains the scalar
    output[idx] = a[idx] * b[0];
}
```

## GEMM (Tiled Matrix Multiplication)

```wgsl
// gemm.wgsl
// Computes C = A @ B where A is [M, K], B is [K, N], C is [M, N]
// Uses tiled algorithm with shared memory for efficiency

const TILE_SIZE: u32 = 16u;

@group(0) @binding(0) var<storage, read> A: array<f32>;
@group(0) @binding(1) var<storage, read> B: array<f32>;
@group(0) @binding(2) var<storage, read_write> C: array<f32>;
@group(0) @binding(3) var<uniform> params: MatmulUniform;

var<workgroup> tile_A: array<f32, 256>;  // 16x16
var<workgroup> tile_B: array<f32, 256>;  // 16x16

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let row = global_id.y;
    let col = global_id.x;
    let local_row = local_id.y;
    let local_col = local_id.x;

    var acc: f32 = 0.0;
    let num_tiles = (params.K + TILE_SIZE - 1u) / TILE_SIZE;

    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        // Load tile from A
        let a_col = t * TILE_SIZE + local_col;
        if (row < params.M && a_col < params.K) {
            tile_A[local_row * TILE_SIZE + local_col] = A[row * params.K + a_col];
        } else {
            tile_A[local_row * TILE_SIZE + local_col] = 0.0;
        }

        // Load tile from B
        let b_row = t * TILE_SIZE + local_row;
        if (b_row < params.K && col < params.N) {
            tile_B[local_row * TILE_SIZE + local_col] = B[b_row * params.N + col];
        } else {
            tile_B[local_row * TILE_SIZE + local_col] = 0.0;
        }

        workgroupBarrier();

        // Compute partial dot product
        for (var k: u32 = 0u; k < TILE_SIZE; k = k + 1u) {
            acc = acc + tile_A[local_row * TILE_SIZE + k] * tile_B[k * TILE_SIZE + local_col];
        }

        workgroupBarrier();
    }

    // Write result
    if (row < params.M && col < params.N) {
        C[row * params.N + col] = acc;
    }
}
```

## Softmax

```wgsl
// softmax.wgsl
// Computes softmax along the last dimension
// Input shape: [batch, seq_len] or flattened with stride info

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: NormUniform;  // size = seq_len

var<workgroup> shared_max: f32;
var<workgroup> shared_sum: f32;
var<workgroup> partial_max: array<f32, 256>;
var<workgroup> partial_sum: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let tid = local_id.x;
    let batch_idx = group_id.x;
    let size = params.size;
    let base = batch_idx * size;

    // Step 1: Find max (parallel reduction)
    var local_max: f32 = -3.402823e+38;  // -FLT_MAX
    for (var i = tid; i < size; i = i + 256u) {
        if (base + i < arrayLength(&input)) {
            local_max = max(local_max, input[base + i]);
        }
    }
    partial_max[tid] = local_max;

    workgroupBarrier();

    // Reduce max within workgroup
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s && tid + s < 256u) {
            partial_max[tid] = max(partial_max[tid], partial_max[tid + s]);
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        shared_max = partial_max[0];
    }
    workgroupBarrier();

    // Step 2: Compute exp(x - max) and sum
    var local_sum: f32 = 0.0;
    for (var i = tid; i < size; i = i + 256u) {
        if (base + i < arrayLength(&input)) {
            let val = exp(input[base + i] - shared_max);
            output[base + i] = val;  // Store temporarily
            local_sum = local_sum + val;
        }
    }
    partial_sum[tid] = local_sum;

    workgroupBarrier();

    // Reduce sum within workgroup
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s && tid + s < 256u) {
            partial_sum[tid] = partial_sum[tid] + partial_sum[tid + s];
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        shared_sum = partial_sum[0];
    }
    workgroupBarrier();

    // Step 3: Normalize
    let inv_sum = 1.0 / shared_sum;
    for (var i = tid; i < size; i = i + 256u) {
        if (base + i < arrayLength(&output)) {
            output[base + i] = output[base + i] * inv_sum;
        }
    }
}
```

## RMSNorm

```wgsl
// rmsnorm.wgsl
// Computes RMSNorm: x * rsqrt(mean(x^2) + eps) * weight
// Used in LLaMA, Mistral, etc.

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read> weight: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> params: NormUniform;

var<workgroup> shared_ss: f32;  // sum of squares
var<workgroup> partial_ss: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let tid = local_id.x;
    let batch_idx = group_id.x;
    let size = params.size;
    let eps = params.eps;
    let base = batch_idx * size;

    // Step 1: Compute sum of squares (parallel reduction)
    var local_ss: f32 = 0.0;
    for (var i = tid; i < size; i = i + 256u) {
        if (base + i < arrayLength(&input)) {
            let val = input[base + i];
            local_ss = local_ss + val * val;
        }
    }
    partial_ss[tid] = local_ss;

    workgroupBarrier();

    // Reduce within workgroup
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s && tid + s < 256u) {
            partial_ss[tid] = partial_ss[tid] + partial_ss[tid + s];
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        shared_ss = partial_ss[0];
    }
    workgroupBarrier();

    // Step 2: Normalize
    let rms = inverseSqrt(shared_ss / f32(size) + eps);
    for (var i = tid; i < size; i = i + 256u) {
        if (base + i < arrayLength(&output)) {
            output[base + i] = input[base + i] * rms * weight[i];
        }
    }
}
```

## Activation Functions

### SiLU (Swish)

```wgsl
// silu.wgsl
// SiLU(x) = x * sigmoid(x) = x / (1 + exp(-x))
// Used in LLaMA FFN

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> size: u32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= size) { return; }

    let x = input[idx];
    // SiLU = x * sigmoid(x)
    // For numerical stability, use: x / (1 + exp(-x))
    let sig = 1.0 / (1.0 + exp(-x));
    output[idx] = x * sig;
}
```

### GELU

```wgsl
// gelu.wgsl
// GELU(x) = 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
// Used in GPT, BERT

const SQRT_2_OVER_PI: f32 = 0.7978845608;  // sqrt(2/pi)

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> size: u32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= size) { return; }

    let x = input[idx];
    let inner = SQRT_2_OVER_PI * (x + 0.044715 * x * x * x);
    output[idx] = 0.5 * x * (1.0 + tanh(inner));
}
```

## RoPE (Rotary Position Embeddings)

```wgsl
// rope.wgsl
// Applies rotary position embeddings to Q and K tensors
// Input shape: [batch, seq_len, num_heads, head_dim]

@group(0) @binding(0) var<storage, read_write> x: array<f32>;  // Q or K
@group(0) @binding(1) var<storage, read> freqs_cos: array<f32>;  // [seq_len, head_dim/2]
@group(0) @binding(2) var<storage, read> freqs_sin: array<f32>;  // [seq_len, head_dim/2]
@group(0) @binding(3) var<uniform> params: RopeUniform;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch = gid.z;
    let pos = gid.y;    // Position in sequence
    let pair = gid.x;   // Which pair of values (head_dim/2 pairs)

    let seq_len = params.seq_len;
    let head_dim = params.head_dim;
    let half_dim = head_dim / 2u;

    if (pos >= seq_len || pair >= half_dim) { return; }

    // Index into x: [batch, pos, pair*2] and [batch, pos, pair*2+1]
    let idx0 = batch * seq_len * head_dim + pos * head_dim + pair * 2u;
    let idx1 = idx0 + 1u;

    // Index into freq tables: [pos, pair]
    let freq_idx = pos * half_dim + pair;

    let cos_val = freqs_cos[freq_idx];
    let sin_val = freqs_sin[freq_idx];

    let x0 = x[idx0];
    let x1 = x[idx1];

    // Apply rotation: [x0, x1] @ [[cos, -sin], [sin, cos]]
    x[idx0] = x0 * cos_val - x1 * sin_val;
    x[idx1] = x0 * sin_val + x1 * cos_val;
}

// Precompute frequency tables
// Call once at startup
@compute @workgroup_size(64)
fn compute_freqs(
    @builtin(global_invocation_id) gid: vec3<u32>,
) {
    let pos = gid.y;
    let i = gid.x;

    let seq_len = params.seq_len;
    let half_dim = params.head_dim / 2u;
    let base = params.base;

    if (pos >= seq_len || i >= half_dim) { return; }

    // theta_i = base^(-2i/d)
    let theta = pow(base, -f32(2u * i) / f32(params.head_dim));
    let angle = f32(pos) * theta;

    let idx = pos * half_dim + i;
    freqs_cos[idx] = cos(angle);
    freqs_sin[idx] = sin(angle);
}
```

## Attention

```wgsl
// attention.wgsl
// Scaled dot-product attention with optional causal mask
// Q, K, V shape: [batch, num_heads, seq_len, head_dim]

@group(0) @binding(0) var<storage, read> Q: array<f32>;
@group(0) @binding(1) var<storage, read> K: array<f32>;
@group(0) @binding(2) var<storage, read> V: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;
@group(0) @binding(4) var<uniform> params: AttentionUniform;

var<workgroup> scores: array<f32, 256>;  // Attention scores for one query
var<workgroup> shared_max: f32;
var<workgroup> shared_sum: f32;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let tid = local_id.x;
    let batch = group_id.z;
    let head = group_id.y;
    let query_pos = group_id.x;

    let B = params.batch_size;
    let H = params.num_heads;
    let S = params.seq_len;
    let D = params.head_dim;
    let scale = params.scale;
    let causal = params.causal;

    // Compute attention scores for this query position
    // Score[j] = Q[query_pos] @ K[j]^T * scale

    // Base index for Q[batch, head, query_pos, :]
    let q_base = batch * H * S * D + head * S * D + query_pos * D;

    // Step 1: Compute scores (each thread handles multiple K positions)
    var local_max: f32 = -3.402823e+38;
    for (var j = tid; j < S; j = j + 256u) {
        // Apply causal mask
        if (causal == 1u && j > query_pos) {
            scores[j % 256u] = -3.402823e+38;  // -inf
            continue;
        }

        let k_base = batch * H * S * D + head * S * D + j * D;

        // Dot product Q @ K^T
        var score: f32 = 0.0;
        for (var d: u32 = 0u; d < D; d = d + 1u) {
            score = score + Q[q_base + d] * K[k_base + d];
        }
        score = score * scale;
        scores[j % 256u] = score;
        local_max = max(local_max, score);
    }

    workgroupBarrier();

    // Step 2: Softmax - find max
    var partial_max: array<f32, 256>;
    partial_max[tid] = local_max;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_max[tid] = max(partial_max[tid], partial_max[tid + s]);
        }
        workgroupBarrier();
    }
    if (tid == 0u) { shared_max = partial_max[0]; }
    workgroupBarrier();

    // Step 3: Softmax - compute exp and sum
    var local_sum: f32 = 0.0;
    for (var j = tid; j < S; j = j + 256u) {
        let exp_score = exp(scores[j % 256u] - shared_max);
        scores[j % 256u] = exp_score;
        local_sum = local_sum + exp_score;
    }

    var partial_sum: array<f32, 256>;
    partial_sum[tid] = local_sum;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_sum[tid] = partial_sum[tid] + partial_sum[tid + s];
        }
        workgroupBarrier();
    }
    if (tid == 0u) { shared_sum = partial_sum[0]; }
    workgroupBarrier();

    // Step 4: Normalize scores
    let inv_sum = 1.0 / shared_sum;
    for (var j = tid; j < S; j = j + 256u) {
        scores[j % 256u] = scores[j % 256u] * inv_sum;
    }
    workgroupBarrier();

    // Step 5: Compute output = scores @ V
    let out_base = batch * H * S * D + head * S * D + query_pos * D;

    // Each thread computes one dimension of output
    for (var d = tid; d < D; d = d + 256u) {
        var acc: f32 = 0.0;
        for (var j: u32 = 0u; j < S; j = j + 1u) {
            let v_idx = batch * H * S * D + head * S * D + j * D + d;
            acc = acc + scores[j % 256u] * V[v_idx];
        }
        output[out_base + d] = acc;
    }
}
```

## Sinkhorn-Knopp (mHC)

For manifold-constrained hyper-connections, we need to project matrices onto the doubly stochastic manifold.

```wgsl
// sinkhorn.wgsl
// Implements Sinkhorn-Knopp algorithm to project a matrix
// onto the Birkhoff polytope (doubly stochastic matrices)
//
// A doubly stochastic matrix has all rows and columns sum to 1.
// Used for mHC (Manifold-Constrained Hyper-Connections).

@group(0) @binding(0) var<storage, read_write> matrix: array<f32>;  // [N, N]
@group(0) @binding(1) var<storage, read_write> row_sums: array<f32>;  // [N]
@group(0) @binding(2) var<storage, read_write> col_sums: array<f32>;  // [N]
@group(0) @binding(3) var<uniform> N: u32;

var<workgroup> partial_sum: array<f32, 256>;

// Step 1: Compute row sums
@compute @workgroup_size(256)
fn compute_row_sums(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let row = global_id.y;
    let tid = local_id.x;

    if (row >= N) { return; }

    // Sum elements in this row
    var local_sum: f32 = 0.0;
    for (var j = tid; j < N; j = j + 256u) {
        local_sum = local_sum + matrix[row * N + j];
    }

    partial_sum[tid] = local_sum;
    workgroupBarrier();

    // Parallel reduction
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_sum[tid] = partial_sum[tid] + partial_sum[tid + s];
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        row_sums[row] = partial_sum[0];
    }
}

// Step 2: Normalize rows (divide by row sum)
@compute @workgroup_size(256)
fn normalize_rows(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.y;
    let col = gid.x;

    if (row >= N || col >= N) { return; }

    let row_sum = row_sums[row];
    if (row_sum > 1e-10) {
        matrix[row * N + col] = matrix[row * N + col] / row_sum;
    }
}

// Step 3: Compute column sums
@compute @workgroup_size(256)
fn compute_col_sums(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let col = global_id.y;
    let tid = local_id.x;

    if (col >= N) { return; }

    // Sum elements in this column
    var local_sum: f32 = 0.0;
    for (var i = tid; i < N; i = i + 256u) {
        local_sum = local_sum + matrix[i * N + col];
    }

    partial_sum[tid] = local_sum;
    workgroupBarrier();

    // Parallel reduction
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_sum[tid] = partial_sum[tid] + partial_sum[tid + s];
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        col_sums[col] = partial_sum[0];
    }
}

// Step 4: Normalize columns (divide by column sum)
@compute @workgroup_size(256)
fn normalize_cols(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.y;
    let col = gid.x;

    if (row >= N || col >= N) { return; }

    let col_sum = col_sums[col];
    if (col_sum > 1e-10) {
        matrix[row * N + col] = matrix[row * N + col] / col_sum;
    }
}
```

### Sinkhorn-Knopp Host Code

```rust
// Rust wrapper for Sinkhorn-Knopp iteration
impl WebGpuDevice {
    pub fn sinkhorn_knopp(&self, matrix: &Tensor, iterations: usize) -> Tensor {
        let n = matrix.shape().dims[0] as usize;
        assert_eq!(n, matrix.shape().dims[1] as usize, "Matrix must be square");

        // Create working buffers
        let row_sums = Tensor::zeros(self, Shape::vec(n), DType::F32);
        let col_sums = Tensor::zeros(self, Shape::vec(n), DType::F32);

        // Load pipelines
        let row_sum_pipeline = self.get_pipeline(SINKHORN_SHADER, "compute_row_sums");
        let normalize_rows_pipeline = self.get_pipeline(SINKHORN_SHADER, "normalize_rows");
        let col_sum_pipeline = self.get_pipeline(SINKHORN_SHADER, "compute_col_sums");
        let normalize_cols_pipeline = self.get_pipeline(SINKHORN_SHADER, "normalize_cols");

        // Create bind groups
        let bind_group = self.create_sinkhorn_bind_group(matrix, &row_sums, &col_sums, n);

        for _ in 0..iterations {
            // Row normalization
            self.dispatch(&row_sum_pipeline, &bind_group, [1, n as u32, 1]);
            self.dispatch(&normalize_rows_pipeline, &bind_group, [
                (n as u32 + 255) / 256,
                n as u32,
                1
            ]);

            // Column normalization
            self.dispatch(&col_sum_pipeline, &bind_group, [1, n as u32, 1]);
            self.dispatch(&normalize_cols_pipeline, &bind_group, [
                (n as u32 + 255) / 256,
                n as u32,
                1
            ]);
        }

        matrix.clone()
    }
}
```

## Dispatch Helper

Helper function to calculate workgroup counts:

```rust
impl WebGpuDevice {
    /// Calculate number of workgroups needed
    pub fn workgroups_for(&self, total: usize, workgroup_size: usize) -> u32 {
        ((total + workgroup_size - 1) / workgroup_size) as u32
    }

    /// Execute matmul: C = A @ B
    pub fn matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        let m = a.shape().dims[0] as usize;
        let k = a.shape().dims[1] as usize;
        let n = b.shape().dims[1] as usize;

        assert_eq!(k, b.shape().dims[0] as usize, "Inner dimensions must match");

        let c = Tensor::zeros(self, Shape::matrix(m, n), DType::F32);

        let pipeline = self.get_pipeline(GEMM_SHADER, "main");

        let params = MatmulUniform {
            M: m as u32,
            N: n as u32,
            K: k as u32,
            _pad: 0,
        };

        let bind_group = self.create_matmul_bind_group(a, b, &c, &params);

        // Dispatch with 16x16 workgroups
        self.dispatch(&pipeline, &bind_group, [
            (n as u32 + 15) / 16,
            (m as u32 + 15) / 16,
            1
        ]);

        c
    }

    /// Execute softmax over last dimension
    pub fn softmax(&self, input: &Tensor) -> Tensor {
        let shape = input.shape().clone();
        let last_dim = shape.dims[(shape.ndim - 1) as usize] as usize;
        let batch_size: usize = shape.numel() / last_dim;

        let output = Tensor::zeros(self, shape, DType::F32);

        let pipeline = self.get_pipeline(SOFTMAX_SHADER, "main");

        let params = NormUniform {
            size: last_dim as u32,
            eps: 0.0,
            _pad0: 0,
            _pad1: 0,
        };

        let bind_group = self.create_norm_bind_group(input, &output, &params);

        // One workgroup per batch element
        self.dispatch(&pipeline, &bind_group, [batch_size as u32, 1, 1]);

        output
    }
}
```

## Performance Tips

1. **Tile sizes**: Use 16x16 for GEMM on most GPUs. Some may benefit from 8x8 or 32x32.

2. **Memory coalescing**: Access adjacent memory locations in adjacent threads. The GEMM kernel loads tiles collaboratively for this reason.

3. **Workgroup barriers**: Minimize `workgroupBarrier()` calls. Each barrier synchronizes all threads.

4. **Avoid divergence**: Minimize `if` statements that cause different threads to take different paths.

5. **Use shared memory**: `var<workgroup>` is much faster than global memory for data shared within a workgroup.

6. **Fused operations**: Combine operations when possible (e.g., fused attention instead of separate Q@K, softmax, @V).

7. **Profile**: Use browser DevTools to profile WebGPU workloads. Look for memory bottlenecks.
