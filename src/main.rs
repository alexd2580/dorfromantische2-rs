use app::App;
use glam::{UVec2, Vec2};
use render::gpu::Gpu;
use render::pipeline::Pipeline;
use std::{env, path::PathBuf};
use ui::egui_integration::EguiIntegration;
use ui::render_ui::render_ui;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod app;
mod best_placements;
mod data;
mod file_watcher;
mod game;
mod game_data;
mod group;
mod group_assignments;
mod hex;
mod map;
mod raw_data;
mod render;
mod tile_frequency;
mod ui;

#[allow(for_loops_over_fallibles)]
fn run(
    event_loop: EventLoop<()>,
    window: Window,
    mut gpu: Gpu,
    mut pipeline: Pipeline,
    mut ui: EguiIntegration,
    mut app: App,
) {
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
                                app.input.grab_move = true;
                            }
                            (MouseButton::Left, ElementState::Released) => {
                                app.input.grab_move = false;
                            }
                            (MouseButton::Right, ElementState::Pressed) => {
                                app.input.grab_rotate = true;
                            }
                            (MouseButton::Right, ElementState::Released) => {
                                app.input.grab_rotate = false;
                            }
                            _ => {}
                        }

                        // Lock the mouse so that we can't leave the window while dragging and
                        // enter a crooked button state.
                        let grab_mode = if !app.input.grab_move && !app.input.grab_rotate {
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
                    } => app.camera.on_scroll(y),
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(size) => {
                        // Window has been resized. Adjust render pipeline settings.
                        gpu.resize(size.width, size.height);
                        pipeline.resize(size.width, size.height);
                        app.camera.resize(UVec2::new(size.width, size.height));

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
                    app.visible_rect = render_ui(
                        &mut app.data,
                        &mut app.camera,
                        &mut app.ui_state,
                        &mut app.file_watcher,
                        &app.input,
                        &app.game_nav,
                        &mut app.pending_zoom_fit,
                        ctx,
                    );
                });

                app.tick(&gpu);
                let bind_groups = app.bind_groups.groups.as_ref().map(<[_; 1]>::as_slice);
                pipeline.redraw(
                    &gpu,
                    bind_groups,
                    &paint_jobs,
                    textures_delta,
                    app.visible_rect,
                );
            }
            _ => {}
        }
    });
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("info,wgpu_core=error,wgpu_hal=error,egui_wgpu=error,naga=error"),
    )
    .init();

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let gpu = pollster::block_on(Gpu::new(&window));
    let mut app = App::new(&window, &gpu);
    let pipeline = Pipeline::new(&gpu, &window, &app.bind_groups.layouts);
    let ui = EguiIntegration::new(&window);

    // Load the specified or previous file.
    let arguments = env::args().collect::<Vec<_>>();
    if arguments.len() > 1 {
        let file = PathBuf::from(&arguments[1]);
        app.file_watcher.set_file_path(&file);
    } else {
        app.file_watcher.use_previous_file_path();
    }

    run(event_loop, window, gpu, pipeline, ui, app);
    dbg!("Exiting");
}
