mod palette;
mod shaders;
mod joypad;
mod texcache;

use glium;
use glium::{Display, Surface};
use glium::glutin::EventsLoop;

use mem::MemDevice;

use self::palette::{BWPalette, Palette};
use self::joypad::Joypad;
use self::texcache::{Hash, TexCache};

const BG_X: u16 = 256;
const BG_Y: u16 = 256;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    texcoord: [f32; 2],
}

implement_vertex!(Vertex, position, texcoord);

fn byte_to_float(byte: u16, scale: u16) -> f32 {
    let (byte_f, scale_f) = (byte as f32, scale as f32);
    let out_f = (byte_f * 2.0) / scale_f;
    out_f - 1.0
}

pub trait VideoDevice: MemDevice {
    fn render_frame(&mut self);
    fn read_inputs(&mut self);

    fn inc_lcdc_y(&mut self);
    fn set_lcdc_y(&mut self, val: u8);
}

pub struct GBVideo {
    // potentially add background, sprite, window objects?
    display_enable:     bool,
    window_offset:      usize,
    window_enable:      bool,
    bg_offset:          usize,
    bg_enable:          bool,
    tile_data_select:   bool,
    sprite_size:        bool,
    sprite_enable:      bool,

    lcd_status:         u8,
    scroll_y:           u8,
    scroll_x:           u8,
    lcdc_y:             u8,
    ly_compare:         u8,
    window_y:           u8,
    window_x:           u8,
    bg_palette:         BWPalette,
    obj_palette_0:      BWPalette,
    obj_palette_1:      BWPalette,

    // joypad inputs
    joypad:             Joypad,

    // raw tiles used for background & sprites
    raw_tile_mem:       Vec<u8>,
    // map for background & window
    tile_map_mem:       Vec<u8>,
    sprite_mem:         Vec<u8>,

    // cache for rendered textures
    tex_cache:          TexCache,

    // Glium graphics data
    display:            Display,
    events_loop:        EventsLoop,
    program:            glium::Program,
}

impl MemDevice for GBVideo {
    fn read(&self, loc: u16) -> u8 {
        match loc {
            0x8000...0x97FF =>  self.raw_tile_mem[(loc - 0x8000) as usize],
            0x9800...0x9FFF =>  self.tile_map_mem[(loc - 0x9800) as usize],
            0xFE00...0xFE9F =>  self.sprite_mem[(loc - 0xFE00) as usize],

            0xFF00 =>           self.joypad.read(),

            0xFF40 =>           self.lcd_control_read(),
            0xFF41 =>           self.lcd_status,
            0xFF42 =>           self.scroll_y,
            0xFF43 =>           self.scroll_x,
            0xFF44 =>           self.lcdc_y,
            0xFF45 =>           self.ly_compare,
            0xFF47 =>           self.bg_palette.read(),
            0xFF48 =>           self.obj_palette_0.read(),
            0xFF49 =>           self.obj_palette_1.read(),
            0xFF4A =>           self.window_y,
            0xFF4B =>           self.window_x,
            _ => 0,
        }
    }

    fn write(&mut self, loc: u16, val: u8) {
        match loc {
            0x8000...0x97FF =>  self.write_raw_tile(loc, val),
            0x9800...0x9FFF =>  self.tile_map_mem[(loc - 0x9800) as usize] = val,
            0xFE00...0xFE9F =>  self.sprite_mem[(loc - 0xFE00) as usize] = val,

            0xFF00 =>           self.joypad.write(val),

            0xFF40 =>           self.lcd_control_write(val),
            0xFF41 =>           self.lcd_status = val,
            0xFF42 =>           self.scroll_y = val,
            0xFF43 =>           self.scroll_x = val,
            0xFF44 =>           self.lcdc_y = 0,
            0xFF45 =>           self.ly_compare = val,
            0xFF47 =>           {self.bg_palette.write(val); self.tex_cache.clear_all()},
            0xFF48 =>           self.obj_palette_0.write(val),
            0xFF49 =>           self.obj_palette_1.write(val),
            0xFF4A =>           self.window_y = val,
            0xFF4B =>           self.window_x = val,
            _ => return,
        }
    }
}

