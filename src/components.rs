
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;

pub struct RAM {
    size: u16,
    space: Vec<u8>
}
impl RAM {
    pub fn new () -> RAM {
        RAM {
            size: 4096,
            space: vec![0u8;4096]
        }
    }
    pub fn get (&mut self, addr: u16) -> u8 { self.space[addr as usize] }
    pub fn set (&mut self, addr: u16, val: u8) { self.space[addr as usize] = val; }
    pub fn load_from_rom (&mut self, loc: u16, file: PathBuf) {
        let mut rom_file = File::open(file).expect("Failed to open ROM file.");
        let mut buf = Vec::new();
        rom_file.read_to_end(&mut buf).expect("Failed to read ROM file.");
        for i in 0..buf.len() as u16 {
            self.set(i+loc,buf[i as usize]);
        }
    }
    pub fn generate_dump (&mut self, start_loc: u16, end_loc: u16) -> String {
        let mut dump = String::new();
        for i in start_loc..end_loc {
            if i % 16 == 0 {
                dump += format!("{:#5X} =>",i).as_str();
            }
            if i % 16 == 15 {
                dump += "\n";
            }
            dump += format!(" {:#4X}",self.get(i)).as_str();
        }
        dump
    }
}

pub struct Registers {
    size: u8,
    space: Vec<u8>
}
impl Registers {
    pub fn new () -> Registers {
        Registers {
            size: 0x10,
            space: vec![0u8;0x10]
        }
    }
    pub fn get (&mut self, addr: u8) -> u8 {
        self.space[addr as usize]
    }
    pub fn set (&mut self, addr: u8, val: u8) {
        self.space[addr as usize] = val;
    }
}