#[derive(Clone, Copy, Default)]
pub struct Obj {
    pub y: u8,
    pub x: u8,
    pub tile_id: u8,
    pub attr: u8,
}

pub struct Oam {
    pub objects: [Obj; 40],
}

impl Oam {
    pub fn new() -> Self {
        Self {
            objects: [Obj::default(); 40],
        }
    }

    pub fn get(&self, addr: u8) -> u8 {
        let obj = &self.objects[addr as usize / 4];
        match addr % 4 {
            0 => obj.y,
            1 => obj.x,
            2 => obj.tile_id,
            3 => obj.attr,
            _ => unreachable!(),
        }
    }
    pub fn set(&mut self, addr: u8, value: u8) {
        let obj = &mut self.objects[addr as usize / 4];
        match addr % 4 {
            0 => obj.y = value,
            1 => obj.x = value,
            2 => obj.tile_id = value,
            3 => obj.attr = value,
            _ => unreachable!(),
        }
    }
}
