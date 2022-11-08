use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

const RAM_SIZE: usize = 4096;
pub struct Ram {
    space: [u8; RAM_SIZE],
}
impl Ram {
    pub fn new() -> Ram {
        Ram {
            space: [0; RAM_SIZE],
        }
    }
    pub fn get(&mut self, addr: u16) -> u8 {
        self.space[addr as usize]
    }
    pub fn set(&mut self, addr: u16, val: u8) {
        self.space[addr as usize] = val;
    }
    pub fn load_from_rom(&mut self, loc: u16, file: PathBuf) {
        let mut rom_file = File::open(file).expect("Failed to open ROM file.");
        let mut buf = Vec::new();
        rom_file
            .read_to_end(&mut buf)
            .expect("Failed to read ROM file.");
        for i in 0..buf.len() as u16 {
            self.set(i + loc, buf[i as usize]);
        }
    }
    pub fn generate_dump(&mut self, start_loc: u16, end_loc: u16) -> String {
        let mut dump = String::new();
        for i in start_loc..end_loc {
            if i % 16 == 0 {
                dump += format!("{:#5X} =>", i).as_str();
            }
            if i % 16 == 15 {
                dump += "\n";
            }
            dump += format!(" {:#4X}", self.get(i)).as_str();
        }
        dump
    }
}

const NUM_REGISTERS: usize = 0x10;
pub struct Registers {
    space: [u8; NUM_REGISTERS],
}
impl Registers {
    pub fn new() -> Registers {
        Registers {
            space: [0; NUM_REGISTERS],
        }
    }
    pub fn get(&mut self, addr: u8) -> u8 {
        self.space[addr as usize]
    }
    pub fn set(&mut self, addr: u8, val: u8) {
        self.space[addr as usize] = val;
    }
}
