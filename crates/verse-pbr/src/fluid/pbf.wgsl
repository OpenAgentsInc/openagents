// Position-based fluid on the GPU (#11660): the same steps as the CPU
// reference in `physics::fluid::pbf`, in f32. Neighbours come from a
// counting-sort grid rebuilt every step, so the order particles are summed
// in varies between runs: results match the CPU statistically, not bit
// for bit. Reimplemented from Macklin and Müller, "Position Based Fluids"
// (2013), and Akinci et al. (2012) boundary particles. Genesis
// (https://github.com/Genesis-Embodied-AI/Genesis, Apache-2.0) ships a PBF
// liquid and is credited as the prompt; none of its code is translated.

struct Params {
    n: u32,
    ncells: u32,
    nblocks: u32,
    flip: u32,
    dims: vec3<u32>,
    iterations: u32,
    origin: vec3<f32>,
    h: f32,
    tank_min: vec3<f32>,
    radius: f32,
    tank_max: vec3<f32>,
    dt: f32,
    gravity: vec3<f32>,
    mass: f32,
    rho0: f32,
    eps: f32,
    poly6: f32,
    spiky: f32,
    viscosity: f32,
    tensile: f32,
    w_dq: f32,
    mq: f32,
}

@group(0) @binding(0) var<uniform> p: Params;
// pos [0, n), vel [n, 2n), predicted A [2n, 3n), B [3n, 4n), velocity
// before smoothing [4n, 5n).
@group(0) @binding(1) var<storage, read_write> state: array<vec4<f32>>;
// cell of each particle [0, n), rank in its cell [n, 2n), sorted particle
// indices [2n, 3n), cell starts [3n, 3n + ncells + 1), block sums after
// that (nblocks), then boundary cell starts (ncells + 1).
@group(0) @binding(2) var<storage, read_write> ints: array<u32>;
@group(0) @binding(3) var<storage, read_write> counts: array<atomic<u32>>;
// lambda [0, n), density [n, 2n).
@group(0) @binding(4) var<storage, read_write> scalars: array<f32>;
// Boundary particles sorted by cell: xyz position, w volume weight psi.
@group(0) @binding(5) var<storage, read> boundary: array<vec4<f32>>;

const WG: u32 = 128u;
const BLOCK: u32 = 256u;

fn src() -> u32 {
    return select(2u, 3u, p.flip == 1u) * p.n;
}
fn dst() -> u32 {
    return select(3u, 2u, p.flip == 1u) * p.n;
}
fn starts() -> u32 {
    return 3u * p.n;
}
fn blocks() -> u32 {
    return 3u * p.n + p.ncells + 1u;
}
fn bstarts() -> u32 {
    return 3u * p.n + p.ncells + 1u + p.nblocks;
}

fn cell_coord(x: vec3<f32>) -> vec3<i32> {
    let c = vec3<i32>(floor((x - p.origin) / p.h));
    return clamp(c, vec3<i32>(0), vec3<i32>(p.dims) - vec3<i32>(1));
}
fn cell_index(c: vec3<i32>) -> u32 {
    return u32(c.x) + p.dims.x * (u32(c.y) + p.dims.y * u32(c.z));
}
fn in_grid(c: vec3<i32>) -> bool {
    return all(c >= vec3<i32>(0)) && all(c < vec3<i32>(p.dims));
}

fn w(r2: f32) -> f32 {
    if r2 >= p.h * p.h {
        return 0.0;
    }
    let d = p.h * p.h - r2;
    return p.poly6 * d * d * d;
}
fn grad(x: vec3<f32>) -> vec3<f32> {
    let r = length(x);
    if r >= p.h || r <= 1e-9 {
        return vec3<f32>(0.0);
    }
    let d = p.h - r;
    return x * (p.spiky * d * d / r);
}
fn clamp_tank(x: vec3<f32>) -> vec3<f32> {
    let r = p.radius;
    return vec3<f32>(
        clamp(x.x, p.tank_min.x + r, p.tank_max.x - r),
        max(x.y, p.tank_min.y + r),
        clamp(x.z, p.tank_min.z + r, p.tank_max.z - r),
    );
}

@compute @workgroup_size(WG)
fn predict(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    let v = state[p.n + i].xyz + p.gravity * p.dt;
    state[p.n + i] = vec4<f32>(v, 0.0);
    state[2u * p.n + i] = vec4<f32>(clamp_tank(state[i].xyz + v * p.dt), 0.0);
}

@compute @workgroup_size(WG)
fn clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < p.ncells {
        atomicStore(&counts[id.x], 0u);
    }
}

@compute @workgroup_size(WG)
fn count(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    let c = cell_index(cell_coord(state[2u * p.n + i].xyz));
    ints[i] = c;
    ints[p.n + i] = atomicAdd(&counts[c], 1u);
}

var<workgroup> scan: array<u32, BLOCK>;

// Exclusive prefix sums of the cell counts within each block of 256.
@compute @workgroup_size(BLOCK)
fn scan_blocks(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_id) local: vec3<u32>, @builtin(workgroup_id) group: vec3<u32>) {
    let c = id.x;
    var value = 0u;
    if c < p.ncells {
        value = atomicLoad(&counts[c]);
    }
    scan[local.x] = value;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if offset >= BLOCK {
            break;
        }
        var add = 0u;
        if local.x >= offset {
            add = scan[local.x - offset];
        }
        workgroupBarrier();
        scan[local.x] = scan[local.x] + add;
        workgroupBarrier();
        offset = offset * 2u;
    }
    if c < p.ncells {
        ints[starts() + c] = scan[local.x] - value;
    }
    if local.x == BLOCK - 1u {
        ints[blocks() + group.x] = scan[local.x];
    }
}

