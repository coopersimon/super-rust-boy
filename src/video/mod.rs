mod palette;
mod shaders;

use glium;
use glium::{DisplayBuild, Surface};
use glium::texture::texture2d::Texture2d;
use glium::backend::glutin_backend::GlutinFacade;
use self::palette::{BWPalette, Palette};
use mem::MemDevice;

const BG_X: u8 = 255;
const BG_Y: u8 = 255;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    texcoord: [f32; 2],
}

implement_vertex!(Vertex, position, texcoord);

fn byte_to_float(byte: u8, scale: u8) -> f32 {
    let (byte_f, scale_f) = (byte as f32, scale as f32);
    let out_f = (byte_f * 2.0) / scale_f;
    out_f - 1.0
}

pub trait VideoDevice: MemDevice {
    fn render_frame(&mut self);
}

pub struct GBVideo {
    // potentially add background, sprite, window objects?
    display_enable: bool,
    window_offset: usize,
    window_enable: bool,
    bg_offset: usize,
    bg_enable: bool,
    tile_data_select: bool,
    sprite_size: bool,
    sprite_enable: bool,

    scroll_y: u8,
    scroll_x: u8,
    lcdc_y: u8,
    ly_compare: u8,
    window_y: u8,
    window_x: u8,
    bg_palette: BWPalette,
    obj_palette_0: BWPalette,
    obj_palette_1: BWPalette,
    
    // raw tiles used for background & sprites
    raw_tile_mem: Vec<u8>,
    // map for background & window
    tile_map_mem: Vec<u8>,
    sprite_mem: Vec<u8>,

    display: GlutinFacade,
    program: glium::Program,
}

impl MemDevice for GBVideo {
    fn read(&self, loc: u16) -> u8 {
        match loc {
            0x8000...0x97FF => self.raw_tile_mem[(loc - 0x8000) as usize],
            0x9800...0x9FFF => self.tile_map_mem[(loc - 0x9800) as usize],
            0xFE00...0xFE9F => self.sprite_mem[(loc - 0xFE00) as usize],
            0xFF40 => self.lcd_control_read(),
            0xFF41 => self.lcd_status_read(),
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.lcdc_y,
            0xFF45 => self.ly_compare,
            0xFF47 => self.bg_palette.read(),
            0xFF48 => self.obj_palette_0.read(),
            0xFF49 => self.obj_palette_1.read(),
            0xFF4A => self.window_y,
            0xFF4B => self.window_x,
            _ => 0,
        }
    }

    fn write(&mut self, loc: u16, val: u8) {
        match loc {
            0x8000...0x97FF => self.raw_tile_mem[(loc - 0x8000) as usize] = val,
            0x9800...0x9FFF => self.tile_map_mem[(loc - 0x9800) as usize] = val,
            0xFE00...0xFE9F => self.sprite_mem[(loc - 0xFE00) as usize] = val,
            0xFF40 => self.lcd_control_write(val),
            0xFF41 => self.lcd_status_write(val),
            0xFF42 => self.scroll_y = val,
            0xFF43 => self.scroll_x = val,
            0xFF44 => self.lcdc_y = val,
            0xFF45 => self.ly_compare = val,
            0xFF47 => self.bg_palette.write(val),
            0xFF48 => self.obj_palette_0.write(val),
            0xFF49 => self.obj_palette_1.write(val),
            0xFF4A => self.window_y = val,
            0xFF4B => self.window_x = val,
            _ => return,
        }
    }
}

impl VideoDevice for GBVideo {
    // Drawing for a single frame
    fn render_frame(&mut self) {
        let mut target = self.display.draw();
        target.clear_color(1.0, 1.0, 1.0, 1.0);

        // render background
        if self.bg_enable {
            println!("frame");
            for x in 0..32 {
                for y in 0..32 {
                    // get tile number from background map
                    let offset = (x + (y*32)) as usize;
                    let tile = self.tile_map_mem[self.bg_offset + offset];
                    // get tile location from number & addressing mode
                    let tile_loc = if self.tile_data_select {
                        (tile as isize) * 16
                    } else {
                        0x1000 + ((tile as i8) as isize * 16)
                    } as usize;

                    let tex = {
                        let raw_tex = &self.raw_tile_mem[tile_loc..(tile_loc*16)];
                        self.bg_palette.make_texture(&raw_tex, &self.display)
                    };
                    self.draw_square(&mut target, x*8, y*8, tex);
                };
            };
        };

        // render sprites
        // render window
        target.finish().unwrap();
    }
}