impl VideoDevice for GBVideo {
    // Drawing for a single frame
    fn render_frame(&mut self) {
        let mut target = self.display.draw();

        if self.display_enable {
            target.clear_color(1.0, 1.0, 1.0, 1.0);

            // render background
            if self.bg_enable {
                let bg_offset = self.bg_offset;
                self.draw_tilespace(&mut target, bg_offset);
            }

            // render sprites
            if self.sprite_enable {
                /*for s in (0..self.sprite_mem.size()).step_by(4) {
                    let y_pos = self.sprite_mem[s] - 16;
                    let x_pos = self.sprite_mem[s+1] - 8;
                    let
                }*/
                //println!("sprites please");
            }

            // render window
            if self.window_enable { // && self.bg_enable
                let window_offset = self.window_offset;
                self.draw_tilespace(&mut target, window_offset);
            }
        } else {
            target.clear_color(0.0, 0.0, 0.0, 1.0);
        }

        target.finish().unwrap();
    }

    // Read inputs and store
    fn read_inputs(&mut self) {
        use glium::glutin::{Event, WindowEvent, ElementState, VirtualKeyCode};

        let joypad = &mut self.joypad;

        self.events_loop.poll_events(|e| {
            match e {
                Event::WindowEvent {
                    window_id: _,
                    event: w,
                } => match w {
                    WindowEvent::CloseRequested => {
                        ::std::process::exit(0);
                    },
                    WindowEvent::KeyboardInput {
                        device_id: _,
                        input: k,
                    } => {
                        let pressed = match k.state {
                            ElementState::Pressed => true,
                            ElementState::Released => false,
                        };
                        match k.virtual_keycode {
                            Some(VirtualKeyCode::Z)         => joypad.a = pressed,
                            Some(VirtualKeyCode::X)         => joypad.b = pressed,
                            Some(VirtualKeyCode::Space)     => joypad.select = pressed,
                            Some(VirtualKeyCode::Return)    => joypad.start = pressed,
                            Some(VirtualKeyCode::Up)        => joypad.up = pressed,
                            Some(VirtualKeyCode::Down)      => joypad.down = pressed,
                            Some(VirtualKeyCode::Left)      => joypad.left = pressed,
                            Some(VirtualKeyCode::Right)     => joypad.right = pressed,
                            _ => {},
                        }
                    },
                    _ => {},
                },
                _ => {},
            }
        });
    }

    fn inc_lcdc_y(&mut self) {
        self.lcdc_y += 1;
    }

    fn set_lcdc_y(&mut self, val: u8) {
        self.lcdc_y = val;
    }
}

// Control functions
impl GBVideo {
    pub fn new() -> GBVideo {
        let events_loop = glium::glutin::EventsLoop::new();

        // create display
        let window = glium::glutin::WindowBuilder::new()
            .with_dimensions(glium::glutin::dpi::LogicalSize::new(320.0, 288.0))
            .with_title("Super Rust Boy");
        let context = glium::glutin::ContextBuilder::new();
        let display = glium::Display::new(window, context, &events_loop).unwrap();

        // compile program
        let program = glium::Program::from_source(&display,
                                                  shaders::VERTEX_SRC,
                                                  shaders::FRAGMENT_SRC,
                                                  None).unwrap();

        GBVideo {
            display_enable:     true,
            window_offset:      0x0,
            window_enable:      false,
            tile_data_select:   true,
            bg_offset:          0x0,
            sprite_size:        false,
            sprite_enable:      false,
            bg_enable:          true,

            lcd_status:         0, // TODO: check
            scroll_y:           0,
            scroll_x:           0,
            lcdc_y:             0,
            ly_compare:         0,
            window_y:           0,
            window_x:           0,
            bg_palette:         BWPalette::new(),
            obj_palette_0:      BWPalette::new(),
            obj_palette_1:      BWPalette::new(),

            joypad:             Joypad::new(),

            raw_tile_mem:       vec![0; 0x1800],
            tile_map_mem:       vec![0; 0x800],
            sprite_mem:         vec![0; 0x100],

            tex_cache:          TexCache::new(),

            display:            display,
            events_loop:        events_loop,
            program:            program,
        }
    }

