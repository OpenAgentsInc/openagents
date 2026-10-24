// The GPU visibility pass from shaders/cull.wgsl, authored in Slang
// (decision 0008). tools/build-shaders.sh compiles it to
// shaders/generated/cull.slang.wgsl; the lab can run either version, and the
// browser check requires identical images. Bindings and entry points match
// the WGSL version exactly.

static const uint LODS = 4;

struct Frame
{
    column_major float4x4 view_proj;
    column_major float4x4 view_proj_nj;
    column_major float4x4 prev_view_proj;
    column_major float4x4 inv_view_proj;
    float4 camera;
    float4 sun;
    float4 planes[6];
    float4 viewport;
    float4 jitter;
    uint4 flags;
    float4 lod;
    float4 tiers;
};

struct Instance
{
    float3 position;
    float scale;
    float yaw;
    float height_scale;
    uint mesh;
    uint material;
};

struct MeshBounds
{
    float3 center;
    float radius;
};

struct MeshLod
{
    uint first_index;
    uint index_count;
    uint base_vertex;
    uint first_cluster;
    uint cluster_count;
    uint pad0;
    uint pad1;
    uint pad2;
};

[[vk::binding(0, 0)]] ConstantBuffer<Frame> frame;
[[vk::binding(1, 0)]] StructuredBuffer<Instance> instances;
[[vk::binding(2, 0)]] StructuredBuffer<MeshBounds> bounds;
[[vk::binding(3, 0)]] StructuredBuffer<MeshLod> lods;
[[vk::binding(4, 0)]] RWStructuredBuffer<uint> visibility;
[[vk::binding(5, 0)]] RWStructuredBuffer<Atomic<uint>> args_a;
[[vk::binding(6, 0)]] RWStructuredBuffer<uint> list_a;
[[vk::binding(7, 0)]] RWStructuredBuffer<Atomic<uint>> args_b;
[[vk::binding(8, 0)]] RWStructuredBuffer<uint> list_b;
[[vk::binding(9, 0)]] StructuredBuffer<uint> bucket_base;
[[vk::binding(10, 0)]] Texture2D<float> hiz;
[[vk::binding(11, 0)]] RWStructuredBuffer<Atomic<uint>> stats;

struct Test
{
    float3 center;
    float radius;
    uint lod;
};

float3 instance_point(Instance inst, float3 p)
{
    float3 q = p * float3(inst.scale, inst.scale * inst.height_scale, inst.scale);
    float c = cos(inst.yaw);
    float n = sin(inst.yaw);
    return float3(c * q.x - n * q.z, q.y, n * q.x + c * q.z) + inst.position;
}

bool sphere_in_frustum(float3 center, float radius)
{
    for (uint i = 0; i < 6; i++)
    {
        float4 pl = frame.planes[i];
        if (dot(pl.xyz, center) + pl.w < -radius)
            return false;
    }
    return true;
}

uint select_lod(float3 center, float radius)
{
    if (frame.flags.x == 0)
        return 0;
    float dist = max(distance(center, frame.camera.xyz), 0.1);
    float pixels = radius / dist * frame.lod.x * frame.lod.y;
    if (pixels > 160.0) return 0;
    if (pixels > 60.0) return 1;
    if (pixels > 24.0) return 2;
    return 3;
}

Test prepare(uint index)
{
    Instance inst = instances[index];
    MeshBounds b = bounds[inst.mesh];
    Test t;
    t.center = instance_point(inst, b.center);
    t.radius = b.radius * inst.scale * max(1.0, inst.height_scale);
    t.lod = select_lod(t.center, t.radius);
    return t;
}

uint2 mip_size(uint level)
{
    uint w, h, levels;
    hiz.GetDimensions(level, w, h, levels);
    return uint2(w, h);
}

float hiz_at(uint2 p, uint level)
{
    return hiz.Load(int3(int2(p), int(level)));
}

