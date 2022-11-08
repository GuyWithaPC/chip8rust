use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use clap::Parser;
use olc_pge as olc;
use olc_pge::Key;
use rodio::{Decoder, OutputStream, Sink, Source};

mod components;
mod instructions;

use components::{Ram, Registers};

const SCR_W: usize = 64;
const SCR_H: usize = 32;
const DRAW_BIGGER_PIXELS: i32 = 4;
const START_RUN_MODE: RunMode = RunMode::Step;

const KEYS: [Key; 0x10] = [
    Key::X,
    Key::K1,
    Key::K2,
    Key::K3,
    Key::Q,
    Key::W,
    Key::E,
    Key::A,
    Key::S,
    Key::D,
    Key::Z,
    Key::C,
    Key::K4,
    Key::R,
    Key::F,
    Key::V,
];


#[derive(PartialEq, Debug, Clone, clap::ValueEnum)]
enum RunMode {
    Play,
    Step,
}

#[derive(PartialEq, Debug, Clone, clap::ValueEnum)]
enum ColorMode {
    Green,
    Gray,
    White,
}

#[derive(PartialEq, Debug, Clone, clap::ValueEnum)]
enum InputMode {
    Once,
    Hold,
}

#[derive(Debug, Parser)]
struct Args {
    /// The ROM file to load
    #[clap(short, long, value_parser)]
    rom_file: String,
    /// The target execution speed for the processor (in cycles per second)
    #[clap(short, long, default_value_t = 600.0)]
    cycle_speed: f32,
    /// Whether to start the program paused or not
    #[clap(value_enum, short = 'm', long, default_value_t = RunMode::Play)]
    run_mode: RunMode,
    /// The color mode to use for the display
    #[clap(value_enum, long, default_value_t = ColorMode::White)]
    color_mode: ColorMode,
    /// The mode for the input keys (press once / hold)
    #[clap(value_enum, long, default_value_t = InputMode::Hold)]
    input_mode: InputMode,
}

impl olc::PGEApplication for Emulator {
    const APP_NAME: &'static str = "Chip8 Emulator - Rust";

    fn on_user_create(&mut self, _pge: &mut olc::PixelGameEngine) -> bool {
        true
    }

    fn on_user_update(&mut self, pge: &mut olc::PixelGameEngine, delta: f32) -> bool {
        for (i, key) in KEYS.iter().enumerate() {
            match self.input_mode {
                InputMode::Hold => { self.keys[i] = pge.get_key(*key).held; },
                InputMode::Once => {
                    if self.run_mode == RunMode::Step { // it's still hold mode for stepping, otherwise it'd be hard
                        self.keys[i] = pge.get_key(*key).held;
                    } else {
                        self.keys[i] = pge.get_key(*key).pressed;
                    }
                },
            }
        }

        if self.key_block != 0x10 {
            for (i, key) in self.keys.iter().enumerate() {
                if *key {
                    self.registers.set(self.key_block, i as u8);
                    self.key_block = 0x10;
                    break;
                }
            }
            if self.key_block != 0x10 {
                return true;
            }
        }
        if self.run_mode == RunMode::Play {
            // run continuously at 600 CPS
            //decrement the timers
            self.cycle_time += delta;
            self.timer_time += delta;
            if self.timer_time >= 1.0 / 60.0 {
                self.timer = if self.timer == 0 { 0 } else { self.timer - 1 };
                self.sound_timer = if self.sound_timer == 0 {
                    0
                } else {
                    self.sound_timer - 1
                };
                self.timer_time = 0.0;
            }
            // run the beeper if the sound timer is > 0
            if self.sound_timer > 0 {
                self.beeper.play();
            } else {
                self.beeper.pause();
            }

            // run once if the cycle time is full
            if self.cycle_time >= self.time_per_cycle {
                let (redraw, summary) = self.cycle();
                if redraw {
                    self.draw(pge);
                }
                self.draw_debug(pge, summary);
                self.cycle_time = 0.0;
            }
            if pge.get_key(olc::Key::Space).pressed {
                self.run_mode = RunMode::Step
            }
        } else {
            // run step-by-step
            self.beeper.pause(); // it'd be annoying if this kept going
            if pge.get_key(olc::Key::Tab).pressed {
                self.timer_time += 1.0 / 600.0;
                if self.timer_time >= 1.0 / 60.0 {
                    self.timer = if self.timer == 0 { 0 } else { self.timer - 1 };
                    self.sound_timer = if self.sound_timer == 0 {
                        0
                    } else {
                        self.sound_timer - 1
                    };
                    self.timer_time = 0.0
                }
                let (_redraw, summary) = self.cycle();
                self.draw(pge);
                self.draw_debug(pge, summary);
            }
            if pge.get_key(olc::Key::Space).pressed {
                self.run_mode = RunMode::Play
            }
        }
        true
    }
}