// Control functions
impl GBVideo {
    pub fn new() -> GBVideo {
        // create window
        let display = glium::glutin::WindowBuilder::new().build_glium().unwrap();
        // compile program
        let program = glium::Program::from_source(&display, shaders::VERTEX_SRC,
                                                   shaders::FRAGMENT_SRC, None).unwrap();

        GBVideo {
            display_enable: true,
            window_offset: 0x9800,
            window_enable: false,
            bg_offset: 0x9800,
            bg_enable: false,
            tile_data_select: false,
            sprite_size: false,
            sprite_enable: false,

            scroll_y: 0,
            scroll_x: 0,
            lcdc_y: 0,
            ly_compare: 0,
            window_y: 0,
            window_x: 0,
            bg_palette: BWPalette::new(),
            obj_palette_0: BWPalette::new(),
            obj_palette_1: BWPalette::new(),

            raw_tile_mem: vec![0; 0x1800],
            tile_map_mem: vec![0; 0x800],
            sprite_mem: vec![0; 0x100],

            display: display,
            program: program,
        }
    }




    fn lcd_control_write(&mut self, val: u8) {
        self.display_enable = if val & 0x80 == 0x80 {true} else {false};
        self.window_offset = if val & 0x40 == 0x40 {0x400} else {0x0};
        self.window_enable = if val & 0x20 == 0x20 {true} else {false};
        self.tile_data_select = if val & 0x10 == 0x10 {true} else {false};
        self.bg_offset = if val & 0x8 == 0x8 {0x400} else {0x0};
        self.sprite_size = if val & 0x4 == 0x4 {true} else {false};
        self.sprite_enable = if val & 0x2 == 0x2 {true} else {false};
        self.bg_enable = if val & 0x1 == 0x1 {true} else {false};
    }

    fn lcd_control_read(&self) -> u8 {
        let val_7 = if self.display_enable {0x80} else {0};
        let val_6 = if self.window_offset == 0x400 {0x40} else {0};
        let val_5 = if self.window_enable {0x20} else {0};
        let val_4 = if self.tile_data_select {0x10} else {0};
        let val_3 = if self.bg_offset == 0x400 {0x8} else {0};
        let val_2 = if self.sprite_size {0x4} else {0};
        let val_1 = if self.sprite_enable {0x2} else {0};
        let val_0 = if self.bg_enable {0x1} else {0};
        val_7 | val_6 | val_5 | val_4 | val_3 | val_2 | val_1 | val_0
    }

    fn lcd_status_write(&mut self, val: u8) {
    }

    fn lcd_status_read(&self) -> u8 {
        0
    }    
}


// Internal graphics functions
impl GBVideo {

    // draw 8x8 textured square
    fn draw_square(&mut self, mut target: &mut glium::Frame, x: u8, y: u8, texture: Texture2d) {
        use glium::index::{NoIndices, PrimitiveType};

        let (x_a, y_a) = (byte_to_float(x, BG_X), byte_to_float(y, BG_Y));
        let (x_b, y_b) = (byte_to_float(x + 8, BG_X), byte_to_float(y + 8, BG_Y));

        let uniforms = uniform!{tex: &texture};

        let tile = vec![
            Vertex { position: [x_a, y_a], texcoord: [0.0, 0.0] },
            Vertex { position: [x_b, y_a], texcoord: [1.0, 0.0] },
            Vertex { position: [x_a, y_b], texcoord: [0.0, 1.0] },
            Vertex { position: [x_b, y_b], texcoord: [1.0, 1.0] }
        ];
        let vertex_buffer = glium::VertexBuffer::new(&self.display, &tile).unwrap();

        target.draw(&vertex_buffer, NoIndices(PrimitiveType::TriangleStrip),
                    &self.program, &uniforms, &Default::default()).unwrap();
    }
}
