struct _MatrixStorage_float4x4_ColMajorstd140_0
{
    @align(16) data_0 : array<vec4<f32>, i32(4)>,
};

struct Frame_std140_0
{
    @align(16) view_proj_0 : _MatrixStorage_float4x4_ColMajorstd140_0,
    @align(16) view_proj_nj_0 : _MatrixStorage_float4x4_ColMajorstd140_0,
    @align(16) prev_view_proj_0 : _MatrixStorage_float4x4_ColMajorstd140_0,
    @align(16) inv_view_proj_0 : _MatrixStorage_float4x4_ColMajorstd140_0,
    @align(16) camera_0 : vec4<f32>,
    @align(16) sun_0 : vec4<f32>,
    @align(16) planes_0 : array<vec4<f32>, i32(6)>,
    @align(16) viewport_0 : vec4<f32>,
    @align(16) jitter_0 : vec4<f32>,
    @align(16) flags_0 : vec4<u32>,
    @align(16) lod_0 : vec4<f32>,
    @align(16) tiers_0 : vec4<f32>,
};

@binding(0) @group(0) var<uniform> frame_0 : Frame_std140_0;
@binding(8) @group(0) var<storage, read_write> list_b_0 : array<u32>;

@binding(4) @group(0) var<storage, read_write> visibility_0 : array<u32>;

@binding(9) @group(0) var<storage, read> bucket_base_0 : array<u32>;

@binding(6) @group(0) var<storage, read_write> list_a_0 : array<u32>;

@binding(10) @group(0) var hiz_0 : texture_2d<f32>;

@binding(5) @group(0) var<storage, read_write> args_a_0 : array<atomic<u32>>;

@binding(7) @group(0) var<storage, read_write> args_b_0 : array<atomic<u32>>;

@binding(11) @group(0) var<storage, read_write> stats_0 : array<atomic<u32>>;

struct MeshLod_std430_0
{
    @align(4) first_index_0 : u32,
    @align(4) index_count_0 : u32,
    @align(4) base_vertex_0 : u32,
    @align(4) first_cluster_0 : u32,
    @align(4) cluster_count_0 : u32,
    @align(4) pad0_0 : u32,
    @align(4) pad1_0 : u32,
    @align(4) pad2_0 : u32,
};

@binding(3) @group(0) var<storage, read> lods_0 : array<MeshLod_std430_0>;

struct MeshBounds_std430_0
{
    @align(16) center_0 : vec3<f32>,
    @align(4) radius_0 : f32,
};

@binding(2) @group(0) var<storage, read> bounds_0 : array<MeshBounds_std430_0>;

struct Instance_std430_0
{
    @align(16) position_0 : vec3<f32>,
    @align(4) scale_0 : f32,
    @align(16) yaw_0 : f32,
    @align(4) height_scale_0 : f32,
    @align(8) mesh_0 : u32,
    @align(4) material_0 : u32,
};

@binding(1) @group(0) var<storage, read> instances_0 : array<Instance_std430_0>;

fn touch_all_0()
{
    if((frame_0.flags_0.z) == u32(4294967295))
    {
        const _S1 : vec3<i32> = vec3<i32>(i32(0), i32(0), i32(0));
        var _S2 : u32 = visibility_0[i32(0)] + bucket_base_0[i32(0)] + list_a_0[i32(0)] + u32((textureLoad((hiz_0), ((_S1)).xy, ((_S1)).z).x));
        var _S3 : u32 = atomicLoad(&(args_a_0[i32(0)]));
        var _S4 : u32 = _S2 + _S3;
        var _S5 : u32 = atomicLoad(&(args_b_0[i32(0)]));
        var _S6 : u32 = _S4 + _S5;
        var _S7 : u32 = atomicLoad(&(stats_0[i32(0)]));
        list_b_0[i32(0)] = _S6 + _S7 + lods_0[i32(0)].index_count_0 + u32(bounds_0[i32(0)].radius_0);
    }
    return;
}

fn instance_index_0( id_0 : vec3<u32>) -> u32
{
    return id_0.x + id_0.y * u32(65535) * u32(64);
}

fn instance_point_0( inst_0 : ptr<function, Instance_std430_0>,  p_0 : vec3<f32>) -> vec3<f32>
{
    var q_0 : vec3<f32> = p_0 * vec3<f32>((*inst_0).scale_0, (*inst_0).scale_0 * (*inst_0).height_scale_0, (*inst_0).scale_0);
    var c_0 : f32 = cos((*inst_0).yaw_0);
    var n_0 : f32 = sin((*inst_0).yaw_0);
    var _S8 : f32 = q_0.x;
    var _S9 : f32 = q_0.z;
    return vec3<f32>(c_0 * _S8 - n_0 * _S9, q_0.y, n_0 * _S8 + c_0 * _S9) + (*inst_0).position_0;
}

