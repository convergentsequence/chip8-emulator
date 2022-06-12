use std::io::Read;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::fs::File;

use egui::mutex::Mutex;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::{Sdl, render::Canvas, video::Window};
use sdl2::render::{RenderTarget};

const WINDOW_TITLE: &str = "CHIP-8";

struct GraphicsContext<T: RenderTarget>{
    sdl_ctx: Sdl,
    canvas: Canvas<T>,
}

#[allow(non_snake_case, dead_code)]
pub struct C8 {
    memory: [u8; 4096],
    V: [u8; 16],
    I: u16,
    PC: u16,
    stack: [u16; 16],
    SP: u8,
}

impl Default for C8{
    fn default() -> Self {
        Self { memory: [0; 4096], V: [0; 16], I: 0, PC: 0x200, stack: [0; 16], SP: 0 }
    }
}
//#[allow(dead_code)]
struct UIInterface{
    kill_receiver: Receiver<bool>,
    target_file: String,
    opcodes_vec: Arc<Mutex<Vec<String>>>,
    egui_ctx: egui::Context,
}

/// interfaces the ui
impl UIInterface{
    fn new( kill_receiver: Receiver<bool>, 
            target_file: String, 
            opcodes_vec: Arc<Mutex<Vec<String>>>, 
            egui_ctx: egui::Context) -> Self
    {
        Self { 
            kill_receiver, 
            target_file, 
            opcodes_vec,
            egui_ctx,
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

    fn new(kill_receiver: Receiver<bool>, target_file: String, opcode_vec: Arc<Mutex<Vec<String>>>, egui_ctx: egui::Context) -> Emulator {
        Emulator { 
            ui_interface: UIInterface::new(kill_receiver, target_file, opcode_vec, egui_ctx),
            context: Emulator::init_context(),
        }
    }

    fn send_opcode(&mut self, value: String) {
        let mut locked = self.ui_interface.opcodes_vec.lock();
        let vec = locked.deref_mut();
        vec.push(value);
        if vec.len() > 100 {
            vec.remove(0);
        }
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
        
        let mut last_opcode_tick = 0u32;
        let mut last_render_tick = 0u32;
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

            
            clocked!({
                let opcode: u16 = (internals.memory[internals.PC as usize] as u16) << 8 | internals.memory[(internals.PC + 1) as usize] as u16;
                
                if opcode != 0 {
                    self.send_opcode(format!("{:04X}: {:04X}",internals.PC, opcode));
                    internals.PC += 2;
                }else{
                    internals.PC = 0x200;
                }
            }, last_opcode_tick, 1000);
            
         
            clocked!({
                self.render_graphics(&gbuf);
                self.ui_interface.egui_ctx.request_repaint();
            }, last_render_tick, 60);
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


pub fn start_thread(kill_receiver: Receiver<bool>, opcode_vec: Arc<Mutex<Vec<String>>>, egui_ctx: egui::Context) -> thread::JoinHandle<()>{
    thread::spawn(move || {
        let mut emulator = Emulator::new(kill_receiver, (r"C:\C8Games\Connect_4.ch8").to_owned(), opcode_vec, egui_ctx);
        emulator.start();
    })
}