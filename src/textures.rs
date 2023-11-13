use crate::gpu::Gpu;

pub struct Textures {
    _forest_texture: wgpu::Texture,
    forest_view: wgpu::TextureView,
    _city_texture: wgpu::Texture,
    city_view: wgpu::TextureView,
    _wheat_texture: wgpu::Texture,
    wheat_view: wgpu::TextureView,
    _river_texture: wgpu::Texture,
    river_view: wgpu::TextureView,

    texture_sampler: wgpu::Sampler,
}

impl Textures {
    fn load_texture(path: &str, gpu: &Gpu) -> wgpu::Texture {
        let image = image::io::Reader::open(path).unwrap().decode().unwrap();
        let image = image.to_rgba8();
        gpu.upload_texture(path, image)
    }

    pub fn new(gpu: &Gpu) -> Self {
        // Textures tutorial:
        // https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/#the-bindgroup
        let forest_texture = Self::load_texture("seamless-forest.jpg", gpu);
        let forest_view = forest_texture.create_view(&Default::default());
        let city_texture = Self::load_texture("seamless-city.jpg", gpu);
        let city_view = city_texture.create_view(&Default::default());
        let river_texture = Self::load_texture("seamless-river.jpg", gpu);
        let river_view = river_texture.create_view(&Default::default());
        let wheat_texture = Self::load_texture("seamless-wheat.jpg", gpu);
        let wheat_view = wheat_texture.create_view(&Default::default());

        let texture_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            _forest_texture: forest_texture,
            forest_view,
            _city_texture: city_texture,
            city_view,
            _wheat_texture: wheat_texture,
            wheat_view,
            _river_texture: river_texture,
            river_view,
            texture_sampler,
        }
    }

    pub fn binding_resources(&self) -> impl Iterator<Item = (u32, wgpu::BindingResource)> {
        [
            (2, wgpu::BindingResource::Sampler(&self.texture_sampler)),
            (3, wgpu::BindingResource::TextureView(&self.forest_view)),
            (4, wgpu::BindingResource::TextureView(&self.city_view)),
            (5, wgpu::BindingResource::TextureView(&self.wheat_view)),
            (6, wgpu::BindingResource::TextureView(&self.river_view)),
        ]
        .into_iter()
    }
}
