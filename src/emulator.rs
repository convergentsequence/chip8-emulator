#![allow(arithmetic_overflow)]

use std::io::{Read, IntoInnerError};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::{thread, usize};
use std::fs::File;

use egui::mutex::{Mutex, MutexGuard};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::{Sdl, render::Canvas, video::Window};
use sdl2::render::{RenderTarget};

use crate::emulator_ui::InterThreadData;

const WINDOW_TITLE: &str = "CHIP-8";

struct GraphicsContext<T: RenderTarget>{
    sdl_ctx: Sdl,
    canvas: Canvas<T>,
}

#[allow(non_snake_case, dead_code)]
#[derive(Clone)]
pub struct C8 {
    pub memory: [u8; 4096],
    pub V: [u8; 16],
    pub I: u16,
    pub PC: u16,
    pub stack: [u16; 16],
    pub SP: usize,
    pub delay_timer: u8,
    pub sound_timer: u8,
}

impl Default for C8{
    fn default() -> Self {
        Self { memory: [0; 4096], V: [0; 16], I: 0, PC: 0x200, stack: [0; 16], SP: 1, delay_timer: 0, sound_timer: 0 }
    }
}
//#[allow(dead_code)]
struct UIInterface{
    kill_receiver: Receiver<bool>,
    target_file: String,
    egui_ctx: egui::Context,
    inter_thread: Arc<Mutex<InterThreadData>>,
}

/// interfaces the ui
impl UIInterface{
    fn new( kill_receiver: Receiver<bool>, 
            target_file: String, 
            egui_ctx: egui::Context,
            inter_thread: Arc<Mutex<InterThreadData>>) -> Self
    {
        Self { 
            kill_receiver, 
            target_file, 
            egui_ctx,
            inter_thread,
        }
    }
}

pub struct Emulator{
    ui_interface: UIInterface,
    context: GraphicsContext<Window>,
}


impl Emulator{
    fn init_context() -> GraphicsContext<Window> {
        let sdl_ctx = sdl2::init().unwrap();
        let video_subsystem = sdl_ctx.video().unwrap();

        let window = video_subsystem
            .window(WINDOW_TITLE, 640, 420)
            .position_centered()
            .build()
            .unwrap();
        
        let mut canvas = window.into_canvas().build().unwrap();
        canvas.set_logical_size(64, 32).unwrap();
        
        GraphicsContext{ sdl_ctx: sdl_ctx, canvas: canvas }
    }

    fn new(kill_receiver: Receiver<bool>, target_file: String, egui_ctx: egui::Context, inter_thread: Arc<Mutex<InterThreadData>>) -> Emulator {
        inter_thread.lock().executed_instructions.clear();
        inter_thread.lock().internal_state.clone_from(&C8::default());
        Emulator { 
            ui_interface: UIInterface::new(kill_receiver, target_file, egui_ctx, inter_thread),
            context: Emulator::init_context(),
        }
    }
    
    fn send_state(locked: &mut MutexGuard<InterThreadData>, opcode: String, internal_state: &C8) {
        locked.executed_instructions.push(opcode);
        if locked.executed_instructions.len() > 100 {
            locked.executed_instructions.remove(0);
        }
        locked.internal_state.clone_from(internal_state);
    }

