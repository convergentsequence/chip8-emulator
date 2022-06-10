#![allow(unused_variables, dead_code, unused_imports)]

use std::io::Read;
use std::sync::mpsc::Receiver;
use std::thread;
use std::fs::File;

use egui::Memory;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Point;
use sdl2::{Sdl, VideoSubsystem, sys::SDL_Window, render::Canvas, video::Window};
use sdl2::render::{self, RenderTarget};

const WINDOW_TITLE: &str = "CHIP-8";

struct EmulatorContext<T: RenderTarget>{
    sdl_ctx: Sdl,
    canvas: Canvas<T>,
}

#[allow(non_snake_case)]
struct C8 {
    memory: [u8; 4096],
    V: [u8; 16],
    I: u16,
    PC: u16,
    stack: [u16; 16],
    SP: u8,
}

impl Default for C8{
    fn default() -> Self {
        Self { memory: [0; 4096], V: [0; 16], I: 0, PC: 0, stack: [0; 16], SP: 0 }
    }
}

pub struct Emulator{
    kill_receiver: Receiver<bool>,
    target_file: String,
    context: EmulatorContext<Window>,
}

impl Emulator{
    fn init_context() -> EmulatorContext<Window> {
        let sdl_ctx = sdl2::init().unwrap();
        let video_subsystem = sdl_ctx.video().unwrap();

        let window = video_subsystem
            .window(WINDOW_TITLE, 640, 420)
            .position_centered()
            .build()
            .unwrap();
        
        let mut canvas = window.into_canvas().build().unwrap();
        canvas.set_logical_size(64, 32).unwrap();
        
        let gbuf: Box<[bool; 64*32]> = Box::new([false; 64*32]);
        
        EmulatorContext{ sdl_ctx: sdl_ctx, canvas: canvas }
    }

    fn new(kill_receiver: Receiver<bool>, target_file: String) -> Emulator {
        Emulator { 
            kill_receiver: kill_receiver, 
            target_file: target_file,
            context: Emulator::init_context(),
        }
    }

    /// Execute \<func> with parameters \<params> \<freq> times a second within an infinite loop
    fn clocked_execution<F, T>(func: F, ctx: &EmulatorContext<Window>, freq: u32, last_tick: &mut u32, params: T) 
    where F: Fn(T) -> () {
        let sdl = &ctx.sdl_ctx;
        let current_tick = sdl.timer().unwrap().ticks();

        if current_tick - *last_tick >= 1000/freq {
            func(params);
            *last_tick = current_tick;
        }
    }

    fn start(&mut self){
        let mut event_pump = self.context.sdl_ctx.event_pump().unwrap();
        let mut internals = C8::default();

        { //read file block
            let mut file = File::open(self.target_file.clone()).unwrap();

            file.read(&mut internals.memory[0x200..]).unwrap();
        }

        let mut gbuf = Box::new([0u8; 64*32]);

        gbuf[64+20] = 1;

        'running: loop {
            if let Ok(_) = self.kill_receiver.try_recv() {
                break 'running;
            }

            for event in event_pump.poll_iter(){
                if let Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape | Keycode::Q), .. } = event {
                    break 'running;
                }
            }


            


            self.render_graphics(&gbuf);
        }
    }

    fn render_graphics(&mut self, gbuf: &[u8; 64*32]){
        let canvas = &mut self.context.canvas;
        canvas.set_draw_color(Color::RGB(0,0,0));
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


pub fn start_thread(kill_receiver: Receiver<bool>) -> thread::JoinHandle<()>{
    thread::spawn(move || {
        let mut emulator = Emulator::new(kill_receiver, (r"C:\C8Games\Connect_4.ch8").to_owned());
        emulator.start();
    })
}