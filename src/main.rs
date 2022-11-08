use std::path::PathBuf;
use std::vec;

use olc_pge as olc;

mod components;
mod instructions;

use components::{Registers, RAM};

const SCR_W: usize = 64;
const SCR_H: usize = 32;
const ROM_FILE: &str = "chip8demos/BLINKY.ch8";
const CYCLES_PER_SECOND: f32 = 600.0;
const SECONDS_PER_CYCLE: f32 = 1.0 / CYCLES_PER_SECOND;
const DRAW_BIGGER_PIXELS: i32 = 4;
const START_RUN_MODE: RUN = RUN::STEP;

#[derive(PartialEq)]
enum RUN {
    CONT,
    STEP,
}

impl olc::PGEApplication for Emulator {
    const APP_NAME: &'static str = "Chip8 Emulator - Rust";

    fn on_user_create(&mut self, _pge: &mut olc::PixelGameEngine) -> bool {
        self.ram
            .load_from_rom(0x000, PathBuf::from("system/font.bin"));
        self.ram.load_from_rom(0x200, PathBuf::from(ROM_FILE));
        true
    }

    fn on_user_update(&mut self, pge: &mut olc::PixelGameEngine, delta: f32) -> bool {
        self.keys[0x0] = pge.get_key(olc::Key::X).held;
        self.keys[0x1] = pge.get_key(olc::Key::K1).held;
        self.keys[0x2] = pge.get_key(olc::Key::K2).held;
        self.keys[0x3] = pge.get_key(olc::Key::K3).held;
        self.keys[0x4] = pge.get_key(olc::Key::Q).held;
        self.keys[0x5] = pge.get_key(olc::Key::W).held;
        self.keys[0x6] = pge.get_key(olc::Key::E).held;
        self.keys[0x7] = pge.get_key(olc::Key::A).held;
        self.keys[0x8] = pge.get_key(olc::Key::S).held;
        self.keys[0x9] = pge.get_key(olc::Key::D).held;
        self.keys[0xA] = pge.get_key(olc::Key::Z).held;
        self.keys[0xB] = pge.get_key(olc::Key::C).held;
        self.keys[0xC] = pge.get_key(olc::Key::K4).held;
        self.keys[0xD] = pge.get_key(olc::Key::R).held;
        self.keys[0xE] = pge.get_key(olc::Key::F).held;
        self.keys[0xF] = pge.get_key(olc::Key::V).held;
        if self.key_block != 0x10 {
            for i in 0..0x10 as usize {
                if self.keys[i] {
                    self.registers.set(self.key_block, i as u8);
                    self.key_block = 0x10;
                    break;
                }
            }
            if self.key_block != 0x10 {
                return true;
            }
        }
        if self.run_mode == RUN::CONT {
            // run continuously at 600 CPS
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
            if self.cycle_time >= SECONDS_PER_CYCLE {
                let (redraw, summary) = self.cycle();
                if redraw {
                    self.draw(pge);
                }
                self.draw_debug(pge, summary);
                self.cycle_time = 0.0;
            }
            if pge.get_key(olc::Key::Space).pressed {
                self.run_mode = RUN::STEP
            }
        } else {
            // run step-by-step
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
                let (redraw, summary) = self.cycle();
                self.draw(pge);
                self.draw_debug(pge, summary);
            }
            if pge.get_key(olc::Key::Space).pressed {
                self.run_mode = RUN::CONT
            }
        }
        true
    }
}

fn main() {
    olc::PixelGameEngine::construct(
        Emulator::new(),
        (SCR_W + (SCR_W / 2)) * DRAW_BIGGER_PIXELS as usize,
        (SCR_H + (SCR_H / 2)) * DRAW_BIGGER_PIXELS as usize,
        2,
        2,
    )
    .start();
}

struct Emulator {
    cycle_time: f32,
    timer_time: f32,
    display: Vec<Vec<bool>>,
    ram: RAM,
    timer: u8,
    sound_timer: u8,
    registers: Registers,
    program_counter: u16,
    stack_pointer: u16,
    call_stack: Vec<u16>,
    key_block: u8,
    keys: Vec<bool>,
    run_mode: RUN,
}
impl Emulator {
    fn new() -> Emulator {
        Emulator {
            timer_time: 0.0,
            cycle_time: 0.0,
            display: vec![vec![false; 32]; 64], // x, y format
            ram: RAM::new(), // using RAM rather than a Vec because it encapsulates ROM loading
            timer: 0x00,     // basic timer
            sound_timer: 0x00, // sound timer, plays a beep while > 0
            registers: Registers::new(), // registers 0 through F
            program_counter: 0x200, // programs always start at location 0x200 in RAM
            stack_pointer: 0x000, // doesn't matter where this starts, programs will modify it
            call_stack: Vec::new(),
            key_block: 0x10,
            keys: vec![false; 0x10],
            run_mode: START_RUN_MODE,
        }
    }
    fn draw(&mut self, pge: &mut olc::PixelGameEngine) {
        let bigger_draw = if self.run_mode == RUN::STEP {
            DRAW_BIGGER_PIXELS
        } else {
            DRAW_BIGGER_PIXELS + (DRAW_BIGGER_PIXELS / 2)
        };
        let color_on = if self.run_mode == RUN::STEP {
            olc::Pixel::rgb(0xDF, 0x00, 0x00)
        } else {
            olc::Pixel::rgb(0x00, 0xDF, 0x00)
        };
        let color_off = if self.run_mode == RUN::STEP {
            olc::Pixel::rgb(0x10, 0x00, 0x00)
        } else {
            olc::Pixel::rgb(0x00, 0x10, 0x00)
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
        if self.run_mode == RUN::STEP {
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
