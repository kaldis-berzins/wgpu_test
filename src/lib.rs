use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer,
};
use wgpu::{util::DeviceExt, MultisampleState};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
    window::WindowBuilder,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct WindowUniform {
    size: [f32; 2],
    scale_factor: f32,
    _padding: f32,
}

#[derive(Clone, Copy)]
struct Fill {
    color: [f32; 4],
}

#[derive(Clone, Copy)]
struct Stroke {
    color: [f32; 3],
    width: f32,
}

struct Rect {
    position: [f32; 2],
    size: [f32; 2],
    border_radius: u32,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    z_index: f32,
    softness: f32,
}

const RECTANGLES: &[Rect] = &[
    Rect {
        position: [200.0, 200.0],
        size: [100.0, 100.0],
        border_radius: 30,
        fill: Some(Fill {
            color: [0.0, 0.0, 0.0, 0.7],
        }),
        stroke: None,
        z_index: 0.5,
        softness: 5.0,
    },
    Rect {
        position: [198.0, 198.0],
        size: [100.0, 100.0],
        border_radius: 30,
        fill: Some(Fill {
            color: [1.0, 0.0, 0.0, 1.0],
        }),
        stroke: None,
        z_index: 0.0,
        softness: 1.0,
    },
];

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    position: [f32; 2],
    z_index: f32,
    color: [f32; 4],
    border_radius: f32,
    rect_pos: [f32; 2],
    rect_size: [f32; 2],
    rect_softness: f32,
}

impl RectVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32,
        2 => Float32x4,
        3 => Float32,
        4 => Float32x2,
        5 => Float32x2,
        6 => Float32,
    ];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Window,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    window_buffer: wgpu::Buffer,
    window_bind_group: wgpu::BindGroup,
    font_system: FontSystem,
    cache: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    buffer: Buffer,
}

impl State {
    async fn new(window: Window) -> Self {
        let size = window.inner_size();
        let window_uniform = WindowUniform {
            size: [size.width as f32, size.height as f32],
            scale_factor: window.scale_factor() as f32,
            _padding: 0.0,
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let num_vertices: u32 = (RECTANGLES.len() * 4) as u32;
        let num_indices: u32 = (RECTANGLES.len() * 6) as u32;
        let mut vertices: Vec<RectVertex> = vec![];
        let mut indices: Vec<u16> = vec![];

        for i in 0..RECTANGLES.len() {
            let rect_i = &RECTANGLES[i];
            vertices.push(RectVertex {
                position: [
                    rect_i.position[0] + rect_i.size[0] / 2.0,
                    rect_i.position[1] - rect_i.size[1] / 2.0,
                ],
                z_index: rect_i.z_index as f32,
                color: rect_i.fill.unwrap().color,
                border_radius: rect_i.border_radius as f32,
                rect_pos: rect_i.position,
                rect_size: rect_i.size,
                rect_softness: rect_i.softness,
            });
            vertices.push(RectVertex {
                position: [
                    rect_i.position[0] + rect_i.size[0] / 2.0,
                    rect_i.position[1] + rect_i.size[1] / 2.0,
                ],
                z_index: rect_i.z_index as f32,
                color: rect_i.fill.unwrap().color,
                border_radius: rect_i.border_radius as f32,
                rect_pos: rect_i.position,
                rect_size: rect_i.size,
                rect_softness: rect_i.softness,
            });
            vertices.push(RectVertex {
                position: [
                    rect_i.position[0] - rect_i.size[0] / 2.0,
                    rect_i.position[1] + rect_i.size[1] / 2.0,
                ],
                z_index: rect_i.z_index as f32,
                color: rect_i.fill.unwrap().color,
                border_radius: rect_i.border_radius as f32,
                rect_pos: rect_i.position,
                rect_size: rect_i.size,
                rect_softness: rect_i.softness,
            });
            vertices.push(RectVertex {
                position: [
                    rect_i.position[0] - rect_i.size[0] / 2.0,
                    rect_i.position[1] - rect_i.size[1] / 2.0,
                ],
                z_index: rect_i.z_index as f32,
                color: rect_i.fill.unwrap().color,
                border_radius: rect_i.border_radius as f32,
                rect_pos: rect_i.position,
                rect_size: rect_i.size,
                rect_softness: rect_i.softness,
            });

            indices.push((i * 4) as u16);
            indices.push((i * 4 + 2) as u16);
            indices.push((i * 4 + 1) as u16);

            indices.push((i * 4) as u16);
            indices.push((i * 4 + 3) as u16);
            indices.push((i * 4 + 2) as u16);
        }

        // vertices = vertices
        //     .iter()
        //     .map(|v| RectVertex {
        //         position: [
        //             (2.0 * v.position[0] / window_uniform.size[0]) - 1.0,
        //             1.0 - (2.0 * v.position[1] / window_uniform.size[1]),
        //         ],
        //         z_index: v.z_index,
        //         color: v.color,
        //         border_radius: v.border_radius,
        //         rect_pos: v.rect_pos,
        //         rect_size: v.rect_size,
        //     })
        //     .collect();

        println!("{:#?}", vertices);
        println!("{:#?}", window_uniform);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let window_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Window uniform"),
            contents: bytemuck::cast_slice(&[window_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let window_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("Window Bind Group Layout"),
            });

        let window_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &window_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: window_buffer.as_entire_binding(),
            }],
            label: Some("Window Bind Group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&window_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[RectVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        surface.configure(&device, &config);

        let mut font_system = FontSystem::new();
        let cache = SwashCache::new();
        let mut atlas = TextAtlas::new(&device, &queue, surface_format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

        buffer.set_size(&mut font_system, size.width as f32, size.height as f32);
        buffer.set_text(
            &mut font_system,
            "This is sample text",
            Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut font_system);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            num_vertices,
            index_buffer,
            num_indices,
            window_buffer,
            window_bind_group,
            font_system,
            cache,
            atlas,
            text_renderer,
            buffer,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self) -> bool {
        false
    }

    fn update(&mut self) {
        self.queue.write_buffer(
            &self.window_buffer,
            0,
            bytemuck::cast_slice(&[WindowUniform {
                size: [
                    self.window.inner_size().width as f32,
                    self.window.inner_size().height as f32,
                ],
                scale_factor: self.window.scale_factor() as f32,
                _padding: 0.0,
            }]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                Resolution {
                    width: self.size.width,
                    height: self.size.height,
                },
                [TextArea {
                    buffer: &self.buffer,
                    left: 10.0,
                    top: 10.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: 400,
                        bottom: 100,
                    },
                    default_color: Color::rgb(255, 255, 255),
                }],
                &mut self.cache,
            )
            .unwrap();
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.window_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            self.text_renderer
                .render(&self.atlas, &mut render_pass)
                .unwrap();
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.atlas.trim();

        Ok(())
    }
}

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(window).await;

    event_loop.run(move |event, _, control_flow| match event {
        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            state.update();
            match state.render() {
                Ok(_) => {}

                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(e) => eprintln!("{:?}", e),
            }
        }

        Event::MainEventsCleared => {
            state.window().request_redraw();
        }

        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

            WindowEvent::Resized(physical_size) => {
                state.resize(*physical_size);
            }

            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                state.resize(**new_inner_size);
            }
            _ => {}
        },
        _ => {}
    });
}
