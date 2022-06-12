use egui::{Ui, style};
use egui::mutex::Mutex;
use core::panic;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender};
use std::thread::JoinHandle;

use crate::emulator;

/// Holds open/closed states of all ui windows
struct WindowStates {
    control_panel: bool,
    opcodes_view: bool,
    internals: bool,
    memory: bool,
    keybinds: bool,
}

impl Default for WindowStates {
    fn default() -> Self {
        Self { control_panel: true, opcodes_view: false, internals: false, memory: false, keybinds: false }
    }
}

struct UIStates {
    memory_slider: i32,
}

impl Default for UIStates{
    fn default() -> Self {
        Self { memory_slider: 3840, }
    }
}

/// Data that both threads have access to, used for the emulator to communicate
/// its current state to the ui thread.
pub struct InterThreadData{
    pub executed_instructions: Vec<String>,
    pub internal_state: emulator::C8,
    pub freeze: bool,
}

impl InterThreadData{
    fn new() -> Self{
        Self{
            executed_instructions: vec![],
            internal_state: emulator::C8::default(),
            freeze: false,
        }
    }
}

/// Controls and communicates with the emulator thread
struct EmulatorInterface {
    /// Sender used to close the emulator externally, when any value is sent the emulator closes
    kill_sender: Option<Sender<bool>>, 
    /// Handle to emulator thread
    emulator_handle: Option<JoinHandle<()>>,
    // Used by emulator to communicate its current state
    inter_thread: Arc<Mutex<InterThreadData>>,
}

impl EmulatorInterface{
    /// Returns true if the emulator is currently running
    fn status(&self) -> bool {
        match &self.emulator_handle {
            Some(handle) => !handle.is_finished(),
            None => false
        }
    }

    fn join_thread(&mut self) {
        let handle = std::mem::replace(&mut self.emulator_handle, None).unwrap();
        handle.join().unwrap();
    }

    fn start(&mut self, egui_ctx: &egui::Context) {
        if let Some(_) = self.emulator_handle{
            if self.status() {
                panic!("Attempted to start emulator while already running");
            }else{
                self.join_thread();
            }
        }

        let kill_channel = channel();
        self.kill_sender = Some(kill_channel.0);
        self.emulator_handle = Some(emulator::start_thread(kill_channel.1, egui_ctx.clone(), self.inter_thread.clone()));
    }
    
    fn kill(&mut self){
        if let None = self.emulator_handle{
            panic!("Attempted to kill emulator while it is not running");
        }

        self.kill_sender.as_ref().unwrap().send(true).unwrap();

        self.join_thread();
    }
}

impl Default for EmulatorInterface {
    fn default() -> Self {
        Self{
            kill_sender: None,
            emulator_handle: None,
            inter_thread: Arc::new(Mutex::new(InterThreadData::new())),
        }
    }
} 

/// Renders the actual ui
pub struct EmulatorUI {
    window_states: WindowStates,
    ui_states: UIStates,
    emulator_interface: EmulatorInterface
}

impl EmulatorUI {
    /// Draws a button that controls open/closed state of a window that the window_state belongs to
    #[inline]
    fn create_window_toggle(ui: &mut Ui, window_state: &mut bool, name: &str) {
        if ui.button(if *window_state {format!("[*] {}", name)} else {format!("[_] {}", name)}).clicked() {
            *window_state = !*window_state;
        }
    }
}

impl Default for EmulatorUI {
    fn default() -> Self {
        Self { 
            window_states: WindowStates::default(),
            ui_states: UIStates::default(),
            emulator_interface: EmulatorInterface::default(),
        }
    }
}

