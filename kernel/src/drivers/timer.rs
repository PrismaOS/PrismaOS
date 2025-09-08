use core::any::Any;
use super::{Driver, DriverError};

pub struct TimerDriver {
    initialized: bool,
    tick_count: u64,
}

impl TimerDriver {
    pub fn new() -> Self {
        TimerDriver {
            initialized: false,
            tick_count: 0,
        }
    }

    pub fn get_ticks(&self) -> u64 {
        self.tick_count
    }
}

impl Driver for TimerDriver {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn init(&mut self) -> Result<(), DriverError> {
        self.initialized = true;
        self.tick_count = 0;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        self.initialized = false;
        Ok(())
    }

    fn interrupt_handler(&mut self, _irq: u8) -> bool {
        self.tick_count += 1;
        crate::scheduler::scheduler_tick(0); // Notify scheduler
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}