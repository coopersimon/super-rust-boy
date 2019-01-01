//

mod mb1;
mod mb3;

use self::mb1::MB1;
use self::mb3::MB3;

use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::fs::File;

use super::MemDevice;

// Cartridge Memory Bank type
enum MBC {
    _0,
    _1(MB1),
    _2,
    _3(MB3),
    _4(u8),
    _5(u8),
}

// Swap Bank instructions
enum Swap {
    ROM(u8),
    RAM(u8),
    Both(u8, u8),
    None
}

pub struct Cartridge {
    rom_bank_0: [u8; 0x4000],
    rom_bank_n: [u8; 0x4000],
    ram:        Vec<u8>,

    rom_file:   BufReader<File>,
    mem_bank:   MBC,
    ram_enable: bool,
    ram_offset: usize,
    battery:    bool,
}

impl MemDevice for Cartridge {
    fn read(&self, loc: u16) -> u8 {
        match loc {
            0x0...0x3FFF    => self.rom_bank_0[loc as usize],
            0x4000...0x7FFF => self.rom_bank_n[(loc - 0x4000) as usize],
            _ => self.read_ram(loc - 0xA000),
        }
    }

    fn write(&mut self, loc: u16, val: u8) {
        if (loc >= 0xA000) && (loc < 0xC000) {
            self.write_ram(loc - 0xA000, val);
        } else {
            let swap_instr = match self.mem_bank {
                MBC::_1(ref mut mb) => {
                    let old_rom_bank = mb.get_rom_bank();
                    let old_ram_bank = mb.get_ram_bank();
                    match loc {
                        0x0000...0x1FFF => self.ram_enable = (val & 0xA) == 0xA,
                        0x2000...0x3FFF => mb.set_lower(val),
                        0x4000...0x5FFF => mb.set_upper(val),
                        _ => mb.mem_type_select(val),
                    }

                    let new_rom_bank = mb.get_rom_bank();
                    let new_ram_bank = mb.get_ram_bank();
                    let diff_rom_bank = new_rom_bank != old_rom_bank;
                    let diff_ram_bank = new_ram_bank != old_ram_bank;

                    if diff_rom_bank && diff_ram_bank {
                        Swap::Both(new_rom_bank, new_ram_bank)
                    } else if diff_rom_bank {
                        Swap::ROM(new_rom_bank)
                    } else if diff_ram_bank {
                        Swap::RAM(new_ram_bank)
                    } else {
                        Swap::None
                    }
                },
                MBC::_2 => match loc {
                    0x0000...0x1FFF => {self.ram_enable = (loc & 0x10) == 0; Swap::None},
                    0x2000...0x3FFF => Swap::ROM(val & 0xF), // If loc & 0x10 == 0x10
                    _ => Swap::None,
                },
                MBC::_3(ref mut mb) => match (loc, val) {
                    (0x0000...0x1FFF, x)            => {self.ram_enable = (x & 0xF) == 0xA; Swap::None},
                    (0x2000...0x3FFF, 0)            => Swap::ROM(1),
                    (0x2000...0x3FFF, x)            => Swap::ROM(x),
                    (0x4000...0x5FFF, x @ 0...3)    => Swap::RAM(x),
                    (0x4000...0x5FFF, x @ 8...0xC)  => {mb.select_rtc(x); Swap::None},
                    (0x6000...0x7FFF, 1)            => {mb.latch_clock(); Swap::None},
                    _ => Swap::None,
                },
                _ => Swap::None,
            };

            match swap_instr {
                Swap::Both(rom,ram) => {
                    self.swap_rom_bank(rom);
                    self.swap_ram_bank(ram);
                },
                Swap::ROM(rom) => self.swap_rom_bank(rom),
                Swap::RAM(ram) => self.swap_ram_bank(ram),
                Swap::None => {},
            }
        }
    }
}

impl Cartridge {
    pub fn new(rom_file: &str) -> Result<Cartridge, String> {
        let f = try!(File::open(rom_file).map_err(|e| e.to_string()));

        let mut reader = BufReader::new(f);
        let mut buf = [0_u8; 0x4000];
        //try!(reader.read_exact(&mut buf).map_err(|e| e.to_string()));
        try!(reader.read(&mut buf).map_err(|e| e.to_string()));

        let bank_type = match buf[0x147] {
            0x1...0x3   => MBC::_1(MB1::new()),
            0x5...0x6   => MBC::_2,
            0xF...0x13  => MBC::_3(MB3::new()),
            0x15...0x17 => MBC::_4(0),
            0x19...0x1E => MBC::_5(0),
            _           => MBC::_0,
        };

        let ram_size = match (&bank_type, buf[0x149]) {
            (MBC::_2,_) => 0x200,
            (_,0x1)     => 0x800,
            (_,0x2)     => 0x2000,
            (_,0x3)     => 0x8000,
            _           => 0,
        };

        let mut ret = Cartridge {
            rom_bank_0: buf,
            rom_bank_n: [0; 0x4000],
            ram:        vec!(0; ram_size),
            rom_file:   reader,
            mem_bank:   bank_type,
            ram_enable: false,
            ram_offset: 0,
            battery:    false,
        };

        ret.swap_rom_bank(1);

        Ok(ret)
    }

    pub fn swap_rom_bank(&mut self, bank: u8)/* -> Result<(), String>*/ {
        //println!("Swapping in bank: {}", bank);
        let pos = (bank as u64) * 0x4000;
        match self.rom_file.seek(SeekFrom::Start(pos)) {
            Ok(_) => {},
            Err(s) => panic!("Couldn't swap in bank: {}", s),
        }
        //try!(self.rom_file.read_exact(&mut self.rom_bank_n).map_err(|e| e.to_string()));
        match self.rom_file.read(&mut self.rom_bank_n) {
            Ok(_) => {},
            Err(s) => panic!("Couldn't swap in bank: {}", s),
        }
    }

    #[inline]
    pub fn swap_ram_bank(&mut self, bank: u8) {
        self.ram_offset = (bank as usize) * 0x2000;
    }

    #[inline]
    pub fn read_ram(&self, loc: u16) -> u8 {
        if self.ram_enable {
            match self.mem_bank {
                MBC::_3(ref mb) => if mb.ram_select {self.ram[self.ram_offset + (loc as usize)]}
                                   else {mb.get_rtc_reg()},
                _ => self.ram[self.ram_offset + (loc as usize)],
            }
        }
        else {
            0
        }
    }

    #[inline]
    pub fn write_ram(&mut self, loc: u16, val: u8) {
        if self.ram_enable {
            match self.mem_bank {
                MBC::_2             => self.ram[self.ram_offset + (loc as usize)] = val & 0xF,
                MBC::_3(ref mut mb) => if mb.ram_select {self.ram[self.ram_offset + (loc as usize)] = val}
                                       else {mb.set_rtc_reg(val)},
                _ => self.ram[self.ram_offset + (loc as usize)] = val,
            }
        }
    }
}
