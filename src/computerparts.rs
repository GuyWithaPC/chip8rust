use std::fs::File;
use std::io::{Read,Write};
use std::vec;
use std::time;

pub struct RAM {
    pub range: u16,
    space: Vec<u8>
}
impl RAM {
    pub fn empty() -> RAM {
        RAM {
            range: 4096u16,
            space: vec![0; 4096]
        }
    }
    pub fn init_default(&mut self){
        self.load_from_rom("SysROM/font.bin",0x00);
    }
    pub fn load_from_rom(&mut self, rom_path: &str, start_index: u16) {
        let mut rom_file = File::open(&rom_path).expect("Could not open the ROM file.");
        let metadata = std::fs::metadata(&rom_path).expect("Unable to read ROM metadata");
        let mut read_space = vec![0u8; metadata.len() as usize];
        rom_file.read(&mut read_space).expect("Could not read from the ROM file.");
        for i in 0..read_space.len() as u16 {
            self.set(start_index + i as u16, read_space[i as usize]);
        }
    }
    pub fn get(&self,index: u16) -> u8 {
        self.space[index as usize]
    }
    pub fn set(&mut self, index: u16, value: u8) {
        self.space[index as usize] = value;
    }
}

pub struct Stack {
    pub pointers: Vec<u16>,
    size: usize
}
impl Stack {
    pub fn empty() -> Stack {
        Stack {
            pointers: Vec::new(),
            size: 0
        }
    }
    pub fn push(&mut self, pointer: u16) {
        self.pointers.push(pointer);
        self.size += 1;
    }
    pub fn pop(&mut self) -> u16 {
        self.size -= 1;
        self.pointers.pop().unwrap()
    }
}

pub struct Instruction {
    pub opcode: u8,
    pub x: u8,
    pub y: u8,
    pub n: u8,
    pub nn: u8,
    pub nnn: u16
}
impl Instruction {
    pub fn from_bytes(highbyte: u8, lowbyte: u8) -> Instruction {
        Instruction {
            opcode: (highbyte & 0xF0) >> 4,
            x: highbyte & 0x0F,
            y: (lowbyte & 0xF0) >> 4,
            n: lowbyte & 0x0F,
            nn: lowbyte,
            nnn: (((highbyte as u16) << 8) | lowbyte as u16) & 0x0FFF
        }
    }
}

pub struct Registers {
    pub p_c: u16,
    pub ind: u16,
    gen: Vec<u8>
}
impl Registers {
    pub fn new() -> Registers {
        Registers {
            p_c: 0x0200,
            ind: 0x0000,
            gen: vec![0u8;0x10]
        }
    }
    pub fn get(&mut self, reg: u8) -> u8 {
        self.gen[reg as usize]
    }
    pub fn set(&mut self, reg: u8, val: u8) {
        self.gen[reg as usize] = val;
    }
    pub fn set_flag(&mut self, val: u8) { self.gen[0xF] = val; }
}

pub struct Timers {
    pub delay: u8,
    pub sound: u8,
    delta: u128
}
impl Timers {
    pub fn new() -> Timers {
        Timers {
            delay: 0u8,
            sound: 0u8,
            delta: 0
        }
    }
    pub fn decrement(&mut self, dur: time::Duration) {
        self.delta += dur.as_millis();
        if self.delta >= (1000 / 60) {
            self.delay = if self.delay == 0 {0} else {self.delay - 1};
            self.sound = if self.sound == 0 {0} else {self.sound - 1};
        }
    }
}