fn main() {
    let args = Args::parse();

    // set up audio (rodio audio setup only works in main)
    let mut emulator = Emulator::new();
    emulator.load_rom(&args.rom_file);
    emulator.time_per_cycle = 1.0 / args.cycle_speed;
    emulator.run_mode = args.run_mode;
    emulator.color_mode = args.color_mode;
    emulator.input_mode = args.input_mode;

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file = File::open("system/square.ogg").unwrap();
    let new_sound = stream_handle.play_once(BufReader::new(file)).unwrap();
    new_sound.set_volume(0.2);

    let looping_source = Decoder::new(File::open("system/square.ogg").unwrap()).unwrap();
    let looping_source = Source::repeat_infinite(looping_source);
    new_sound.append(looping_source);

    new_sound.pause();

    emulator.beeper = new_sound;

    // run the olc::pge application
    olc::PixelGameEngine::construct(
        emulator,
        (SCR_W + (SCR_W / 2)) * DRAW_BIGGER_PIXELS as usize,
        (SCR_H + (SCR_H / 2)) * DRAW_BIGGER_PIXELS as usize,
        2,
        2,
    )
    .start();
}

struct Emulator {
    time_per_cycle: f32,
    cycle_time: f32,
    timer_time: f32,
    display: [[bool; SCR_H]; SCR_W],
    color_mode: ColorMode,
    ram: Ram,
    timer: u8,
    sound_timer: u8,
    registers: Registers,
    program_counter: u16,
    stack_pointer: u16,
    call_stack: Vec<u16>,
    key_block: u8,
    keys: [bool; 0x10],
    input_mode: InputMode,
    run_mode: RunMode,
    beeper: Sink,
}
impl Emulator {
    fn new() -> Emulator {
        let mut ram = Ram::new();
        ram.load_from_rom(0x000, PathBuf::from("system/font.bin"));

        Emulator {
            time_per_cycle: 1.0/600.0,
            timer_time: 0.0,
            cycle_time: 0.0,
            display: [[false; SCR_H]; SCR_W], // x, y format
            color_mode: ColorMode::White,
            ram, // using RAM rather than a Vec because it encapsulates ROM loading
            timer: 0x00,     // basic timer
            sound_timer: 0x00, // sound timer, plays a beep while > 0
            registers: Registers::new(), // registers 0 through F
            program_counter: 0x200, // programs always start at location 0x200 in RAM
            stack_pointer: 0x000, // doesn't matter where this starts, programs will modify it
            call_stack: Vec::new(),
            key_block: 0x10,
            keys: [false; 0x10],
            input_mode: InputMode::Hold,
            run_mode: START_RUN_MODE,
            beeper: Sink::try_new(&OutputStream::try_default().unwrap().1).unwrap(),
        }
    }
    fn load_rom(&mut self, rom_file: &str) {
        self.ram.load_from_rom(0x200, PathBuf::from(rom_file));
    }
    fn draw(&mut self, pge: &mut olc::PixelGameEngine) {
        let bigger_draw = if self.run_mode == RunMode::Step {
            DRAW_BIGGER_PIXELS
        } else {
            DRAW_BIGGER_PIXELS + (DRAW_BIGGER_PIXELS / 2)
        };
        let color_on = match self.color_mode {
            ColorMode::White => olc::WHITE,
            ColorMode::Gray => olc::DARK_GREY,
            ColorMode::Green => olc::GREEN,
        };
        let color_off = match self.color_mode {
            ColorMode::White => olc::BLACK,
            ColorMode::Gray => olc::GREY,
            ColorMode::Green => olc::VERY_DARK_GREEN,
        };
        pge.clear(olc::BLACK);
        for x in 0..64 {
            for y in 0..32 {
                for xs in 0..bigger_draw {
                    for ys in 0..bigger_draw {
                        let pixel = if self.display[x][y] {
                            color_on
                        } else {
                            color_off
                        };
                        pge.draw(
                            x as i32 * bigger_draw + xs,
                            y as i32 * bigger_draw + ys,
                            pixel,
                        );
                    }
                }
            }
        }
    }
    fn draw_debug(&mut self, pge: &mut olc::PixelGameEngine, summary: String) {
        if self.run_mode == RunMode::Step {
            for i in 0..0x8 {
                let mut string = String::new();
                string += format!("R{:1X}:", i).as_str();
                string += format!("{:2X}", self.registers.get(i as u8)).as_str();
                pge.draw_string(
                    64 * DRAW_BIGGER_PIXELS + 4,
                    4 + (i * 8),
                    &string,
                    olc::WHITE,
                );
            }
            for i in 0..0x8 {
                let mut string = String::new();
                string += format!("R{:1X}:", 0x8 + i).as_str();
                string += format!("{:2X}", self.registers.get(0x8 + i as u8)).as_str();
                pge.draw_string(
                    64 * DRAW_BIGGER_PIXELS + 64,
                    4 + (i * 8),
                    &string,
                    olc::GREY,
                );
            }
            let (stringa, stringb) = summary.split_once(" => ").unwrap();
            pge.draw_string(
                4,
                32 * DRAW_BIGGER_PIXELS + 8,
                &stringa.to_string(),
                olc::WHITE,
            );
            pge.draw_string(
                4,
                32 * DRAW_BIGGER_PIXELS + 16,
                &stringb.to_string(),
                olc::WHITE,
            );
        }
    }
}