impl eframe::App for EmulatorUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
     
        ctx.set_visuals(egui::Visuals::dark());   // dark theme
        
        // <background and menu bar>
        egui::CentralPanel::default()
            .show(ctx, |ui|{
                ui.horizontal(|ui| {
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.control_panel, "Control Panel");
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.opcodes_view, "Instructions");
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.internals, "Internals");
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.memory, "Memory");
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.keybinds, "Keybinds");
                });
            });
        // </background and menu bar>

        // <control panel>
        egui::Window::new("Control Panel")
            .resizable(true)
            .open(&mut self.window_states.control_panel)
            .default_pos(egui::pos2(10f32, 40f32))
            .default_size([250.0, 150.0])
            .show(ctx, |ui| {

                ui.allocate_space(egui::vec2(0f32, 5f32)); // padding

                let should_start = !self.emulator_interface.status();

                // <emulator status>
                ui.horizontal(|ui| {
                    ui.label("Status: ");
                    if should_start {
                        ui.colored_label(egui::Color32::LIGHT_RED, "Inactive");
                    }else{
                        if self.emulator_interface.inter_thread.lock().freeze {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, "Frozen");
                        }else{
                            ui.colored_label(egui::Color32::LIGHT_GREEN, "Running");
                        }
                    }
                });
                // </emulator status>

                ui.allocate_space(egui::vec2(0f32, 5f32)); // padding

                // <start stop button>
                if ui.button(if should_start {"Start Emulator"} else {"Stop Emulator"}).clicked() {
                    if should_start{
                        self.emulator_interface.start(&ctx);
                    }else{
                        self.emulator_interface.kill();
                    }
                }
                // </start stop button>
                ui.allocate_space(egui::vec2(0f32, 5f32)); // padding
                ui.checkbox(&mut self.emulator_interface.inter_thread.lock().freeze, "Freeze");

                ui.allocate_space(egui::vec2(60f32, 10f32)); // padding
                ui.allocate_space(ui.available_size());
            }); 
        // </control panel>


        // <opcodes view>
        egui::Window::new("Instructions")
            .open(&mut self.window_states.opcodes_view)
            .default_pos(egui::pos2(50f32, 40f32))
            .default_size([500.0, 500.0])
            .resizable(false)
            .show(ctx, |ui| { 
                // <executed opcodes list>
                egui::containers::ScrollArea::new([true, true])
                .max_height(500f32)
                .show(ui, |ui|{
                    egui::Grid::new("Opcodes_Grid")
                    .num_columns(1)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        let mut locked = self.emulator_interface.inter_thread.lock();
                        let vec = &mut locked.executed_instructions;
                        for oc in vec.iter().rev() {
                            ui.horizontal(|ui| {
                                ui.label(oc);
                                ui.allocate_space(egui::Vec2::new(ui.available_width(), 0f32));
                            });
                            ui.end_row();
                        }
                        
                    });

                    ui.allocate_space(ui.available_size()); // allocate space when the list is empty
                });
                // <executed opcodes list>
            });
        // </opcodes view>

        
        // <internals>
        egui::Window::new("Internals")
            .open(&mut self.window_states.internals)
            .resizable(false)
            .default_size([300.0, 500.0])
            .show(ctx, |ui|{
                let locked = self.emulator_interface.inter_thread.lock();
                let internals = &locked.internal_state;
                
                let mut internals_color = egui::Color32::LIGHT_RED;
                if self.emulator_interface.status() {
                    if locked.freeze {
                        internals_color = egui::Color32::LIGHT_BLUE;
                    }else{
                        internals_color = egui::Color32::LIGHT_GREEN;
                    }
                }

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui|{
                            ui.colored_label(internals_color, "PC: ");
                            ui.label(format!("0x{:04X}", internals.PC));
                        });
        
                        ui.horizontal(|ui| {
                            ui.colored_label(internals_color, "I: ");
                            ui.label(format!("0x{:04X}", internals.I));
                        });
        
                        egui::Grid::new("V_Grid")
                            .num_columns(1)
                            .spacing([0.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for (i, v) in internals.V.iter().enumerate() {
                                    ui.colored_label(internals_color, format!("V{:X}: ", i));
                                    ui.label(format!("0x{:02X}", v));
                                    ui.end_row();
                                }
                            });
                    });
                    ui.allocate_space(egui::Vec2::new(10f32, 0f32));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(internals_color, "SP: ");
                            ui.label(format!("0x{:04X}", internals.SP));
                        });

                        egui::Grid::new("Stack_Grid")
                            .num_columns(1)
                            .spacing([0.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for (i, v) in internals.stack.iter().enumerate() {
                                    ui.colored_label(internals_color, format!("+0x{:X}: ", i));
                                    ui.label(format!("0x{:02X}", v));
                                    ui.end_row();
                                }
                            });

                        ui.allocate_space(egui::Vec2::new(0f32, ui.available_height()));
                    });

                    ui.vertical(|ui|{
                        ui.horizontal(|ui|{
                            ui.colored_label(internals_color, "Delay timer: ");
                            ui.label(format!("{}", internals.delay_timer));
                        });
                        ui.horizontal(|ui|{
                            ui.colored_label(internals_color, "Sound timer: ");
                            ui.label(format!("{}", internals.sound_timer));
                        });
                    });
                });

            });
        // </internals>

        // <memory>
        let memory_window = egui::Window::new("Memory")
            .open(&mut self.window_states.memory)
            .default_pos(egui::pos2(50f32, 40f32))
            .default_size([500.0, 500.0])
            .resizable(false)
            .show(ctx, |ui| {
                let locked = self.emulator_interface.inter_thread.lock();
                let internals = &locked.internal_state;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        egui::Grid::new("Memory_Grid")
                            .num_columns(1)
                            //.spacing([40.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                let start_point = 3840 - self.ui_states.memory_slider;
                                let mut line: String = "".to_owned();
                                let mem_area = &internals.memory[start_point as usize..(start_point + 16*16) as usize];
                                for (i, byte) in mem_area.iter().enumerate() {
                                    if i % 16 == 0 && i != 0{
                                        ui.monospace(&mut line);
                                        ui.end_row();
                                        line.clear();
                                    }
                                    line.push_str(&format!(" {:02X}", byte));
                                }
                            });
                    });

                    ui.vertical(|ui| {
                        let mut style = ctx.style().as_ref().clone();
                        style.spacing.slider_width = 330f32;
                        ctx.set_style(style);

                        ui.add_sized(
                            ui.available_size(),
                            egui::Slider::new(&mut self.ui_states.memory_slider, 0..=3840 )
                            .vertical()
                            .show_value(false)
                            .step_by(16f64),
                        );
                        ui.allocate_space(egui::Vec2::new(0.0, ui.available_height()));
                    });
                    
                });

                //ui.allocate_space(ui.available_size());
            });
            match memory_window {
                Some(window) => {
                    if window.response.hovered() {
                        let events = &ctx.input().events;
                        for event in events.iter() {
                            if let egui::Event::Scroll(scroll) = event {
                                let direction = (scroll[1] / scroll[1].abs()) as i32; 
                                self.ui_states.memory_slider += direction * 16;
                                self.ui_states.memory_slider = self.ui_states.memory_slider.clamp(0, 3840);
                            }
                        }
                    }
                },
                None => {},
            }
  
        // </memory>
    }
}