fn select_lod_0( center_1 : vec3<f32>,  radius_1 : f32) -> u32
{
    if((frame_0.flags_0.x) == u32(0))
    {
        return u32(0);
    }
    var pixels_0 : f32 = radius_1 / max(distance(center_1, frame_0.camera_0.xyz), 0.10000000149011612f) * frame_0.lod_0.x * frame_0.lod_0.y;
    if((pixels_0 > 160.0f))
    {
        return u32(0);
    }
    if((pixels_0 > 60.0f))
    {
        return u32(1);
    }
    if((pixels_0 > 24.0f))
    {
        return u32(2);
    }
    return u32(3);
}

struct Test_0
{
     center_2 : vec3<f32>,
     radius_2 : f32,
     lod_1 : u32,
};

fn prepare_0( index_0 : u32) -> Test_0
{
    var _S10 : Instance_std430_0 = instances_0[index_0];
    var b_0 : MeshBounds_std430_0 = bounds_0[_S10.mesh_0];
    var t_0 : Test_0;
    var _S11 : vec3<f32> = instance_point_0(&(_S10), b_0.center_0);
    t_0.center_2 = _S11;
    var _S12 : f32 = b_0.radius_0 * _S10.scale_0 * max(1.0f, _S10.height_scale_0);
    t_0.radius_2 = _S12;
    t_0.lod_1 = select_lod_0(_S11, _S12);
    return t_0;
}

fn sphere_in_frustum_0( center_3 : vec3<f32>,  radius_3 : f32) -> bool
{
    var i_0 : u32 = u32(0);
    loop
    {
        if((i_0 < u32(6)))
        {
        }
        else
        {
            break;
        }
        if(((dot(frame_0.planes_0[i_0].xyz, center_3) + frame_0.planes_0[i_0].w) < (- radius_3)))
        {
            return false;
        }
        i_0 = i_0 + u32(1);
    }
    return true;
}

fn count_visible_0( index_1 : u32,  lod_2 : u32)
{
    var _S13 : u32 = atomicAdd(&(stats_0[i32(0)]), u32(1));
    var _S14 : u32 = atomicAdd(&(stats_0[i32(1)]), lods_0[instances_0[index_1].mesh_0 * u32(4) + lod_2].index_count_0 / u32(3));
    return;
}

