use log::{debug,error};
use pixels::{Error, Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;
use rodio::{Decoder,OutputStream,source::Source};

use std::{thread,time,vec,io};
use std::fs::File;
use std::io::{Read,Write};

const SCR_WIDTH: usize = 64;
const SCR_HEIGHT: usize = 32;
const CYCLES_PER_SECOND: u64 = 700;
const MICROS_PER_CYCLE: u64 = 1_000_000 / CYCLES_PER_SECOND;

struct RAM {
    range: u16,
    space: Vec<u8>
}
impl RAM {
    fn empty() -> RAM {
        RAM {
            range: 4096u16,
            space: vec![0; 4096]
        }
    }
    fn init_default(&mut self){
        self.load_from_rom("SysROM/font.bin",0x00);
    }
    fn load_from_rom(&mut self, rom_path: &str, start_index: u16) {
        let mut rom_file = File::open(&rom_path).expect("Could not open the ROM file.");
        let metadata = std::fs::metadata(&rom_path).expect("Unable to read ROM metadata");
        let mut read_space = vec![0u8; metadata.len() as usize];
        rom_file.read(&mut read_space).expect("Could not read from the ROM file.");
        for i in 0..read_space.len() as u16 {
            self.set(start_index + i as u16, read_space[i as usize]);
        }
    }
    fn get(&self,index: u16) -> u8 {
        self.space[index as usize]
    }
    fn set(&mut self, index: u16, value: u8) {
        self.space[index as usize] = value;
    }
}

struct Stack {
    pointers: Vec<u16>,
    size: usize
}
impl Stack {
    fn empty() -> Stack {
        Stack {
            pointers: Vec::new(),
            size: 0
        }
    }
    fn push(&mut self, pointer: u16) {
        self.pointers.push(pointer);
        self.size += 1;
    }
    fn pop(&mut self) -> u16 {
        self.size -= 1;
        self.pointers.pop().unwrap()
    }
}

struct Display {
    pixels: Vec<bool> // this is all the pixels arranged linearly left-right top-bottom
}
impl Display {
    fn empty() -> Display {
        Display {
            pixels: vec![false;SCR_WIDTH * SCR_HEIGHT]
        }
    }
    fn clear(&mut self) {
        self.pixels = vec![false;SCR_WIDTH * SCR_HEIGHT];
    }
    fn set_pixel(&mut self, x: usize, y: usize, case: bool) {
        if y * SCR_WIDTH + x >= SCR_WIDTH * SCR_HEIGHT {()} else {
            self.pixels[y * SCR_WIDTH + x] = case;
        }
    }
    fn flip_pixel(&mut self, x: usize, y: usize) {
        if y * SCR_WIDTH + x >= SCR_WIDTH * SCR_HEIGHT {()} else {
            self.pixels[y * SCR_WIDTH + x] = !self.pixels[y * SCR_WIDTH + x];
        }
    }
    fn draw(&self, screen: &mut [u8]) {
        for (b,pix) in self.pixels.iter().zip(screen.chunks_exact_mut(4)) {
            let color = if *b {[0xff,0xff,0xff,0xff]} else {[0x00,0x00,0x00,0xff]};
            pix.copy_from_slice(&color);
        }
    }
}

fn main() -> Result<(),Error>{

    // display and event loop setup

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(SCR_WIDTH as f64, SCR_HEIGHT as f64);
        let scaled_size = LogicalSize::new(SCR_WIDTH as f64 * 3.0, SCR_HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title("chip-8 interpreter")
            .with_inner_size(scaled_size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width,window_size.height,&window);
        Pixels::new(SCR_WIDTH as u32, SCR_HEIGHT as u32, surface_texture)?
    };

    let mut display = Display::empty();

    // display setup finished. now processor setup begins.

    let mut ram = RAM::empty();
    ram.init_default();
    ram.load_from_rom("chip8demos/IBM Logo.ch8",0x200);
    println!("ram dump: ");
    for i in 0..ram.range as usize {
        print!("{:#02x} ",ram.space[i]);
        if i % 16 == 15 {
            println!();
            print!("{:#04x} => ",i+1);
        }
    }
    io::stdout().flush().unwrap();
    let mut callstack = Stack::empty();
    let mut index_register: u16 = 0;
    let mut registers = vec![0u8;16];
    let mut program_counter: u16 = 0x200;
    let mut delay_timer: u8 = 0;
    let mut sound_timer: u8 = 0;
    let mut time_since_count: u128 = 0;

    // processor setup finished. event loop now.
    let now = time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        // do time stuff to decrement the delay timer
        let delta = now.elapsed().as_millis();
        time_since_count += delta;
        let now = time::Instant::now();
        if time_since_count > (1000/60) as u128 {
            delay_timer = if delay_timer == 0 {0} else {delay_timer - 1};
            sound_timer = if sound_timer == 0 {0} else {sound_timer - 1};
            time_since_count = 0;
        }

        // do program counter and instruction stuff

        let current_instruction: u16 = ((ram.get(program_counter) as u16) << 8u16) | ram.get(program_counter+1) as u16;
        let opcode = (current_instruction & 0xF000) >> 12;
        //println!("op: {:#04x}",opcode);
        let n_x = (current_instruction & 0x0F00) >> 8;
        let n_y = (current_instruction & 0x00F0) >> 4;
        let n_n = current_instruction & 0x000F;
        let n_nn = current_instruction & 0x00FF;
        let n_nnn = current_instruction & 0x0FFF;
        program_counter += 2;
        match opcode {
            0x0 => {
                if (n_x << 8) | (n_y << 4) | n_n == 0x0E0 {
                    display.clear();
                }
                println!("cleared the display.");
            },
            0x1 => {
                program_counter = n_nnn;
            },
            0x2 => {

            },
            0x3 => {

            },
            0x4 => {

            },
            0x5 => {

            },
            0x6 => {
                registers[n_x as usize] = n_nn as u8;
                println!("set register {:#01x} to {:02x}",n_x,n_nn);
            },
            0x7 => {
                registers[n_x as usize] = (registers[n_x as usize] as u16 + n_nn) as u8;
                println!("set register {:#01x} to {:02x}",n_x,registers[n_x as usize]);
            },
            0x8 => {

            },
            0x9 => {

            },
            0xA => {
                index_register = n_nnn;
                println!("set index register to {:03x}",n_nnn);
            },
            0xB => {

            },
            0xC => {

            },
            0xD => {
                let x_coord = registers[n_x as usize];
                let y_coord = registers[n_y as usize];
                println!("draw function called with parameters x: {x_coord}, y: {y_coord}, and bytes: {n_n}.");
                let mut draw_bytes = vec![0u8; n_n as usize];
                for i in 0..n_n {
                    let bytebools = byte_to_bools(&ram.get(index_register+i));
                    for x in 0..8 {
                        if bytebools[x] {
                            display.flip_pixel(x+(x_coord as usize),(i as usize+y_coord as usize) as usize);
                        }
                    }
                }
            },
            0xE => {

            },
            0xF => {

            },
            _ => {}
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                control_flow.set_exit();
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            },
            Event::RedrawRequested(_) => {
                display.draw(pixels.get_frame_mut());
                if pixels.render()
                    .map_err(|e| error!("pixels.render() failed: {}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            },
            _ => {}
        }

        if input.update(&event) {
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }
            window.request_redraw();
        }
        //control_flow.set_wait_until(time::Instant::now() + time::Duration::from_micros(MICROS_PER_CYCLE));
    });
}

fn byte_to_bools(byte: &u8) -> Vec<bool> {
    let mut byte_vector = vec![false;8];
    for i in 0..8 {
        byte_vector[i] = (byte & (1 << (7-i))) >> (7-i) == 1;
    }
    byte_vector
}