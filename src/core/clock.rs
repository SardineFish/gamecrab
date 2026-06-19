pub struct Clock {
    t_state: u64,
}

impl Clock {
    pub fn new() -> Self {
        Self { t_state: 0 }
    }

    pub fn get_t_state(&self) -> u64 {
        self.t_state
    }
    pub fn add_t_state(&mut self, t_state: u8) {
        self.t_state += t_state as u64;
    }
}
