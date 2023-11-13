# QML

QT module for writing guis in QML (qt markup language)
C++ library by default

three rust crates for bindings

- qml [archived]
https://crates.io/crates/qml
All-Time: 7,844
Updated: almost 7 years ago

- qmlrs
https://crates.io/crates/qmlrs
All-Time: 4,645
Updated: over 7 years ago
8 contributors/432 stars

- qmetaobject
https://crates.io/crates/qmetaobject
All-Time: 34,945
Updated: about 1 hour ago

relies heavily on macro use
requires a C++ build step
contains lots of unsafe code


# EGUI

Pure rust framework
280 dependencies
Complex setups for different platforms

https://github.com/emilk/egui
All-Time: 1,472,058
320 contributors/17k stars


## What's good about egui (the good)

egui is not a framework, but a library. Therefore it does not influence the
structure of the app at all. The control flow stays within the app at all
times. Integration of egui requires three parts:

- **ui context object and `run` function**: The actual UI code. Uses mutable
local variables that are available inside the event-loop where the ui is "run"
to produce a list of texture operations and a list of shapes to be rendered.

```rust
// Right before drawing, every tick.
let full_output = egui_context.run(raw_input, |ctx| {
    egui::CentralPanel::default()
        // Transparent background so that we can render the UI on top of the app canvas.
        .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
        .show(&ctx, |ui| {
            ui.label("Hello egui!");
        });
});
```

- **window framework integration (egui-winit)**: Set of functions and a `State`
struct for linking UI interactions to window iteractions. Events should be
forwarded to these, which update the internal state, and respond whether the
event was consumed by egui or whether it should be forwarded to the game/app
logic. Once a tick we `take` the accumulated state and pass it to egui for
rendering.

```rust
// Inside the event loop, for every event.
match event {
    Event::WindowEvent { event, .. } => {
        // First try to handle the event using egui.
        let event_response = winit_state.on_event(&egui_context, &event);

        if event_response.repaint {
            window.request_redraw();
        }

        if event_response.consumed {
            return;
        }

        // ... Handle event in app logic.
    }
    // ... Handle other events.
}
```

- **wgpu integration (egui-wgpu, renderer/low-level interface)**: Set of
functions wrapping wgpu procedures for updating graphics resources and actually
rendering the primitives that egui produces.

```rust
// In the render procedure, every tick.
let egui_paint_jobs = ui.context.tessellate(full_output.shapes);
let texture_sets = full_output.textures_delta.set;

// Update textures.
for (id, image_delta) in texture_sets {
    self.egui_renderer
        .update_texture(&self.device, &self.queue, id, &image_delta);
}

// Write the primitives to uniforms/buffers.
let mut command_buffers = self.egui_renderer.update_buffers(
    &self.device,
    &self.queue,
    &mut encoder,
    &egui_paint_jobs,
    &screen_descriptor,
);

// ... Render the app content.

// After rendering the app.
self.egui_renderer.render(&mut render_pass, &egui_paint_jobs, &screen_descriptor);
```

## What's difficult about egui (the bad)

As the documentation states - egui is


## What's bad about egui (the ugly)
