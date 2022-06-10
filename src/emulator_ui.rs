use egui::Ui;

struct WindowStates{
    control_panel: bool
}

impl Default for WindowStates{
    fn default() -> Self {
        Self { control_panel: true }
    }
}

pub struct EmulatorUI{
    window_states: WindowStates,
}

impl EmulatorUI{
    #[inline]
    fn create_window_toggle(ui: &mut Ui, window_state: &mut bool, name: &str){
        if ui.button(if *window_state {format!("[*] {}", name)} else {format!("[_] {}", name)}).clicked() {
            *window_state = !*window_state;
        }
    }
}

impl Default for EmulatorUI{
    fn default() -> Self {
        Self { 
            window_states: WindowStates::default(),
        }
    }
}

impl eframe::App for EmulatorUI{
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
     
        ctx.set_visuals(egui::Visuals::dark());   // dark theme

        // <background and menu bar>
        egui::CentralPanel::default()
            .show(ctx, |ui|{
                ui.horizontal(|ui|{
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.control_panel, "Control Panel");
                });
            });
        // </background and menu bar>

        // <control panel>
        egui::Window::new("Control panel")
            .resizable(true)
            .open(&mut self.window_states.control_panel)
            .default_pos(egui::pos2(10f32, 40f32))
            .show(ctx, |ui|{

                ui.allocate_space(egui::vec2(0f32, 5f32)); // padding

                

                ui.allocate_space(egui::vec2(60f32, 10f32)); // padding
            }); 
        // </control panel>
    }
}