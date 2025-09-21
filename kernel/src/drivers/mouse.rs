use core::any::Any;
use super::{Driver, DriverError};

pub struct MouseDriver {
    initialized: bool,
}

impl MouseDriver {
    pub fn new() -> Self {
        MouseDriver {
            initialized: false,
        }
    }
}

impl Driver for MouseDriver {
    fn name(&self) -> &'static str {
        "mouse"
    }

    fn init(&mut self) -> Result<(), DriverError> {
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        self.initialized = false;
        Ok(())
    }

    fn interrupt_handler(&mut self, _irq: u8) -> bool {
        // Mouse interrupt handling
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}