// The block totals, scanned on one thread: a few thousand additions.
@compute @workgroup_size(1)
fn scan_totals() {
    var total = 0u;
    for (var b = 0u; b < p.nblocks; b = b + 1u) {
        let t = ints[blocks() + b];
        ints[blocks() + b] = total;
        total = total + t;
    }
    ints[starts() + p.ncells] = total;
}

@compute @workgroup_size(BLOCK)
fn scan_add(@builtin(global_invocation_id) id: vec3<u32>, @builtin(workgroup_id) group: vec3<u32>) {
    if id.x < p.ncells {
        ints[starts() + id.x] = ints[starts() + id.x] + ints[blocks() + group.x];
    }
}

@compute @workgroup_size(WG)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    ints[2u * p.n + ints[starts() + ints[i]] + ints[p.n + i]] = i;
}

@compute @workgroup_size(WG)
fn lambda(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    let xi = state[src() + i].xyz;
    let home = cell_coord(xi);
    var rho = p.mass * w(0.0);
    var gi = vec3<f32>(0.0);
    var g2 = 0.0;
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let c = home + vec3<i32>(dx, dy, dz);
                if !in_grid(c) {
                    continue;
                }
                let cell = cell_index(c);
                for (var k = ints[starts() + cell]; k < ints[starts() + cell + 1u]; k = k + 1u) {
                    let j = ints[2u * p.n + k];
                    if j == i {
                        continue;
                    }
                    let r = xi - state[src() + j].xyz;
                    let r2 = dot(r, r);
                    if r2 < p.h * p.h {
                        rho = rho + p.mass * w(r2);
                        let g = grad(r) * p.mq;
                        gi = gi + g;
                        g2 = g2 + dot(g, g);
                    }
                }
                for (var k = ints[bstarts() + cell]; k < ints[bstarts() + cell + 1u]; k = k + 1u) {
                    let b = boundary[k];
                    let r = xi - b.xyz;
                    let r2 = dot(r, r);
                    if r2 < p.h * p.h {
                        rho = rho + b.w * w(r2);
                        gi = gi + grad(r) * (b.w / p.rho0);
                    }
                }
            }
        }
    }
    let c = max(rho / p.rho0 - 1.0, 0.0);
    scalars[i] = -c / (dot(gi, gi) + g2 + p.eps);
    scalars[p.n + i] = rho;
}

@compute @workgroup_size(WG)
fn delta(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    let xi = state[src() + i].xyz;
    let li = scalars[i];
    let home = cell_coord(xi);
    var dp = vec3<f32>(0.0);
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let c = home + vec3<i32>(dx, dy, dz);
                if !in_grid(c) {
                    continue;
                }
                let cell = cell_index(c);
                for (var k = ints[starts() + cell]; k < ints[starts() + cell + 1u]; k = k + 1u) {
                    let j = ints[2u * p.n + k];
                    if j == i {
                        continue;
                    }
                    let r = xi - state[src() + j].xyz;
                    let r2 = dot(r, r);
                    if r2 < p.h * p.h {
                        var corr = 0.0;
                        if p.tensile > 0.0 {
                            let q = w(r2) / p.w_dq;
                            corr = -p.tensile * q * q * q * q;
                        }
                        dp = dp + grad(r) * (p.mq * (li + scalars[j] + corr));
                    }
                }
                for (var k = ints[bstarts() + cell]; k < ints[bstarts() + cell + 1u]; k = k + 1u) {
                    let b = boundary[k];
                    let r = xi - b.xyz;
                    if dot(r, r) < p.h * p.h {
                        dp = dp + grad(r) * (b.w / p.rho0 * 2.0 * li);
                    }
                }
            }
        }
    }
    state[dst() + i] = vec4<f32>(clamp_tank(xi + dp), 0.0);
}

@compute @workgroup_size(WG)
fn velocity(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    state[4u * p.n + i] = vec4<f32>((state[2u * p.n + i].xyz - state[i].xyz) / p.dt, 0.0);
}

@compute @workgroup_size(WG)
fn xsph(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= p.n {
        return;
    }
    let xi = state[2u * p.n + i].xyz;
    let vi = state[4u * p.n + i].xyz;
    let home = cell_coord(xi);
    var dv = vec3<f32>(0.0);
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let c = home + vec3<i32>(dx, dy, dz);
                if !in_grid(c) {
                    continue;
                }
                let cell = cell_index(c);
                for (var k = ints[starts() + cell]; k < ints[starts() + cell + 1u]; k = k + 1u) {
                    let j = ints[2u * p.n + k];
                    if j == i {
                        continue;
                    }
                    let r = xi - state[2u * p.n + j].xyz;
                    let r2 = dot(r, r);
                    if r2 < p.h * p.h {
                        dv = dv + (state[4u * p.n + j].xyz - vi) * (p.mass / scalars[p.n + j] * w(r2));
                    }
                }
            }
        }
    }
    state[p.n + i] = vec4<f32>(vi + dv * p.viscosity, 0.0);
    state[i] = vec4<f32>(xi, 0.0);
}
