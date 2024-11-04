use crate::gpu::Gpu;

pub struct BindGroups {
    pub layouts: [wgpu::BindGroupLayout; 1],
    pub groups: Option<[wgpu::BindGroup; 1]>,
}

impl BindGroups {
    pub const UNIFORM: wgpu::BindingType = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    pub const STORAGE: wgpu::BindingType = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    pub const SAMPLER: wgpu::BindingType =
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    pub const TEXTURE: wgpu::BindingType = wgpu::BindingType::Texture {
        multisampled: false,
        view_dimension: wgpu::TextureViewDimension::D2,
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
    };

    pub fn new(gpu: &Gpu, entries: &[(u32, wgpu::BindingType)]) -> BindGroups {
        let entries = entries
            .iter()
            .copied()
            .map(|(binding, ty)| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty,
                count: None,
            })
            .collect::<Vec<_>>();
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &entries,
                label: Some("bind_group_layout"),
            });

        Self {
            layouts: [layout],
            groups: Option::default(),
        }
    }

    pub fn generate_bind_groups(&mut self, gpu: &Gpu, entries: &[(u32, wgpu::BindingResource)]) {
        let entries = entries
            .iter()
            .cloned()
            .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource })
            .collect::<Vec<_>>();
        self.groups = Some([gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.layouts[0],
            entries: &entries,
            label: Some("bind_group"),
        })]);
    }
}
