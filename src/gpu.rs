use wgpu::util::DeviceExt;
use winit::window::Window;

#[derive(Clone, Copy)]
pub enum SizeOrContent<'a> {
    Size(u64),
    _Content(&'a [u8]),
}

pub struct Buffer {
    size: u64,
    buffer: wgpu::Buffer,
}

impl Buffer {
    pub fn binding(&self) -> wgpu::BindingResource {
        self.buffer.as_entire_binding()
    }

    pub fn write<'a>(&'a self, gpu: &'a Gpu) -> wgpu::QueueWriteBufferView {
        let view_buffer_size = self.size.try_into().unwrap();
        gpu.queue
            .write_buffer_with(&self.buffer, 0, view_buffer_size)
            .expect("Failed to create buffer view")
    }
}

pub struct Gpu {
    _instance: wgpu::Instance,

    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    swapchain_format: wgpu::TextureFormat,

    _adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub async fn new(window: &Window) -> Self {
        let instance = wgpu::Instance::default();

        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::default(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    // limits: wgpu::Limits::downlevel_webgl2_defaults()
                    //     .using_resolution(adapter.limits()),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        surface.configure(&device, &surface_config);

        Self {
            _instance: instance,
            surface,
            surface_config,
            swapchain_format,
            _adapter: adapter,
            device,
            queue,
        }
    }

    pub fn swapchain_format(&self) -> wgpu::TextureFormat {
        self.swapchain_format
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn upload_texture(
        &self,
        path: &str,
        image: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> wgpu::Texture {
        let dimensions = image.dimensions();
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some(path),
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );
        texture
    }

    pub fn create_buffer(
        &self,
        label: &str,
        usage: wgpu::BufferUsages,
        size_or_content: &SizeOrContent,
    ) -> Buffer {
        match size_or_content {
            SizeOrContent::Size(size) => {
                let size = *size;
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    usage,
                    size,
                    mapped_at_creation: false,
                });
                Buffer { size, buffer }
            }
            SizeOrContent::_Content(contents) => {
                let size = contents.len() as u64;
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        usage,
                        contents,
                    });
                Buffer { size, buffer }
            }
        }
    }

    pub fn get_current_texture(&self) -> (wgpu::SurfaceTexture, wgpu::TextureView) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swapchain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        (frame, view)
    }

    pub fn create_encoder(&self) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None })
    }
}
