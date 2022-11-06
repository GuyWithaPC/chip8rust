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
use rand;

use std::{thread,time,vec,io};
use std::fs::File;
use std::io::{Read,Write};

const SCR_WIDTH: usize = 64;
const SCR_HEIGHT: usize = 32;
const CYCLES_PER_SECOND: u64 = 100;
const MICROS_PER_CYCLE: u64 = 1_000_000 / CYCLES_PER_SECOND;
const CLASSIC_BITSHIFT: bool = false;
const CLASSIC_JUMP: bool = false;

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
    fn flip_pixel(&mut self, x: usize, y: usize) -> bool {
        if y * SCR_WIDTH + x >= SCR_WIDTH * SCR_HEIGHT {return false} else {
            self.pixels[y * SCR_WIDTH + x] = !self.pixels[y * SCR_WIDTH + x];
            return !self.pixels[y*SCR_WIDTH+x]
        }
        return false
    }
    fn draw(&self, screen: &mut [u8]) {
        for (b,pix) in self.pixels.iter().zip(screen.chunks_exact_mut(4)) {
            let color = if *b {[0xff,0xff,0xff,0xff]} else {[0x00,0x00,0x00,0xff]};
            pix.copy_from_slice(&color);
        }
    }
}

struct KeyBlock {
    blocked: bool,
    register: u8
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
    ram.load_from_rom("chip8demos/Keypad Test.ch8",0x200);
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

    let mut keys_pressed = vec![false;0x10];

    let mut rng = rand::thread_rng();

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

