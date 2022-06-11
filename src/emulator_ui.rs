use egui::Ui;
use egui::mutex::Mutex;
use core::panic;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;

use crate::emulator;

/// Holds open/closed states of all ui windows
struct WindowStates {
    control_panel: bool
}

impl Default for WindowStates {
    fn default() -> Self {
        Self { control_panel: true }
    }
}

/// Controls and communicates with the emulator thread
struct EmulatorInterface {
    /// Sender used to close the emulator externally, when any value is sent the emulator closes
    kill_sender: Option<Sender<bool>>, 
    /// Passed to emulator thread to write all executed opcodes to
    executed_opcodes: Arc<Mutex<Vec<String>>>,
    /// Handle to emulator thread
    emulator_handle: Option<JoinHandle<()>>,
}   

impl EmulatorInterface{
    /// Returns true if the emulator is currently running
    fn status(&mut self) -> bool {
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
        self.emulator_handle = Some(emulator::start_thread(kill_channel.1, self.executed_opcodes.clone(), egui_ctx.clone()));
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
            executed_opcodes: Arc::new(Mutex::new(vec![])),
            emulator_handle: None,
        }
    }
} 

/// Renders the actual ui
pub struct EmulatorUI {
    window_states: WindowStates,
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
            emulator_interface: EmulatorInterface::default(),
        }
    }
}

impl eframe::App for EmulatorUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
     
        ctx.set_visuals(egui::Visuals::dark());   // dark theme
        {
            let vec_mutex = self.emulator_interface.executed_opcodes.lock();
            let vec = vec_mutex.deref();
            if let Some(oc) = vec.last() {
                println!("{}",oc);
            }
           
        }
        // <background and menu bar>
        egui::CentralPanel::default()
            .show(ctx, |ui|{
                ui.horizontal(|ui| {
                    EmulatorUI::create_window_toggle(ui, &mut self.window_states.control_panel, "Control Panel");
                });
            });
        // </background and menu bar>

        // <control panel>
        egui::Window::new("Control Panel")
            .resizable(true)
            .open(&mut self.window_states.control_panel)
            .default_pos(egui::pos2(10f32, 40f32))
            .show(ctx, |ui| {

                ui.allocate_space(egui::vec2(0f32, 5f32)); // padding

                // <start stop button>
                let should_start = !self.emulator_interface.status();
                if ui.button(if should_start {"Start Emulator"} else {"Stop Emulator"}).clicked() {
                    if should_start{
                        self.emulator_interface.start(&ctx);
                    }else{
                        self.emulator_interface.kill();
                    }
                }
                // </start stop button>

                ui.allocate_space(egui::vec2(60f32, 10f32)); // padding
            }); 
        // </control panel>
    }
}