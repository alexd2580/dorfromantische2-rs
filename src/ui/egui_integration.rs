use winit::event::WindowEvent;
use winit::window::Window;

pub struct EguiIntegration {
    context: egui::Context,
    state: egui_winit::State,
}

impl EguiIntegration {
    pub fn new(window: &Window) -> Self {
        Self {
            context: egui::Context::default(),
            state: egui_winit::State::new(window),
        }
    }

    pub fn on_event(&mut self, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_event(&self.context, event)
    }

    pub fn run(
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
