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
    pub fn tick(&mut self) -> bool {
        self.tick_many(1)
    }

    pub fn tick_many(&mut self, t_states: u16) -> bool {
        if t_states == 0 {
            return false;
        }

        let old_counter = self.counter();
        let new_counter = old_counter.wrapping_add(t_states);
        self.div = (new_counter >> 8) as u8;
        self.subdiv = new_counter as u8;

        if self.tac & 0b_100 == 0 {
            return false;
        }

        let increments = timer_increments(old_counter, t_states, self.tac & 0b_11);
        self.increment_tima(increments)
    }

    fn counter(&self) -> u16 {
        (self.div as u16) << 8 | self.subdiv as u16
    }

    fn increment_tima(&mut self, increments: u16) -> bool {
        let mut overflow = false;

        for _ in 0..increments {
            self.tima = self.tima.wrapping_add(1);
            if self.tima == 0 {
                self.tima = self.tma;
                self.overflow = true;
                overflow = true;
            }
        }

        overflow
    }
}

fn timer_increments(old_counter: u16, t_states: u16, tac_select: u8) -> u16 {
    let period = match tac_select {
        0 => 1024,
        1 => 16,
        2 => 64,
        3 => 256,
        _ => unreachable!(),
    };

    let offset = old_counter % period;
    let to_next = if offset == 0 { period } else { period - offset };

    if t_states < to_next {
        0
    } else {
        1 + (t_states - to_next) / period
    }
}
