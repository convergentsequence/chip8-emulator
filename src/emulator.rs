#![allow(unused_variables, dead_code, unused_imports)]

use std::sync::mpsc::Receiver;
use std::thread;

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

pub struct Emulator{
    kill_receiver: Receiver<bool>,
    target_file: String,
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

        EmulatorContext{ sdl_ctx: sdl_ctx, canvas: canvas }
    }

    fn new(kill_receiver: Receiver<bool>, target_file: String) -> Emulator {
        Emulator { 
            kill_receiver: kill_receiver, 
            target_file: target_file 
        }
    }

    fn start(&mut self){
        let mut context = Emulator::init_context();
        let mut event_pump = context.sdl_ctx.event_pump().unwrap();

        'running: loop {
            if let Ok(_) = self.kill_receiver.try_recv() {
                break 'running;
            }

            context.canvas.set_draw_color(Color::RGB(0,0,0));
            context.canvas.clear();

            context.canvas.set_draw_color(Color::RGB(255, 255, 255));
            context.canvas.draw_point(Point::new(10,10)).unwrap();

            for event in event_pump.poll_iter(){
                if let Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape | Keycode::Q), .. } = event {
                    break 'running;
                }
            }

            context.canvas.present();
        }
    }
}


pub fn start_thread(kill_receiver: Receiver<bool>) -> thread::JoinHandle<()>{
    thread::spawn(move || {
        let mut emulator = Emulator::new(kill_receiver, "testing".to_owned());
        emulator.start();
    })
}