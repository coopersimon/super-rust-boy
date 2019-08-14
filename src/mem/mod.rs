// mem.rs module: Memory bus and devices

mod cartridge;

use crate::video::VideoDevice;
use crate::audio::AudioDevice;
use crate::timer::Timer;
use cartridge::Cartridge;

use bitflags::bitflags;


bitflags! {
    #[derive(Default)]
    pub struct InterruptFlags: u8 {
        const V_BLANK  = 1 << 0;
        const LCD_STAT = 1 << 1;
        const TIMER    = 1 << 2;
        const SERIAL   = 1 << 3;
        const JOYPAD   = 1 << 4;
    }
}

pub trait MemDevice {
    fn read(&self, loc: u16) -> u8;
    fn write(&mut self, loc: u16, val: u8);
}

pub struct MemBus {
    cart:               Cartridge,

    ram_bank:           WriteableMem,
    ram:                WriteableMem,
    high_ram:           WriteableMem,

    interrupt_flag:     InterruptFlags,
    interrupt_enable:   InterruptFlags,

    video_device:       VideoDevice,

    audio_device:       AudioDevice,

    timer:              Timer,
}

impl MemBus {
    pub fn new(rom_file: &str, video_device: VideoDevice, audio_device: AudioDevice) -> MemBus {
        let rom = match Cartridge::new(rom_file) {
            Ok(r) => r,
            Err(s) => panic!("Could not construct ROM: {}", s),
        };

        MemBus {
            cart:               rom,
            ram_bank:           WriteableMem::new(0x2000),
            ram:                WriteableMem::new(0x2000),
            high_ram:           WriteableMem::new(0x7F),
            interrupt_flag:     InterruptFlags::default(),
            interrupt_enable:   InterruptFlags::default(),
            video_device:       video_device,
            audio_device:       audio_device,
            timer:              Timer::new(),
        }
    }

    pub fn render_frame(&mut self) {
        self.audio_device.frame_update();
        self.video_device.render_frame();
    }

    pub fn update_timers(&mut self, clock_count: u32) {
        self.audio_device.send_update(clock_count);
        if self.timer.update_timers(clock_count) {
            self.interrupt_flag.insert(InterruptFlags::TIMER);
        }
    }

    // Set the current video mode based on the cycle count.
    pub fn video_mode(&mut self, cycle_count: &mut u32) -> bool {
        let (ret, int) = self.video_device.video_mode(cycle_count);
        self.interrupt_flag.insert(int);
        ret
    }

    // Gets any interrupts that have been triggered and are enabled.
    pub fn get_interrupts(&self) -> InterruptFlags {
        self.interrupt_flag & self.interrupt_enable
    }

    // Clears an interrupt flag.
    pub fn clear_interrupt_flag(&mut self, flag: InterruptFlags) {
        self.interrupt_flag.remove(flag);
    }

    pub fn read_inputs(&mut self) {
        self.video_device.read_inputs();
    }

    fn dma(&mut self, val: u8) {
        let hi_byte = (val as u16) << 8;
        for lo_byte in 0_u16..=0x9F_u16 {
            let src_addr = hi_byte | lo_byte;
            let dest_addr = 0xFE00 | lo_byte;
            let byte = self.read(src_addr);
            self.video_device.write(dest_addr, byte);
        }
    }
}

impl MemDevice for MemBus {
    fn read(&self, loc: u16) -> u8 {
        match loc {
            0x0000...0x7FFF => self.cart.read(loc),
            0x8000...0x9FFF => self.video_device.read(loc),
            0xA000...0xBFFF => self.cart.read(loc),
            0xC000...0xDFFF => self.ram.read(loc - 0xC000),
            0xE000...0xFDFF => self.ram.read(loc - 0xE000),
            0xFE00...0xFE9F => self.video_device.read(loc),
            0xFF00          => self.video_device.read(loc),
            0xFF04...0xFF07 => self.timer.read(loc),
            0xFF0F          => self.interrupt_flag.bits(),
            0xFF10...0xFF3F => self.audio_device.read(loc),
            0xFF40...0xFF4B => self.video_device.read(loc),
            0xFF80...0xFFFE => self.high_ram.read(loc - 0xFF80),
            0xFFFF          => self.interrupt_enable.bits(),
            _ => self.ram.read(0),
        }
    }

    fn write(&mut self, loc: u16, val: u8) {
        match loc {
            0x0000...0x7FFF => self.cart.write(loc, val),
            0x8000...0x9FFF => self.video_device.write(loc, val),
            0xA000...0xBFFF => self.cart.write(loc, val),
            0xC000...0xDFFF => self.ram.write(loc - 0xC000, val),
            0xE000...0xFDFF => self.ram.write(loc - 0xE000, val),
            0xFE00...0xFE9F => self.video_device.write(loc, val),
            0xFF00          => self.video_device.write(loc, val),
            0xFF04...0xFF07 => self.timer.write(loc, val),    
            0xFF0F          => self.interrupt_flag = InterruptFlags::from_bits_truncate(val),
            0xFF10...0xFF3F => self.audio_device.write(loc, val),
            0xFF40...0xFF45 => self.video_device.write(loc, val), 
            0xFF46          => self.dma(val),
            0xFF47...0xFF4B => self.video_device.write(loc, val),
            0xFF80...0xFFFE => self.high_ram.write(loc - 0xFF80, val),
            0xFFFF          => self.interrupt_enable = InterruptFlags::from_bits_truncate(val),
            _ => {},
        }
    }
}

struct WriteableMem {
    mem: Vec<u8>,
}

impl WriteableMem {
    fn new(size: usize) -> WriteableMem {
        WriteableMem {mem: vec![0;size]}
    }
}

impl MemDevice for WriteableMem {
    fn read(&self, loc: u16) -> u8 {
        self.mem[loc as usize]
    }

    fn write(&mut self, loc: u16, val: u8) {
        self.mem[loc as usize] = val;
    }
}
