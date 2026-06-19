#[derive(Default)]
pub struct Timer {
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub overflow: bool,
    subdiv: u8,
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, addr_offset: u8) -> u8 {
        match addr_offset {
            0 => self.div,
            1 => self.tima,
            2 => self.tma,
            3 => self.tac,
            _ => panic!(),
        }
    }
    pub fn set(&mut self, addr_offset: u8, value: u8) {
        match addr_offset {
            0 => {
                self.div = 0;
                self.subdiv = 0;
            }
            1 => self.tima = value,
            2 => self.tma = value,
            3 => self.tac = value,
            _ => panic!(),
        }
    }

    /**
     * Called every T-state.
     */
    pub fn tick(&mut self) {
        self.subdiv = self.subdiv.wrapping_add(1);
        if self.subdiv == 0 {
            self.div = self.div.wrapping_add(1);
        }
        if self.tac & 0b_100 > 0 {
            let increased = match self.tac & 0b_11 {
                0 => ((self.div as u16) << 8 | self.subdiv as u16) % 1024 == 0,
                1 => self.subdiv % 16 == 0,
                2 => self.subdiv % 64 == 0,
                3 => self.subdiv == 0,
                _ => unreachable!(),
            };
            if increased {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0 {
                    self.tima = self.tma;
                    self.overflow = true;
                }
            }
        }
    }
}
