use app::App;
use glam::{UVec2, Vec2};
use gpu::Gpu;
use pipeline::Pipeline;
use render_ui::render_ui;
use std::{env, path::PathBuf};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod app;
mod best_placements;
mod bind_groups;
mod data;
mod gpu;
mod group;
mod group_assignments;
mod index;
mod lerp;
mod map;
mod pipeline;
mod raw_data;
mod render_ui;
mod shader;
mod textures;

mod opencv;
mod xlib;

struct Ui {
    context: egui::Context,
    state: egui_winit::State,
}

impl Ui {
    fn new(window: &Window) -> Self {
        Self {
            context: egui::Context::default(),
            state: egui_winit::State::new(window),
        }
    }

    fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_event(&self.context, event)
    }

    fn run(
        &mut self,
        window: &Window,
        run_ui: impl FnOnce(&egui::Context),
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta) {
        let egui::FullOutput {
            shapes,
            textures_delta,
            ..
        } = self.context.run(self.state.take_egui_input(window), run_ui);
        (self.context.tessellate(shapes), textures_delta)
    }
}

#[allow(for_loops_over_fallibles)]
fn run(
    event_loop: EventLoop<()>,
    window: Window,
    mut gpu: Gpu,
    mut pipeline: Pipeline,
    mut ui: Ui,
    mut app: App,
) {
    let mut show_tooltip = false;
    let mut sidebar_expanded = true;
    event_loop.run(move |event, _, control_flow| {
        // What the actual??
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        // let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => {
                let event_response = ui.on_event(&event);

                if event_response.repaint {
                    // window.request_redraw();
                }

                if event_response.consumed {
                    return;
                }

                match event {
                    // WindowEvent::CursorMoved { position, .. } => {}
                    WindowEvent::MouseInput { button, state, .. } => {
                        match (button, state) {
                            (MouseButton::Left, ElementState::Pressed) => {
                                app.grab_move = true;
                            }
                            (MouseButton::Left, ElementState::Released) => {
                                app.grab_move = false;
                            }
                            (MouseButton::Right, ElementState::Pressed) => {
                                app.grab_rotate = true;
                            }
                            (MouseButton::Right, ElementState::Released) => {
                                app.grab_rotate = false;
                            }
                            _ => {}
                        }

                        // Lock the mouse so that we can't leave the window while dragging and
                        // enter a crooked button state.
                        let grab_mode = if !app.grab_move && !app.grab_rotate {
                            winit::window::CursorGrabMode::None
                        } else {
                            winit::window::CursorGrabMode::Confined
                        };
                        window.set_cursor_grab(grab_mode).unwrap();
                    }
                    WindowEvent::CursorMoved {
                        position: PhysicalPosition { x, y },
                        ..
                    } => app.on_cursor_move(Vec2::new(x as f32, y as f32)),
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    } => app.on_scroll(y),
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        // Window has been resized. Adjust render pipeline settings.
                        gpu.resize(size.width, size.height);
                        pipeline.resize(size.width, size.height);
                        app.resize(UVec2::new(size.width, size.height));

                        // On macos the window needs to be redrawn manually after resizing
                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let (paint_jobs, textures_delta) = ui.run(&window, |ctx| {
                    // TODO move these bools somewhere.... TODO what bools?
                    render_ui(&mut app, ctx, &mut sidebar_expanded, &mut show_tooltip);
                });

                app.tick(&gpu);
                let bind_groups = app.bind_groups.groups.as_ref().map(<[_; 1]>::as_slice);
                pipeline.redraw(&gpu, bind_groups, &paint_jobs, textures_delta);
            }
            _ => {}
        }
    });
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let gpu = pollster::block_on(Gpu::new(&window));
    let mut app = App::new(&window, &gpu);
    let pipeline = Pipeline::new(&gpu, &window, &app.bind_groups.layouts);
    let ui = Ui::new(&window);

    // Load the specified or previous file.
    let arguments = env::args().collect::<Vec<_>>();
    if arguments.len() > 1 {
        let file = PathBuf::from(&arguments[1]);
        app.set_file_path(&file);
    } else {
        app.use_previous_file_path();
    }

    run(event_loop, window, gpu, pipeline, ui, app);
    dbg!("Exiting");
}