    fn start(&mut self){
        let timer = self.context.sdl_ctx.timer().unwrap();
        let mut current_tick: u32;

        macro_rules! clocked {
            ($code:block, $last_tick:expr, $freq:expr) => {
                if current_tick - $last_tick >= 1000/$freq {
                    $code;
                    $last_tick = current_tick;
                }
            };
            ($code:expr, $last_tick:expr, $freq:expr) => {
                if current_tick - $last_tick >= 1000/$freq {
                    $code();
                    $last_tick = current_tick;
                }
            };
        }

        let mut event_pump = self.context.sdl_ctx.event_pump().unwrap();
        let mut internals = C8::default();

        {
            let mut file = File::open(self.ui_interface.target_file.clone()).unwrap();

            file.read(&mut internals.memory[0x200..]).unwrap();
        }

        let mut gbuf = [0u8; 64*32];

        gbuf[64+20] = 1;
        gbuf[69+420] = 1;
        
        let fontset: [u8; 80] = [
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80  // F
        ];
        internals.memory[0..80].clone_from_slice(&fontset);

        let mut last_opcode_tick = 0u32;
        let mut last_render_tick = 0u32;
        let mut frozen = false;

        'running: loop {
            if let Ok(_) = self.ui_interface.kill_receiver.try_recv() {
                break 'running;
            }

            for event in event_pump.poll_iter() {
                if let Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape | Keycode::Q), .. } = event {
                    break 'running;
                }
            }
            current_tick = timer.ticks();
            
            let mut execute_opcodes = ||{
                let opcode: u16 = (internals.memory[internals.PC as usize] as u16) << 8 | internals.memory[(internals.PC + 1) as usize] as u16;
               
                let old_pc = internals.PC;
                internals.PC += 2;

                let mut opcode_description: String = "Unknown/unimplemented instruction".to_owned();

                match opcode >> 12 {
                    0 => {
                        match opcode & 0xFF {
                            0xE0 => { // 0x00E0 - clear the screen
                                opcode_description = "Clearing screen".to_owned();
                                gbuf.clone_from(&[0; 64*32]);
                            },
                            0xEE => { // 0x00EE - return from subroutine call
                                opcode_description = format!("Reuturning from subroutine to: 0x{:03X}", internals.stack[internals.SP - 1]);
                                internals.SP -= 1;
                                internals.PC = internals.stack[internals.SP];
                            },
                            _ => {}
                        }
                    },
                    1 => { // 0x1NNN - jump to location NNN
                        let nnn = opcode & 0xFFF;
                        opcode_description = format!("Jumping to location 0x{:03X}", nnn);
                        internals.PC = opcode & nnn;
                    },
                    2 => { // 0x2NNN - jump to subroutine at address NNN
                        let nnn = opcode & 0xFFF;
                        internals.stack[internals.SP] = internals.PC;
                        internals.SP += 1;
                        internals.PC = nnn;
                    },
                    3 => { // 0x3XRR - skip next instruction if V[X] == 0xRR 
                        let x = (opcode & 0xF00) >> 8;
                        let rr = (opcode & 0xFF) as u8;
                        opcode_description = format!("Skipping next instruction if V{:X}(0x{:02X}) == 0x{:02X}",x,internals.V[x as usize], rr);
                        if internals.V[x as usize] == rr {
                            internals.PC += 2;
                        }
                    },
                    4 => { // 0x4XRR - skip next intruction if V[X] != 0xRR
                        let x = (opcode & 0xF00) >> 8;
                        let rr = (opcode & 0xFF) as u8;
                        opcode_description = format!("Skipping next instruction if V{:X}(0x{:02X}) != 0x{:02X}",x,internals.V[x as usize], rr);
                        if internals.V[x as usize] != rr {
                            internals.PC += 2;
                        }
                    },
                    5 => { // 0x5XY0 - skip next instruction if V[X] == V[Y]
                        let x = ((opcode & 0xF00) >> 8) as usize;
                        let y = ((opcode & 0xF0) >> 4) as usize;
                        opcode_description = format!("Skipping next instruction if V{:X}(0x{:02X}) == V{:X}(0x{:02X})", x, internals.V[x], y, internals.V[y]);
                        if internals.V[x] == internals.V[y] {
                            internals.PC += 2;
                        }
                    },
                    6 => { // 0x6XRR - move constant RR into V[X]
                        let x = ((opcode & 0xF00) >> 8) as usize;
                        let rr = (opcode & 0xFF) as u8;
                        opcode_description = format!("Moving 0x{:02X} into V{:X}", rr, x);
                        internals.V[x] = rr;
                    },
                    7 => { // 0x7XRR - add RR to value of V[X]
                        let x = ((opcode & 0xF00) >> 8) as usize;
                        let rr = (opcode & 0xFF) as u8;
                        opcode_description = format!("Adding 0x{:02X} to V{:X}", rr, x);
                        internals.V[x] += rr;
                    },
                    8 => {
                        match opcode & 0xF {
                            0 => { // 0x8XY0 - move register VY to register VX
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Moving V{:X} into V{:X}", y, x);
                                internals.V[x] = internals.V[y];
                            }
                            1 => { // 0x8XY1 - stores the value of VX | VY into VX
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Adding V{:X}to V{:X} OR V{:X})",x,x,y);
                                internals.V[x] |= internals.V[y];
                            },
                            2 => { // 0x8XY2 - add value of VY to VX
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Adding V{:X} to V{:X}", y, x);
                                internals.V[x] += internals.V[y];
                            },
                            3 => { // 0x8XY3 - XOR VY and X store in VX
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Set V{:X} to V{:X} XOR V{:X}", x, x, y);
                                internals.V[x] ^= internals.V[y];
                            },
                            4 => { // 0x8XY4 - Add VY to VX store carry in V15
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Add V{:X} to V{:X} and store carry in VF", y, x);
                                internals.V[0xF] = if internals.V[x] as i32 + internals.V[y] as i32 > 255 {1} else {0};
                                internals.V[x] += internals.V[y];
                            },
                            5 => { // 0x8XY5 - Subtract VY from VX and store the borrow in V15
                                let x = ((opcode & 0xF00) >> 8) as usize;
                                let y = ((opcode & 0xF0) >> 4) as usize;
                                opcode_description = format!("Subtract V{:X} from V{:X} and store the borrow in VF" ,y ,x);
                                internals.V[0xF] = if internals.V[x] >= internals.V[y] {1} else {0};
                                internals.V[x] -= internals.V[y];
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }

                {
                    let locked = &mut self.ui_interface.inter_thread.lock();
                    frozen = locked.freeze; // needs to be written to an external variable so timer updates can also be frozen
                                            // without needing to use locks,
                    if frozen {
                        return;
                    }
                    Emulator::send_state(locked, format!("{:04X}: {:04X} - {}", old_pc, opcode, opcode_description), &internals);
                }

            };
            clocked!(execute_opcodes, last_opcode_tick, 500);
            
            let mut execute_render = || {
                if !frozen{
                    internals.delay_timer -= if internals.delay_timer > 0 {1} else {0};
                    internals.sound_timer -= if internals.sound_timer > 0 {1} else {0};
                }
                self.render_graphics(&gbuf);
                self.ui_interface.egui_ctx.request_repaint();
            };
            clocked!(execute_render, last_render_tick, 60);
        }
    }

    fn render_graphics(&mut self, gbuf: &[u8; 64*32]){
        let canvas = &mut self.context.canvas;
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        for i in 0..64usize{
            for j in 0..32usize{
                let pixel: u8 = gbuf[i+j*64] * 255;
                canvas.set_draw_color(Color::RGB(pixel, pixel, pixel));
                canvas.draw_point(Point::new(i as i32, j as i32)).unwrap();
            }
        }
        canvas.present();
    }

}


pub fn start_thread(kill_receiver: Receiver<bool>, egui_ctx: egui::Context, inter_thread: Arc<Mutex<InterThreadData>>) -> thread::JoinHandle<()>{
    thread::spawn(move || {
        let mut emulator = Emulator::new(kill_receiver, (r"C:\C8Games\Connect_4.ch8").to_owned(), egui_ctx, inter_thread);
        emulator.start();
    })
}