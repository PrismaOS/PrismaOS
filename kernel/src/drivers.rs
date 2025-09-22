/// Initialize the device subsystem
pub fn init_devices() {
    crate::println!("Initializing device subsystem...");
    
    // Enumerate PCI devices
    pci_manager().enumerate_devices();
    
    // Initialize core drivers
    init_core_drivers();
    
    crate::println!("Device subsystem initialized");
}

/// Initialize core system drivers
fn init_core_drivers() {
    let dm = device_manager();
    
    // Register framebuffer driver
    let fb_driver = Arc::new(RwLock::new(framebuffer::FramebufferDriver::new()));
    if let Err(e) = dm.register_driver(fb_driver.clone()) {
        crate::println!("Failed to register framebuffer driver: {:?}", e);
    }
    
    // Register keyboard driver
    let kbd_driver = Arc::new(RwLock::new(ps2::KeyboardDriver::new()));
    if let Err(e) = dm.register_driver(kbd_driver.clone()) {
        crate::println!("Failed to register keyboard driver: {:?}", e);
    } else {
        dm.register_irq_handler(33, kbd_driver); // IRQ 33 (32+1) for keyboard
    }
    
    // Register mouse driver
    // TODO: ps2 mouse driver should be created
    // let mouse_driver = Arc::new(RwLock::new(mouse::MouseDriver::new()));
    // if let Err(e) = dm.register_driver(mouse_driver.clone()) {
    //     crate::println!("Failed to register mouse driver: {:?}", e);
    // } else {
    //     dm.register_irq_handler(44, mouse_driver); // IRQ 44 (32+12) for PS/2 mouse
    // }
    
    // Register timer driver
    let timer_driver = Arc::new(RwLock::new(timer::TimerDriver::new()));
    if let Err(e) = dm.register_driver(timer_driver.clone()) {
        crate::println!("Failed to register timer driver: {:?}", e);
    } else {
        dm.register_irq_handler(32, timer_driver); // IRQ 32 (32+0) for PIT timer
    }
}