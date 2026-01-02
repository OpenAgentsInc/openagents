struct MatmulUniform {
    M: u32,
    N: u32,
    K: u32,
    _pad: u32,
};

@group(0) @binding(0) var<storage, read> A: array<f32>;
@group(0) @binding(1) var<storage, read> B: array<f32>;
@group(0) @binding(2) var<storage, read_write> C: array<f32>;
@group(0) @binding(3) var<uniform> params: MatmulUniform;

const TILE_SIZE: u32 = 16u;

var<workgroup> tile_A: array<f32, 256>;
var<workgroup> tile_B: array<f32, 256>;

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
        let a_col = t * TILE_SIZE + local_col;
        if (row < params.M && a_col < params.K) {
            tile_A[local_row * TILE_SIZE + local_col] = A[row * params.K + a_col];
        } else {
            tile_A[local_row * TILE_SIZE + local_col] = 0.0;
        }

        let b_row = t * TILE_SIZE + local_row;
        if (b_row < params.K && col < params.N) {
            tile_B[local_row * TILE_SIZE + local_col] = B[b_row * params.N + col];
        } else {
            tile_B[local_row * TILE_SIZE + local_col] = 0.0;
        }

        workgroupBarrier();

        for (var k: u32 = 0u; k < TILE_SIZE; k = k + 1u) {
            acc = acc + tile_A[local_row * TILE_SIZE + k] * tile_B[k * TILE_SIZE + local_col];
        }

        workgroupBarrier();
    }

    if (row < params.M && col < params.N) {
        C[row * params.N + col] = acc;
    }
}
