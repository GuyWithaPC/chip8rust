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

mod computerparts;
use computerparts::{RAM,Stack,Instruction,Registers,Timers};

const SCR_WIDTH: usize = 64;
const SCR_HEIGHT: usize = 32;
const CYCLES_PER_SECOND: u64 = 100;
const MICROS_PER_CYCLE: u64 = 1_000_000 / CYCLES_PER_SECOND;
const CLASSIC_BITSHIFT: bool = false;
const CLASSIC_JUMP: bool = false;

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
    ram.load_from_rom("chip8demos/c8_test.ch8",0x200);
    println!("ram dump: ");
    for i in 0..ram.range as usize {
        print!("{:#02x} ",ram.get(i as u16));
        if i % 16 == 15 && i != 0x1000-1 {
            println!();
            print!("{:#04x} => ",i+1);
        }
    }
    io::stdout().flush().unwrap();

    let mut callstack = Stack::empty();
    let mut registers = Registers::new();
    let mut timers = Timers::new();

    let mut keys_pressed = vec![false;0x10];

    let mut rng = rand::thread_rng();

    let mut blocked = KeyBlock {
        blocked: false,
        register: 0
    };
    // processor setup finished. event loop now.
    let now = time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        // do time stuff to decrement the delay timer
        let delta = now.elapsed();
        timers.decrement(delta);
        let now = time::Instant::now();

        // do program counter and instruction stuff

        if blocked.blocked { // catch the first key to be pressed and save it to the register in the blocker
            for i in 0..0x10 {
                if keys_pressed[i] {
                    registers.set(blocked.register, i as u8);
                    blocked.blocked = false;
                    break;
                }
            }
        } else {
            let instruction = Instruction::from_bytes(
                ram.get(registers.p_c),
                ram.get(registers.p_c+1)
            );
            registers.p_c += 2;
            match instruction.opcode {
                0x0 => {
                    if instruction.nnn == 0x0E0 { // clear screen
                        display.clear();
                    }
                    if instruction.nnn == 0x0EE { // return from subroutine
                        registers.p_c = callstack.pop();
                    }
                },
                0x1 => { // jump
                    registers.p_c = instruction.nnn;
                },
                0x2 => { // call subroutine
                    callstack.push(registers.p_c);
                    registers.p_c = instruction.nnn;
                },
                0x3 => { // immediate conditional jump (EQ)
                    if registers.get(instruction.x) == instruction.nn {
                        registers.p_c += 2;
                    }
                },
                0x4 => { // immediate conditional jump (NEQ)
                    if registers.get(instruction.x) != instruction.nn {
                        registers.p_c += 2;
                    }
                },
                0x5 => { // register conditional jump (EQ)
                    if registers.get(instruction.y) == registers.get(instruction.x) {
                        registers.p_c += 2;
                    }
                },
                0x6 => { // immediate load
                    registers.set(instruction.x,instruction.nn);
                },
                0x7 => { // immediate add
                    let x = registers.get(instruction.x);
                    let (result, overflow) = x.overflowing_add(instruction.nn);
                    registers.set(instruction.x,result);
                },
                0x8 => { // ALU stuff
                    let rx = registers.get(instruction.x);
                    let ry = registers.get(instruction.y);
                    let result = match instruction.n {
                        0x0 => { // copy y -> x
                            ry
                        },
                        0x1 => { // bitwise or
                            rx | ry
                        },
                        0x2 => { // bitwise and
                            rx & ry
                        },
                        0x3 => { // bitwise xor
                            rx ^ ry
                        },
                        0x4 => { // add (with overflow)
                            let (result, overflow) = rx.overflowing_add(ry);
                            registers.set_flag(if overflow {1} else {0});
                            result
                        },
                        0x5 => { // subtract rx-ry (with !overflow)
                            let (result, overflow) = rx.overflowing_sub(ry);
                            registers.set_flag(if overflow {0} else {1});
                            result
                        },
                        0x6 => { // bit shift right (with overflow)
                            registers.set_flag(rx & 1);
                            rx >> 2
                        },
                        0x7 => { // subtract ry-rx (with !overflow)
                            let (result, overflow) = ry.overflowing_sub(rx);
                            registers.set_flag(if overflow {0} else {1});
                            result
                        },
                        0xE => { // bit shift left (with overflow)
                            registers.set_flag((rx & 0b10000000) >> 7);
                            (rx ^ (rx & 0b10000000)) * 2
                        },
                        _ => { rx }
                    };
                    registers.set(instruction.x,result);
                },
                0x9 => { // register conditional jump (NEQ)
                    if registers.get(instruction.x) != registers.get(instruction.y) {
                        registers.p_c += 2;
                    }
                },
                0xA => { // immediate set index register
                    registers.ind = instruction.nnn;
                },
                0xB => { // jump to nnn + r0
                    registers.p_c = instruction.nnn + registers.get(0x0) as u16;
                },
                0xC => { // set rx to random & nn
                    let random_number: u8 = rand::random();
                    registers.set(instruction.x,random_number & instruction.nn);
                },
                0xD => { // draw bytes
                    let x_coord = registers.get(instruction.x) % 64;
                    let y_coord = registers.get(instruction.y) % 32;
                    let mut pixflip = false;
                    let mut draw_bytes = vec![0u8; instruction.n as usize];
                    for i in 0..instruction.n {
                        let bytebools = byte_to_bools(&ram.get(registers.ind + i as u16));
                        for x in 0..8 {
                            if bytebools[x] {
                                let this_pixel = display.flip_pixel(x + (x_coord as usize), (i as usize + y_coord as usize) as usize);
                                pixflip = if pixflip { pixflip } else {this_pixel};
                            }
                        }
                    }
                    registers.set(0xF,if pixflip { 1 } else { 0 });
                },
                0xE => {
                    if instruction.nn == 0x9E { // skip if key pressed
                        if keys_pressed[instruction.x as usize] {
                            registers.p_c += 2;
                        }
                    }
                    if instruction.nn == 0xA1 { // skip if key not pressed
                        if !keys_pressed[instruction.x as usize] {
                            registers.p_c += 2;
                        }
                    }
                },
                0xF => {
                    match instruction.nn {
                        0x07 => { // copy delay into vx
                            registers.set(instruction.x,timers.delay);
                        },
                        0x15 => { // copy vx to delay timer
                            timers.delay = registers.get(instruction.x);
                        },
                        0x18 => { // copy vx to sound timer
                            timers.sound = registers.get(instruction.x);
                        },
                        0x1E => { // add vx to index register
                            registers.ind += registers.get(instruction.x) as u16;
                        },
                        0x0A => { // block and get keypress
                            blocked.blocked = true;
                            blocked.register = instruction.x as u8;
                        },
                        0x29 => { // index register to font character at rx
                            registers.ind = registers.get(instruction.x) as u16 * 5;
                        },
                        0x33 => { // binary-coded decimal conversion of rx
                            let rx = registers.get(instruction.x);
                            ram.set(registers.ind,rx / 100);
                            ram.set(registers.ind+1,(rx / 10) % 10);
                            ram.set(registers.ind+2, rx % 10);
                        },
                        0x55 => { // memory store
                            for i in 0..0x10 {
                                ram.set(registers.ind+i,registers.get(i as u8));
                            }
                        },
                        0x65 => { // memory load
                            for i in 0..0x10 {
                                registers.set(i as u8, ram.get(registers.ind+i as u16));
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

            keys_pressed[1] = input.key_held(VirtualKeyCode::Key1);
            keys_pressed[2] = input.key_held(VirtualKeyCode::Key2);
            keys_pressed[3] = input.key_held(VirtualKeyCode::Key3);
            keys_pressed[4] = input.key_held(VirtualKeyCode::Q);
            keys_pressed[5] = input.key_held(VirtualKeyCode::W);
            keys_pressed[6] = input.key_held(VirtualKeyCode::E);
            keys_pressed[7] = input.key_held(VirtualKeyCode::A);
            keys_pressed[8] = input.key_held(VirtualKeyCode::S);
            keys_pressed[9] = input.key_held(VirtualKeyCode::D);
            keys_pressed[0] = input.key_held(VirtualKeyCode::X);
            keys_pressed[0xA] = input.key_held(VirtualKeyCode::Z);
            keys_pressed[0xB] = input.key_held(VirtualKeyCode::C);
            keys_pressed[0xC] = input.key_held(VirtualKeyCode::Key4);
            keys_pressed[0xD] = input.key_held(VirtualKeyCode::R);
            keys_pressed[0xE] = input.key_held(VirtualKeyCode::F);
            keys_pressed[0xF] = input.key_held(VirtualKeyCode::V);
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