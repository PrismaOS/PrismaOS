# PrismaOS USB Driver

This crate provides a modular, production-ready USB driver for PrismaOS, designed for both real-world use and as a teaching resource for OSDev learners.

## Features
- Modular bus/controller support (see `src/bus/`)
- Device/class management
- Kernel driver integration
- Comprehensive documentation for education and production

## Directory Structure
- `src/bus/`   : Hardware controller implementations (e.g., xHCI)
- `src/class/`: (Future) USB class implementations
- `src/lib.rs`: Driver entry point, device/class glue, kernel integration

## How to Extend
- Implement a real hardware bus in `src/bus/` (see `src/bus/xhci.rs`)
- Add USB classes in `src/class/`
- Integrate with kernel events/interrupts
- Use as a template for other drivers

## Example Usage
```rust
use usb::UsbDriver;

let mut driver = UsbDriver::new();
driver.init().unwrap();
// In your main loop or interrupt handler:
usb::poll_usb_driver(&mut driver);
```

## xHCI Bus Implementation
See `src/bus/xhci.rs` for a stub and documentation on implementing a real xHCI controller.

## License
MIT OR Apache-2.0