fn emit_bucket_a_0( index_2 : u32,  lod_3 : u32)
{
    var bucket_0 : u32 = instances_0[index_2].mesh_0 * u32(4) + lod_3;
    var slot_0 : u32 = atomicAdd(&(args_a_0[bucket_0 * u32(5) + u32(1)]), u32(1));
    list_a_0[bucket_base_0[bucket_0] + slot_0] = index_2;
    count_visible_0(index_2, lod_3);
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn single_buckets(@builtin(global_invocation_id) id_1 : vec3<u32>)
{
    touch_all_0();
    var index_3 : u32 = instance_index_0(id_1);
    if((index_3 >= (frame_0.flags_0.z)))
    {
        return;
    }
    var t_1 : Test_0 = prepare_0(index_3);
    if(sphere_in_frustum_0(t_1.center_2, t_1.radius_2))
    {
        emit_bucket_a_0(index_3, t_1.lod_1);
    }
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn phase1_buckets(@builtin(global_invocation_id) id_2 : vec3<u32>)
{
    touch_all_0();
    var index_4 : u32 = instance_index_0(id_2);
    var _S15 : bool;
    if((index_4 >= (frame_0.flags_0.z)))
    {
        _S15 = true;
    }
    else
    {
        _S15 = visibility_0[index_4] == u32(0);
    }
    if(_S15)
    {
        return;
    }
    var t_2 : Test_0 = prepare_0(index_4);
    if(sphere_in_frustum_0(t_2.center_2, t_2.radius_2))
    {
        emit_bucket_a_0(index_4, t_2.lod_1);
    }
    return;
}

fn mip_size_0( level_0 : u32) -> vec2<u32>
{
    var w_0 : u32;
    var h_0 : u32;
    var levels_0 : u32;
    {var dim = textureDimensions((hiz_0), (level_0));((w_0)) = dim.x;((h_0)) = dim.y;((levels_0)) = textureNumLevels((hiz_0));};
    return vec2<u32>(w_0, h_0);
}

fn hiz_at_0( p_1 : vec2<u32>,  level_1 : u32) -> f32
{
    var _S16 : vec3<i32> = vec3<i32>(vec2<i32>(p_1), i32(level_1));
    return (textureLoad((hiz_0), ((_S16)).xy, ((_S16)).z).x);
}

fn hiz_occluded_0( center_4 : vec3<f32>,  radius_4 : f32) -> bool
{
    const _S17 : vec2<f32> = vec2<f32>(-1.0e+09f, -1.0e+09f);
    var uv_min_0 : vec2<f32> = vec2<f32>(1.0e+09f, 1.0e+09f);
    var uv_max_0 : vec2<f32> = _S17;
    var z_min_0 : f32 = 1.0e+09f;
    var k_0 : u32 = u32(0);
    loop
    {
        if((k_0 < u32(8)))
        {
        }
        else
        {
            break;
        }
        var _S18 : f32;
        if(((k_0 & (u32(1)))) != u32(0))
        {
            _S18 = radius_4;
        }
        else
        {
            _S18 = - radius_4;
        }
        var _S19 : f32;
        if(((k_0 & (u32(2)))) != u32(0))
        {
            _S19 = radius_4;
        }
        else
        {
            _S19 = - radius_4;
        }
        var _S20 : f32;
        if(((k_0 & (u32(4)))) != u32(0))
        {
            _S20 = radius_4;
        }
        else
        {
            _S20 = - radius_4;
        }
        var clip_0 : vec4<f32> = (((vec4<f32>(center_4 + vec3<f32>(_S18, _S19, _S20), 1.0f)) * (mat4x4<f32>(frame_0.view_proj_nj_0.data_0[i32(0)][i32(0)], frame_0.view_proj_nj_0.data_0[i32(1)][i32(0)], frame_0.view_proj_nj_0.data_0[i32(2)][i32(0)], frame_0.view_proj_nj_0.data_0[i32(3)][i32(0)], frame_0.view_proj_nj_0.data_0[i32(0)][i32(1)], frame_0.view_proj_nj_0.data_0[i32(1)][i32(1)], frame_0.view_proj_nj_0.data_0[i32(2)][i32(1)], frame_0.view_proj_nj_0.data_0[i32(3)][i32(1)], frame_0.view_proj_nj_0.data_0[i32(0)][i32(2)], frame_0.view_proj_nj_0.data_0[i32(1)][i32(2)], frame_0.view_proj_nj_0.data_0[i32(2)][i32(2)], frame_0.view_proj_nj_0.data_0[i32(3)][i32(2)], frame_0.view_proj_nj_0.data_0[i32(0)][i32(3)], frame_0.view_proj_nj_0.data_0[i32(1)][i32(3)], frame_0.view_proj_nj_0.data_0[i32(2)][i32(3)], frame_0.view_proj_nj_0.data_0[i32(3)][i32(3)]))));
        var _S21 : f32 = clip_0.w;
        if((_S21 <= 0.05000000074505806f))
        {
            return false;
        }
        var ndc_0 : vec3<f32> = clip_0.xyz / vec3<f32>(_S21);
        var uv_0 : vec2<f32> = vec2<f32>(ndc_0.x * 0.5f + 0.5f, 0.5f - ndc_0.y * 0.5f);
        var _S22 : vec2<f32> = min(uv_min_0, uv_0);
        var _S23 : vec2<f32> = max(uv_max_0, uv_0);
        var _S24 : f32 = min(z_min_0, ndc_0.z);
        var _S25 : u32 = k_0 + u32(1);
        uv_min_0 = _S22;
        uv_max_0 = _S23;
        z_min_0 = _S24;
        k_0 = _S25;
    }
    const _S26 : vec2<f32> = vec2<f32>(0.0f, 0.0f);
    const _S27 : vec2<f32> = vec2<f32>(1.0f, 1.0f);
    var uv_min_1 : vec2<f32> = clamp(uv_min_0, _S26, _S27);
    var uv_max_1 : vec2<f32> = clamp(uv_max_0, _S26, _S27);
    var extent_0 : vec2<f32> = (uv_max_1 - uv_min_1) * vec2<f32>(mip_size_0(u32(0)));
    var _S28 : u32 = min(u32(ceil(log2(max(max(extent_0.x, extent_0.y), 1.0f)))), u32(frame_0.lod_0.z) - u32(1));
    var dims_0 : vec2<u32> = mip_size_0(_S28);
    var _S29 : vec2<f32> = vec2<f32>(dims_0);
    var _S30 : vec2<u32> = dims_0 - vec2<u32>(u32(1));
    var _S31 : vec2<u32> = min(vec2<u32>(uv_min_1 * _S29), _S30);
    var _S32 : vec2<u32> = min(vec2<u32>(uv_max_1 * _S29), _S30);
    return (z_min_0 > (max(max(hiz_at_0(_S31, _S28), hiz_at_0(vec2<u32>(_S32.x, _S31.y), _S28)), max(hiz_at_0(vec2<u32>(_S31.x, _S32.y), _S28), hiz_at_0(_S32, _S28)))));
}

fn phase_two_0( index_5 : u32,  t_3 : Test_0) -> u32
{
    if(!sphere_in_frustum_0(t_3.center_2, t_3.radius_2))
    {
        visibility_0[index_5] = u32(0);
        return u32(0);
    }
    if(hiz_occluded_0(t_3.center_2, t_3.radius_2))
    {
        visibility_0[index_5] = u32(0);
        var _S33 : u32 = atomicAdd(&(stats_0[i32(6)]), u32(1));
        return u32(0);
    }
    var was_0 : u32 = visibility_0[index_5];
    visibility_0[index_5] = u32(1);
    var _S34 : i32;
    if(was_0 != u32(0))
    {
        _S34 = i32(2);
    }
    else
    {
        _S34 = i32(1);
    }
    return u32(_S34);
}

fn emit_bucket_b_0( index_6 : u32,  lod_4 : u32)
{
    var bucket_1 : u32 = instances_0[index_6].mesh_0 * u32(4) + lod_4;
    var slot_1 : u32 = atomicAdd(&(args_b_0[bucket_1 * u32(5) + u32(1)]), u32(1));
    list_b_0[bucket_base_0[bucket_1] + slot_1] = index_6;
    count_visible_0(index_6, lod_4);
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn phase2_buckets(@builtin(global_invocation_id) id_3 : vec3<u32>)
{
    touch_all_0();
    var index_7 : u32 = instance_index_0(id_3);
    if((index_7 >= (frame_0.flags_0.z)))
    {
        return;
    }
    var t_4 : Test_0 = prepare_0(index_7);
    var _S35 : u32 = phase_two_0(index_7, t_4);
    if(_S35 == u32(1))
    {
        emit_bucket_b_0(index_7, t_4.lod_1);
    }
    return;
}

fn emit_list_a_0( index_8 : u32,  lod_5 : u32)
{
    var slot_2 : u32 = atomicAdd(&(args_a_0[i32(0)]), u32(1));
    var _S36 : u32 = slot_2 * u32(2);
    list_a_0[_S36] = index_8;
    list_a_0[_S36 + u32(1)] = lod_5;
    count_visible_0(index_8, lod_5);
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn single_list(@builtin(global_invocation_id) id_4 : vec3<u32>)
{
    touch_all_0();
    var index_9 : u32 = instance_index_0(id_4);
    if((index_9 >= (frame_0.flags_0.z)))
    {
        return;
    }
    var t_5 : Test_0 = prepare_0(index_9);
    if(sphere_in_frustum_0(t_5.center_2, t_5.radius_2))
    {
        emit_list_a_0(index_9, t_5.lod_1);
    }
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn phase1_list(@builtin(global_invocation_id) id_5 : vec3<u32>)
{
    touch_all_0();
    var index_10 : u32 = instance_index_0(id_5);
    var _S37 : bool;
    if((index_10 >= (frame_0.flags_0.z)))
    {
        _S37 = true;
    }
    else
    {
        _S37 = visibility_0[index_10] == u32(0);
    }
    if(_S37)
    {
        return;
    }
    var t_6 : Test_0 = prepare_0(index_10);
    if(sphere_in_frustum_0(t_6.center_2, t_6.radius_2))
    {
        emit_list_a_0(index_10, t_6.lod_1);
    }
    return;
}

fn emit_list_b_0( index_11 : u32,  lod_6 : u32)
{
    var slot_3 : u32 = atomicAdd(&(args_b_0[i32(0)]), u32(1));
    var _S38 : u32 = slot_3 * u32(2);
    list_b_0[_S38] = index_11;
    list_b_0[_S38 + u32(1)] = lod_6;
    count_visible_0(index_11, lod_6);
    return;
}

@compute
@workgroup_size(64, 1, 1)
fn phase2_list(@builtin(global_invocation_id) id_6 : vec3<u32>)
{
    touch_all_0();
    var index_12 : u32 = instance_index_0(id_6);
    if((index_12 >= (frame_0.flags_0.z)))
    {
        return;
    }
    var t_7 : Test_0 = prepare_0(index_12);
    var _S39 : u32 = phase_two_0(index_12, t_7);
    if(_S39 == u32(1))
    {
        emit_list_b_0(index_12, t_7.lod_1);
    }
    return;
}

