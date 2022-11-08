use crate::Emulator;

pub struct Instruction {
    opcode: u8,
    x: u8,
    y: u8,
    n: u8,
    byte: u8,
    addr: u16,
    full: u16,
}
impl Instruction {
    pub fn from(msb: u8, lsb: u8) -> Instruction {
        let combined = ((msb as u16) << 8) | lsb as u16;
        Instruction {
            full: combined,
            opcode: ((combined & 0xF000) >> 12) as u8, // first nibble
            x: ((combined & 0x0F00) >> 8) as u8,       // second nibble
            y: ((combined & 0x00F0) >> 4) as u8,       // third nibble
            n: (combined & 0x000F) as u8,              // fourth nibble
            byte: lsb,                                 // second byte of instruction
            addr: (combined & 0x0FFF), // 16 bit address (really 12 bit, but whatever)
        }
    }
}

impl Emulator {
    // implement all of the instruction code here, to keep main less cluttered
    pub fn cycle(&mut self) -> (bool, String) {
        let mut summary = String::new();
        let mut redraw = false;

        let instruction = Instruction::from(
            self.ram.get(self.program_counter),
            self.ram.get(self.program_counter + 1),
        );
        self.program_counter += 2;

        let x_reg = instruction.x;
        let y_reg = instruction.y;
        let x = self.registers.get(x_reg);
        let y = self.registers.get(y_reg);
        let n = instruction.n;
        let byte = instruction.byte;
        let addr = instruction.addr;

        summary += format!("{:#6X} => ", instruction.full).as_str();

        match instruction.opcode {
            0x0 => {
                match byte {
                    0xE0 => {
                        // CLS
                        for mut row in self.display {
                            row.fill(false);
                        }
                        summary += "CLS";
                    }
                    0xEE => {
                        // RET
                        self.program_counter = self.call_stack.pop().unwrap();
                        summary += format!("RET {:#5X}", self.program_counter).as_str();
                    }
                    _ => {
                        summary += "???";
                    }
                }
            }
            0x1 => {
                // JMP addr
                self.program_counter = addr;
                summary += format!("JMP {:#5X}", self.program_counter).as_str();
            }
            0x2 => {
                // CALL addr
                self.call_stack.push(self.program_counter);
                self.program_counter = addr;
                summary += format!("CALL {:#5X}", self.program_counter).as_str();
            }
            0x3 => {
                // SKIPIF RX == byte
                if x == byte {
                    self.program_counter += 2;
                }
                summary += format!("SKIPIF R{:1X} == {:#4X}", x_reg, byte).as_str();
            }
            0x4 => {
                // SKIPIF RX != byte
                if x != byte {
                    self.program_counter += 2
                }
                summary += format!("SKIPIF R{:1X} != {:#4X}", x_reg, byte).as_str();
            }
            0x5 => {
                // SKIPIF RX == RY
                if x == y {
                    self.program_counter += 2
                }
                summary += format!("SKIPIF R{:1X} == R{:1X}", x_reg, y_reg).as_str();
            }
            0x6 => {
                // LOAD byte => RX
                self.registers.set(x_reg, byte);
                summary += format!("IMM {:#4X} => R{:1X}", byte, x_reg).as_str();
            }
            0x7 => {
                // IMM ADD RX + byte => RX
                let (result, _overflow) = x.overflowing_add(byte);
                self.registers.set(x_reg, result);
                summary +=
                    format!("IMM ADD R{:1X} + {:#4X} => R{:1X}", x_reg, byte, x_reg).as_str();
            }
            0x8 => {
                // ALU stuff
                match n {
                    0x0 => {
                        // COPY RY => RX
                        self.registers.set(x_reg, y);
                        summary += format!("COPY R{:1X} => R{:1X}", y_reg, x_reg).as_str();
                    }
                    0x1 => {
                        // OR RX | RY => RX
                        self.registers.set(x_reg, x | y);
                        summary +=
                            format!("OR R{:1X} | R{:1X} => R{:1X}", x_reg, y_reg, x_reg).as_str();
                    }
                    0x2 => {
                        // AND RX & RY => RX
                        self.registers.set(x_reg, x & y);
                        summary +=
                            format!("AND R{:1X} & R{:1X} => R{:1X}", x_reg, y_reg, x_reg).as_str();
                    }
                    0x3 => {
                        // XOR RX ^ RY => RX
                        self.registers.set(x_reg, x ^ y);
                        summary +=
                            format!("XOR R{:1X} ^ R{:1X} => R{:1X}", x_reg, y_reg, x_reg).as_str();
                    }
                    0x4 => {
                        // ADD RX + RY => RX (sets overflow flag)
                        let (result, overflow) = x.overflowing_add(y);
                        self.registers.set(0xF, u8::from(overflow));
                        self.registers.set(x_reg, result);
                        summary +=
                            format!("ADD R{:1X} + R{:1X} => R{:1X}", x_reg, y_reg, x_reg).as_str();
                    }
                    0x5 => {
                        // SUB RX - RY => RX (sets !overflow flag)
                        let (result, overflow) = x.overflowing_sub(y);
                        self.registers.set(0xF, u8::from(!overflow));
                        self.registers.set(x_reg, result);
                        summary +=
                            format!("SUB R{:1X} - R{:1X} => R{:1X}", x_reg, y_reg, x_reg).as_str();
                    }
                    0x6 => {
                        // SHR RX >> 1 => RX (sets overflow flag)
                        self.registers.set(0xF, x & 1);
                        self.registers.set(x_reg, x >> 1);
                        summary += format!("SHR R{:1X} >> 1 => R{:1X}", x_reg, x_reg).as_str();
                    }
                    0x7 => {
                        // SUB RY - RX => RX (sets !overflow flag)
                        let (result, overflow) = y.overflowing_sub(x);
                        self.registers.set(0xF, u8::from(!overflow));
                        self.registers.set(x_reg, result);
                        summary +=
                            format!("SUB R{:1X} - R{:1X} => R{:1X}", y_reg, x_reg, x_reg).as_str();
                    }
                    0xE => {
                        // SHL RX << 1 => RX (sets overflow flag)
                        self.registers.set(0xF, (x & (1 << 7)) >> 7);
                        self.registers.set(x_reg, x << 1);
                        summary += format!("SHL R{:1X} << 1 => R{:1X}", x_reg, x_reg).as_str();
                    }
                    _ => {
                        summary += "???";
                    }
                }
            }
            0x9 => {
                // SKIPIF RX != RY
                if x != y {
                    self.program_counter += 2
                }
                summary += format!("SKIPIF R{:1X} != R{:1X}", x_reg, y_reg).as_str();
            }
            0xA => {
                // stack pointer = addr
                self.stack_pointer = addr;
                summary += format!("Set stack pointer to {:#5X}", addr).as_str();
            }
            0xB => {
                // jump to addr + R0
                self.stack_pointer = addr + self.registers.get(0) as u16;
                summary += format!("JMPP {:#5X} + R0", addr).as_str();
            }
            0xC => {
                // RAND & byte => RX
                let random: u8 = rand::random();
                self.registers.set(x_reg, random & byte);
                summary += format!("RAND & {:#4X} => R{:1X}", byte, x_reg).as_str();
            }
            0xD => {
                // DRAW
                redraw = true;
                summary += format!("DRAW {} bytes @ ({}, {})", n, x, y).as_str();
                let mut bytes = Vec::new();
                let mut collision: u8 = 0;
                for i in 0..n {
                    bytes.push(self.ram.get(self.stack_pointer + i as u16));
                }
                for (y_off, byte) in bytes.iter().enumerate().take(n as usize) {
                    let bools = byte_to_bools(*byte);
                    for (x_off, bit) in bools.iter().enumerate().take(8) {
                        let x_pos = (x as usize + x_off) % 64;
                        let y_pos = (y as usize + y_off) % 32;
                        if *bit {
                            if self.display[x_pos][y_pos] {
                                collision = 1;
                                self.display[x_pos][y_pos] = false;
                            } else {
                                self.display[x_pos][y_pos] = true;
                            }
                        }
                    }
                }
                self.registers.set(0xF, collision);
            }
            0xE => {
                match byte {
                    0x9E => {
                        // SKIPIF KEY == RX
                        if self.keys[x as usize] {
                            self.program_counter += 2;
                        }
                        summary += format!("SKIPIF KEY == R{:1X}", x_reg).as_str();
                    }
                    0xA1 => {
                        // SKIPIF KEY != RX
                        if !self.keys[x as usize] {
                            self.program_counter += 2;
                        }
                        summary += format!("SKIPIF KEY != R{:1X}", x_reg).as_str();
                    }
                    _ => {
                        summary += "???";
                    }
                }
            }
            0xF => {
                match byte {
                    0x07 => {
                        // TIMER => RX
                        self.registers.set(x_reg, self.timer);
                        summary += format!("TIMER => R{:1X}", x_reg).as_str();
                    }
                    0x0A => {
                        // KEYBLOCK => RX
                        self.key_block = x_reg;
                        summary += format!("KEYBLOCK => R{:1X}", x_reg).as_str();
                    }
                    0x15 => {
                        // RX => TIMER
                        self.timer = x;
                        summary += format!("R{:1X} => TIMER", x_reg).as_str();
                    }
                    0x18 => {
                        // RX => SOUND
                        self.sound_timer = x;
                        summary += format!("R{:1X} => SOUND", x_reg).as_str();
                    }
                    0x1E => {
                        // STKP += RX
                        self.stack_pointer += x as u16;
                        summary += format!("STKP += R{:1X}", x_reg).as_str();
                    }
                    0x29 => {
                        // STKP = DGT(RX)
                        self.stack_pointer = (x as u16) * 5;
                        summary += format!("STKP = DGT(R{:1X})", x_reg).as_str();
                    }
                    0x33 => {
                        // STORE DEC(RX)
                        self.ram.set(self.stack_pointer, x / 100);
                        self.ram.set(self.stack_pointer + 1, (x / 10) % 10);
                        self.ram.set(self.stack_pointer + 2, x % 10);
                        summary += format!("STORE DEC(R{:1X})", x_reg).as_str();
                    }
                    0x55 => {
                        // STORE R0..RX
                        for i in 0..=x_reg as usize {
                            self.ram
                                .set(self.stack_pointer + i as u16, self.registers.get(i as u8));
                        }
                        summary += format!("STORE R0..R{:1X}", x_reg).as_str();
                    }
                    0x65 => {
                        // LOAD R0..RX
                        for i in 0..=x_reg as usize {
                            self.registers
                                .set(i as u8, self.ram.get(self.stack_pointer + i as u16));
                        }
                        summary += format!("STORE R0..R{:1X}", x_reg).as_str();
                    }
                    _ => {
                        summary += "???";
                    }
                }
            }
            _ => {
                summary += "???";
            }
        }

        (redraw, summary)
    }
}

fn byte_to_bools(byte: u8) -> [bool; 8] {
    let mut bools = [false; 8];
    for (i, value) in bools.iter_mut().enumerate() {
        *value = (byte & (1 << (7 - i))) >> (7 - i) == 1;
    }

    bools
}