bool hiz_occluded(float3 center, float radius)
{
    float2 uv_min = float2(1.0e9, 1.0e9);
    float2 uv_max = float2(-1.0e9, -1.0e9);
    float z_min = 1.0e9;
    for (uint k = 0; k < 8; k++)
    {
        float3 corner = center + float3(
            (k & 1) != 0 ? radius : -radius,
            (k & 2) != 0 ? radius : -radius,
            (k & 4) != 0 ? radius : -radius);
        float4 clip = mul(frame.view_proj_nj, float4(corner, 1.0));
        if (clip.w <= 0.05)
            return false;
        float3 ndc = clip.xyz / clip.w;
        float2 uv = float2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        uv_min = min(uv_min, uv);
        uv_max = max(uv_max, uv);
        z_min = min(z_min, ndc.z);
    }
    uv_min = clamp(uv_min, float2(0.0, 0.0), float2(1.0, 1.0));
    uv_max = clamp(uv_max, float2(0.0, 0.0), float2(1.0, 1.0));
    float2 extent = (uv_max - uv_min) * float2(mip_size(0));
    uint mips = uint(frame.lod.z);
    uint level = min(uint(ceil(log2(max(max(extent.x, extent.y), 1.0)))), mips - 1);
    uint2 dims = mip_size(level);
    uint2 lo = min(uint2(uv_min * float2(dims)), dims - 1);
    uint2 hi = min(uint2(uv_max * float2(dims)), dims - 1);
    float z = max(max(hiz_at(lo, level), hiz_at(uint2(hi.x, lo.y), level)),
                  max(hiz_at(uint2(lo.x, hi.y), level), hiz_at(hi, level)));
    return z_min > z;
}

void count_visible(uint index, uint lod)
{
    stats[0].add(1);
    stats[1].add(lods[instances[index].mesh * LODS + lod].index_count / 3);
}

void emit_bucket_a(uint index, uint lod)
{
    uint bucket = instances[index].mesh * LODS + lod;
    uint slot = args_a[bucket * 5 + 1].add(1);
    list_a[bucket_base[bucket] + slot] = index;
    count_visible(index, lod);
}

void emit_bucket_b(uint index, uint lod)
{
    uint bucket = instances[index].mesh * LODS + lod;
    uint slot = args_b[bucket * 5 + 1].add(1);
    list_b[bucket_base[bucket] + slot] = index;
    count_visible(index, lod);
}

void emit_list_a(uint index, uint lod)
{
    uint slot = args_a[0].add(1);
    list_a[slot * 2] = index;
    list_a[slot * 2 + 1] = lod;
    count_visible(index, lod);
}

void emit_list_b(uint index, uint lod)
{
    uint slot = args_b[0].add(1);
    list_b[slot * 2] = index;
    list_b[slot * 2 + 1] = lod;
    count_visible(index, lod);
}

// 0 culled, 1 newly visible, 2 still visible from phase 1.
uint phase_two(uint index, Test t)
{
    if (!sphere_in_frustum(t.center, t.radius))
    {
        visibility[index] = 0;
        return 0;
    }
    if (hiz_occluded(t.center, t.radius))
    {
        visibility[index] = 0;
        stats[6].add(1);
        return 0;
    }
    uint was = visibility[index];
    visibility[index] = 1;
    return was != 0 ? 2 : 1;
}

// Keeps every binding statically used in every entry point, like touch_all()
// in the WGSL version, so all six pipelines share one bind group shape. The
// condition is never true at runtime (flags.z is the instance count).
void touch_all()
{
    if (frame.flags.z == 0xffffffffu)
    {
        list_b[0] = visibility[0] + bucket_base[0] + list_a[0] + uint(hiz.Load(int3(0, 0, 0)))
            + args_a[0].load() + args_b[0].load() + stats[0].load() + lods[0].index_count + uint(bounds[0].radius);
    }
}

uint instance_index(uint3 id)
{
    return id.x + id.y * 65535 * 64;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void single_buckets(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z) return;
    Test t = prepare(index);
    if (sphere_in_frustum(t.center, t.radius)) emit_bucket_a(index, t.lod);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void phase1_buckets(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z || visibility[index] == 0) return;
    Test t = prepare(index);
    if (sphere_in_frustum(t.center, t.radius)) emit_bucket_a(index, t.lod);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void phase2_buckets(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z) return;
    Test t = prepare(index);
    if (phase_two(index, t) == 1) emit_bucket_b(index, t.lod);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void single_list(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z) return;
    Test t = prepare(index);
    if (sphere_in_frustum(t.center, t.radius)) emit_list_a(index, t.lod);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void phase1_list(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z || visibility[index] == 0) return;
    Test t = prepare(index);
    if (sphere_in_frustum(t.center, t.radius)) emit_list_a(index, t.lod);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void phase2_list(uint3 id : SV_DispatchThreadID)
{
    touch_all();
    uint index = instance_index(id);
    if (index >= frame.flags.z) return;
    Test t = prepare(index);
    if (phase_two(index, t) == 1) emit_list_b(index, t.lod);
}
