mod emulator_ui;
use emulator_ui::EmulatorUI;

mod emulator;

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.initial_window_size = Some(egui::vec2(1024f32, 720f32));

    eframe::run_native(
        "CHIP-8 Emulator", 
        options, 
        Box::new(
            |_cc| Box::new(EmulatorUI::default())
        ));
}
