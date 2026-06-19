#[derive(Clone, Copy, Default)]
pub enum GamepadRegion {
    #[default]
    None,
    DPad,
    Buttons,
}

#[derive(Default)]
pub struct Gamepad {
    pub region: GamepadRegion,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
}

impl Gamepad {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self) -> u8 {
        match self.region {
            GamepadRegion::None => 0,
            GamepadRegion::DPad => self.get_d_pad(),
            GamepadRegion::Buttons => self.get_buttons(),
        }
    }
    pub fn get_d_pad(&self) -> u8 {
        0 | (!self.right as u8) << 0
            | (!self.left as u8) << 1
            | (!self.up as u8) << 2
            | (!self.down as u8) << 3
    }
    pub fn get_buttons(&self) -> u8 {
        0 | (!self.a as u8) << 0
            | (!self.b as u8) << 1
            | (!self.select as u8) << 2
            | (!self.start as u8) << 3
    }
}
