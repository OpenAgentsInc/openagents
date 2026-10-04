//! Bounded skeletal instance storage for dynamic shadow draws.
use super::{GpuVertex, Pose};

pub(super) struct Shadows {
    pub poses: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,
}
pub(super) const INDEX_CAPACITY: u32 = 24 * 1024;
fn source() -> String {
    include_str!("scene.wgsl")
        .split("@fragment fn fs(")
        .next()
        .expect("scene shader has a world fragment entry point")
        .replace("@group(2) @binding(0) var<uniform> pose:Pose;",
            "@group(2) @binding(0) var<storage,read> poses:array<Pose>;\n@group(2) @binding(1) var<storage,read> indices:array<u32>;")
        .replace("@vertex fn vs(v:In)->Out {",
            "@vertex fn vs(v:In,@builtin(instance_index) instance:u32)->Out {\n let pose_index=indices[instance];")
        .replace("pose.", "poses[pose_index].")
}
impl Shadows {
    pub fn new(
        device: &wgpu::Device,
        frame: &wgpu::BindGroupLayout,
        material: &wgpu::BindGroupLayout,
    ) -> Option<Self> {
        let size = std::mem::size_of::<Pose>() as u64 * 1024;
        let limits = device.limits();
        if limits.max_storage_buffers_per_shader_stage < 2
            || u64::from(limits.max_storage_buffer_binding_size) < size
            || limits.max_buffer_size < size
        {
            return None;
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Verse shadow instance storage"),
            entries: &std::array::from_fn::<_, 2, _>(|binding| wgpu::BindGroupLayoutEntry {
                binding: binding as u32,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }),
        });
        let make = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let poses = make("Verse shadow skeletal palettes", size);
        let indices = make("Verse shadow actor indices", u64::from(INDEX_CAPACITY) * 4);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Verse shadow instances"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: poses.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: indices.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Verse instanced skeletal shadows"),
            source: wgpu::ShaderSource::Wgsl(source().into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Verse instanced shadow layout"),
            bind_group_layouts: &[Some(frame), Some(material), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("Verse instanced skeletal shadow"),layout:Some(&pipeline_layout),
            vertex:wgpu::VertexState {module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),
                buffers:&[wgpu::VertexBufferLayout {array_stride:std::mem::size_of::<GpuVertex>() as u64,
                    step_mode:wgpu::VertexStepMode::Vertex,
                    attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32x4,4=>Float32x4,5=>Float32x3]}]},
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},
            depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth32Float,
                depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::LessEqual),
                stencil:Default::default(),bias:wgpu::DepthBiasState {constant:1,slope_scale:1.,clamp:0.}}),
            multisample:Default::default(),fragment:Some(wgpu::FragmentState {module:&shader,
                entry_point:Some("shadow_fs"),compilation_options:Default::default(),targets:&[]}),
            multiview_mask:None,cache:None });
        Some(Self {
            poses,
            indices,
            group,
            pipeline,
        })
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn storage_shader_validates_and_reads_an_instance_index() {
        let text = super::source();
        assert!(!text.contains("var<uniform> pose:"));
        assert!(text.contains("poses[pose_index].bones"));
        let module = naga::front::wgsl::parse_str(&text).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        assert_eq!(
            module
                .entry_points
                .iter()
                .find(|entry| entry.name == "vs")
                .unwrap()
                .function
                .arguments
                .len(),
            2
        );
    }
}
