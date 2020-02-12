use super::VideoMem;
use super::sprite::Sprite;
use super::super::types::Colour;

const TILE_MAP_WIDTH: usize = 32;
const SCREEN_WIDTH: usize = 160;

impl VideoMem {
    pub fn draw_line_gb(&mut self, target: &mut [u8]) {    // TODO: use external type here.
        let target_start = (self.lcdc_y as usize) * SCREEN_WIDTH;

        // Rebuild caches
        if self.map_cache_0_dirty {
            self.construct_map_cache_0();
        }
        if self.map_cache_1_dirty {
            self.construct_map_cache_1();
        }

        // Find objects
        let objects = self.ref_objects_for_line(self.lcdc_y);

        for (x, i) in target.chunks_mut(4).skip(target_start).take(SCREEN_WIDTH).enumerate() {
            // Is there an object here?
            match self.sprite_pixel(&objects, x as u8, self.lcdc_y) {
                SpritePixel::Hi(c) => write_pixel(i, c),
                SpritePixel::Lo(c) => {
                    if let Some(px) = self.window_pixel(x as u8, self.lcdc_y) {
                        match px {
                            BGPixel::Zero(_) => write_pixel(i, c),
                            BGPixel::NonZero(win) => write_pixel(i, win),
                        }
                    } else {
                        match self.background_pixel(x as u8, self.lcdc_y) {
                            BGPixel::Zero(_) => write_pixel(i, c),
                            BGPixel::NonZero(bg) => write_pixel(i, bg),
                        }
                    }
                },
                SpritePixel::None => {
                    if let Some(px) = self.window_pixel(x as u8, self.lcdc_y) {
                        match px {
                            BGPixel::Zero(win) => write_pixel(i, win),
                            BGPixel::NonZero(win) => write_pixel(i, win),
                        }
                    } else {
                        match self.background_pixel(x as u8, self.lcdc_y) {
                            BGPixel::Zero(bg) => write_pixel(i, bg),
                            BGPixel::NonZero(bg) => write_pixel(i, bg),
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn sprite_pixel(&self, objects: &Option<Vec<&Sprite>>, x: u8, y: u8) -> SpritePixel {
        if let Some(obj) = objects {
            // TODO: this lil calc outside
            let hi_x = x + 8;
            let hi_y = y + 8;//if self.is_large_sprites() {16} else {8};    // TODO: large sprites
            for o in obj.iter() {
                let x_offset = hi_x.wrapping_sub(o.x);
                if x_offset < 8 {
                    let y_offset = hi_y.wrapping_sub(o.y);
                    let tile = self.ref_tile(o.tile_num as usize);  // TODO adjust tile num based on y val
                    let texel = tile.get_texel(x_offset as usize, y_offset as usize);
                    return if texel == 0 {
                        SpritePixel::None
                    } else {
                        let pixel = if o.palette_0() {self.get_obj_0_colour(texel)} else {self.get_obj_1_colour(texel)};
                        if o.is_above_bg() {
                            SpritePixel::Hi(pixel)
                        } else {
                            SpritePixel::Lo(pixel)
                        }
                    }
                }
            }
            SpritePixel::None
        } else {
            SpritePixel::None
        }
    }

    #[inline]
    fn window_pixel(&self, x: u8, y: u8) -> Option<BGPixel> {
        if self.get_window_enable() && (x >= self.window_x) && (y >= self.window_y) {
            let win_x = (x - self.window_x) as usize;
            let win_y = (y - self.window_y) as usize;
            let win_texel = self.ref_window()[win_y][win_x];
            Some(if win_texel == 0 {
                BGPixel::Zero(self.get_bg_colour(win_texel))
            } else {
                BGPixel::NonZero(self.get_bg_colour(win_texel))
            })
        } else {
            None
        }
    }

    #[inline]
    fn background_pixel(&self, x: u8, y: u8) -> BGPixel {
        if self.get_background_priority() {
            let bg_x = self.scroll_x.wrapping_add(x) as usize;
            let bg_y = self.scroll_y.wrapping_add(y) as usize;
            let bg_texel = self.ref_background()[bg_y][bg_x];
            if bg_texel == 0 {
                BGPixel::Zero(self.get_bg_colour(bg_texel))
            } else {
                BGPixel::NonZero(self.get_bg_colour(bg_texel))
            }
        } else {
            BGPixel::Zero(Colour::zero())
        }
    }
}

impl VideoMem {
    fn construct_map_cache_0(&mut self) {
        for (i, tile_num) in self.tile_map_0.iter().enumerate() {
            // TODO: iterate over tile
            let base_y = i / 32;
            let base_x = i % 32;
            for y in 0..8 {
                for x in 0..8 {
                    // TODO: attrs
                    self.map_cache_0[base_y][base_x] = self.tile_mem.ref_tile(*tile_num as usize).get_texel(x, y);
                }
            }
        }
    }

    fn construct_map_cache_1(&mut self) {
        for (i, tile_num) in self.tile_map_1.iter().enumerate() {
            // TODO: iterate over tile
            let base_y = i / 32;
            let base_x = i % 32;
            for y in 0..8 {
                for x in 0..8 {
                    // TODO: attrs
                    self.map_cache_1[base_y][base_x] = self.tile_mem.ref_tile(*tile_num as usize).get_texel(x, y);
                }
            }
        }
    }
}

enum SpritePixel {
    Hi(Colour), // High priority
    Lo(Colour), // Low priority
    None
}

enum BGPixel {
    NonZero(Colour),    // Colour 1-3
    Zero(Colour),       // Zero colour (draw LO sprites above this)
}

#[inline]
fn write_pixel(output: &mut [u8], colour: Colour) {
    output[0] = colour.r;
    output[1] = colour.g;
    output[2] = colour.b;
    output[3] = 255;    // TODO: does this need to be written?
}