        let mut blocked = KeyBlock {
            blocked: false,
            register: 0
        };
        if blocked.blocked { // catch the first key to be pressed and save it to
            for i in 0..0x10 {
                if keys_pressed[i] {
                    registers[blocked.register as usize] = i as u8;
                    blocked.blocked = false;
                    break;
                }
            }
        } else {
            let current_instruction: u16 = ((ram.get(program_counter) as u16) << 8u16) | ram.get(program_counter + 1) as u16;
            let opcode = (current_instruction & 0xF000) >> 12;
            //println!("op: {:#04x}",opcode);
            let n_x: usize = ((current_instruction & 0x0F00) >> 8) as usize;
            let n_y: usize = ((current_instruction & 0x00F0) >> 4) as usize;
            let n_n: u8 = (current_instruction & 0x000F) as u8;
            let n_nn: u8 = (current_instruction & 0x00FF) as u8;
            let n_nnn = current_instruction & 0x0FFF;
            program_counter += 2;
            match opcode {
                0x0 => {
                    if n_nnn == 0x0E0 {
                        display.clear();
                        println!("cleared the display.");
                    }
                    if n_nnn == 0x0EE {
                        print!("returned from a routine at {:#03} to ", program_counter);
                        program_counter = callstack.pop();
                        println!("{:03}", program_counter);
                    }
                },
                0x1 => {
                    program_counter = n_nnn;
                },
                0x2 => {
                    callstack.push(program_counter);
                    println!("called a routine at {:#03} from {:#03}.", n_nnn, program_counter);
                    program_counter = n_nnn;
                },
                0x3 => {
                    if registers[n_x] == n_nn {
                        program_counter = program_counter + 2;
                    }
                },
                0x4 => {
                    if registers[n_x] != n_nn {
                        program_counter = program_counter + 2;
                    }
                },
                0x5 => {
                    if registers[n_y] == registers[n_x] {
                        program_counter = program_counter + 2;
                    }
                },
                0x6 => {
                    registers[n_x as usize] = n_nn as u8;
                    println!("set register {:#01x} to {:02x}", n_x, n_nn);
                },
                0x7 => {
                    registers[n_x as usize] = ((registers[n_x as usize] as u16 + n_nn as u16) & 0xFF) as u8;
                    println!("set register {:#01x} to {:02x}", n_x, registers[n_x as usize]);
                },
                0x8 => {
                    // ALU stuff
                    let rx = registers[n_x];
                    let ry = registers[n_y];
                    registers[n_x] = match n_n {
                        0x0 => {
                            ry // copy y -> x
                        },
                        0x1 => {
                            rx | ry // bitwise or
                        },
                        0x2 => {
                            rx & ry // bitwise and
                        },
                        0x3 => {
                            rx ^ ry // bitwise xor
                        },
                        0x4 => {
                            let bigvalue: u16 = rx as u16 + ry as u16; // vx + vy
                            if bigvalue > 255 {
                                registers[0xF] = 1;
                            } else {
                                registers[0xF] = 0;
                            }
                            (bigvalue & 0xFF) as u8
                        },
                        0x5 => {
                            let subtraction: i16 = rx as i16 - ry as i16; // vx - vy
                            if rx > ry {
                                registers[0xF] = 1;
                            } else {
                                registers[0xF] = 0;
                            }
                            (subtraction & 0xFF) as u8
                        },
                        0x6 => {
                            // right bit shift
                            registers[0xF] = if CLASSIC_BITSHIFT { ry as u8 } else { rx as u8 } & 0x1;
                            if CLASSIC_BITSHIFT { // legacy behavior
                                ry >> 1
                            } else { // modern behavior
                                println!("shifted {rx} to {}",rx >> 1);
                                println!("set shift flag to {}",registers[0xF]);
                                (rx >> 1) & 0xFF
                            }
                        },
                        0x7 => {
                            let subtraction: i16 = ry as i16 - rx as i16; // vy - vx
                            if ry > rx {
                                registers[0xF] = 1;
                            } else {
                                registers[0xF] = 0;
                            }
                            (subtraction & 0xFF) as u8
                        },
                        0xE => {
                            // left bit shift
                            registers[0xF] = (if CLASSIC_BITSHIFT { ry } else { rx } >> 7) & 1;
                            if CLASSIC_BITSHIFT { // legacy behavior
                                ry << 1
                            } else { // modern behavior
                                rx << 1
                            }
                        },
                        _ => { rx }
                    }
                },
                0x9 => {
                    if registers[n_y] != registers[n_x] {
                        program_counter = program_counter + 2;
                    }
                },
                0xA => {
                    index_register = n_nnn;
                    println!("set index register to {:03x}", n_nnn);
                },
                0xB => {
                    if CLASSIC_JUMP { // legacy behavior
                        program_counter = n_nnn + registers[0] as u16;
                    } else { // modern behavior
                        program_counter = n_nnn + registers[n_x] as u16;
                    }
                },
                0xC => {
                    let random_number: u8 = rand::random();
                    registers[n_x] = random_number & n_nn;
                },
                0xD => {
                    let x_coord = registers[n_x as usize] % 64;
                    let y_coord = registers[n_y as usize] % 32;
                    let mut pixflip = false;
                    println!("draw function called with parameters x: {x_coord}, y: {y_coord}, and bytes: {n_n}.");
                    let mut draw_bytes = vec![0u8; n_n as usize];
                    for i in 0..n_n {
                        let bytebools = byte_to_bools(&ram.get(index_register + i as u16));
                        for x in 0..8 {
                            if bytebools[x] {
                                let this_pixel = display.flip_pixel(x + (x_coord as usize), (i as usize + y_coord as usize) as usize);
                                pixflip = if pixflip { pixflip } else {this_pixel};
                            }
                        }
                    }
                    registers[0xF] = if pixflip { 1 } else { 0 };
                },
                0xE => {
                    if n_nn == 0x9E { // skip if key pressed
                        if keys_pressed[n_x] {
                            program_counter += 2;
                        }
                    }
                    if n_nn == 0xA1 { // skip if key not pressed
                        if !keys_pressed[n_x] {
                            program_counter += 2;
                        }
                    }
                },
                0xF => {
                    match n_nn {
                        0x07 => { // copy delay into vx
                            registers[n_x] = delay_timer;
                            println!("Moved delay timer time {} into register {:#01}",delay_timer,n_x);
                        },
                        0x15 => { // copy vx to delay timer
                            delay_timer = registers[n_x];
                            println!("Set delay timer to {}",registers[n_x]);
                        },
                        0x18 => { // copy vx to sound timer
                            sound_timer = registers[n_x];
                        },
                        0x1E => { // add vx to index register
                            index_register += registers[n_x] as u16;
                        },
                        0x0A => { // block and get keypress
                            blocked.blocked = true;
                            blocked.register = n_x as u8;
                        },
                        0x29 => { // index register to font character
                            index_register = registers[n_x] as u16 * 5;
                        },
                        0x33 => { // binary-coded decimal conversion
                            let vx = registers[n_x];
                            ram.set(index_register,vx / 100);
                            ram.set(index_register+1,(vx / 10) % 10);
                            ram.set(index_register+2, vx % 10);
                        },
                        0x55 => { // memory store
                            for i in 0..0xF {
                                ram.set(index_register+i,registers[i as usize]);
                            }
                        },
                        0x65 => { // memory load
                            for i in 0..0xF {
                                registers[i] = ram.get(index_register+i as u16);
                            }
                        },
                        _ => {}
                    }
                },
                _ => {}
            }
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

            keys_pressed[1] = input.key_pressed(VirtualKeyCode::Key1);
            keys_pressed[2] = input.key_pressed(VirtualKeyCode::Key2);
            keys_pressed[3] = input.key_pressed(VirtualKeyCode::Key3);
            keys_pressed[4] = input.key_pressed(VirtualKeyCode::Q);
            keys_pressed[5] = input.key_pressed(VirtualKeyCode::W);
            keys_pressed[6] = input.key_pressed(VirtualKeyCode::E);
            keys_pressed[7] = input.key_pressed(VirtualKeyCode::A);
            keys_pressed[8] = input.key_pressed(VirtualKeyCode::S);
            keys_pressed[9] = input.key_pressed(VirtualKeyCode::D);
            keys_pressed[0] = input.key_pressed(VirtualKeyCode::X);
            keys_pressed[0xA] = input.key_pressed(VirtualKeyCode::Z);
            keys_pressed[0xB] = input.key_pressed(VirtualKeyCode::C);
            keys_pressed[0xC] = input.key_pressed(VirtualKeyCode::Key4);
            keys_pressed[0xD] = input.key_pressed(VirtualKeyCode::R);
            keys_pressed[0xE] = input.key_pressed(VirtualKeyCode::F);
            keys_pressed[0xF] = input.key_pressed(VirtualKeyCode::V);
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