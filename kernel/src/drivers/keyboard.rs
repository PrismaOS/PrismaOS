use core::any::Any;
use super::{Driver, DriverError};

pub struct KeyboardDriver {
    initialized: bool,
}

impl KeyboardDriver {
    pub fn new() -> Self {
        KeyboardDriver {
            initialized: false,
        }
    }
}

impl Driver for KeyboardDriver {
    fn name(&self) -> &'static str {
        "keyboard"
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
        // Keyboard interrupt handling is done in executor/keyboard.rs
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}