    fn lcd_control_write(&mut self, val: u8) {
        self.display_enable     = val & 0x80 == 0x80;
        self.window_offset      = if val & 0x40 == 0x40 {0x400} else {0x0};
        self.window_enable      = val & 0x20 == 0x20;
        self.tile_data_select   = val & 0x10 == 0x10;
        self.bg_offset          = if val & 0x8 == 0x8   {0x400} else {0x0};
        self.sprite_size        = val & 0x4 == 0x4;
        self.sprite_enable      = val & 0x2 == 0x2;
        self.bg_enable          = val & 0x1 == 0x1;
    }

    fn lcd_control_read(&self) -> u8 {
        let val_7 = if self.display_enable          {0x80} else {0};
        let val_6 = if self.window_offset == 0x400  {0x40} else {0};
        let val_5 = if self.window_enable           {0x20} else {0};
        let val_4 = if self.tile_data_select        {0x10} else {0};
        let val_3 = if self.bg_offset == 0x400      {0x8} else {0};
        let val_2 = if self.sprite_size             {0x4} else {0};
        let val_1 = if self.sprite_enable           {0x2} else {0};
        let val_0 = if self.bg_enable               {0x1} else {0};
        val_7 | val_6 | val_5 | val_4 | val_3 | val_2 | val_1 | val_0
    }

    #[inline]
    fn write_raw_tile(&mut self, loc: u16, val: u8) {
        let inner_loc = (loc - 0x8000) as usize;
        self.raw_tile_mem[inner_loc] = val;

        let tile_base = inner_loc - (inner_loc % 16);
        self.tex_cache.clear(tile_base, self.bg_palette.data);
    }
}


// Internal graphics functions
impl GBVideo {

    // draw background or window
    fn draw_tilespace(&mut self, target: &mut glium::Frame, map_offset: usize) {
        for y in 0..32 {
            for x in 0..32 {
                // get tile number from background map
                let offset = (x + (y*32)) as usize;
                let tile = self.tile_map_mem[map_offset + offset];

                // get tile location from number & addressing mode
                let tile_loc = if self.tile_data_select {
                    (tile as usize) * 16
                } else {
                    (0x1000 + ((tile as i8) as isize * 16)) as usize
                };

                // Get hash key for Texture.
                let tex_hash = {
                    let hash = TexCache::make_hash(tile_loc, self.bg_palette.data);
                    if !self.tex_cache.contains_key(&hash) {
                        let raw_tex = &self.raw_tile_mem[tile_loc..(tile_loc + 16)];
                        let tex = self.bg_palette.make_texture(&raw_tex, &self.display);
                        self.tex_cache.insert(hash.clone(), tex);
                    }
                    hash
                };
                self.draw_square(target, x*8, y*8, &tex_hash);
            }
        }
    }

    // draw 8x8 textured square
    fn draw_square(&mut self, target: &mut glium::Frame, x: u16, y: u16, hash: &Hash) {
        use glium::index::{NoIndices, PrimitiveType};

        let texture = self.tex_cache.get(hash).expect("Tex cache broken.");
        let (x_a, y_a) = (byte_to_float(x, BG_X), byte_to_float(y, BG_Y));
        let (x_b, y_b) = (byte_to_float(x + 8, BG_X), byte_to_float(y + 8, BG_Y));

        let uniforms = uniform!{tex: texture};

        let tile = vec![
            Vertex { position: [x_a, y_a], texcoord: [0.0, 0.0] },
            Vertex { position: [x_b, y_a], texcoord: [1.0, 0.0] },
            Vertex { position: [x_a, y_b], texcoord: [0.0, 1.0] },
            Vertex { position: [x_b, y_b], texcoord: [1.0, 1.0] }
        ];
        let vertex_buffer = glium::VertexBuffer::new(&self.display, &tile).unwrap();
        //println!("{},{}", x_a,y_a);

        target.draw(&vertex_buffer, NoIndices(PrimitiveType::TriangleStrip),
                    &self.program, &uniforms, &Default::default()).unwrap();
    }
}
