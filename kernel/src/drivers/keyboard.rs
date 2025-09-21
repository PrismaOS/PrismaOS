#![allow(dead_code)]

use core::any::Any;
use super::{Driver, DriverError};
use crate::api::commands::{inb, outb};

// PS/2 Keyboard constants
const KB_DATA_PORT: u16 = 0x60;
const KB_STATUS_PORT: u16 = 0x64;
const KB_COMMAND_PORT: u16 = 0x64;

// Keyboard commands
const KB_ENABLE_KEYBOARD: u8 = 0xAE;

// Status flags
const KB_STATUS_OUTPUT_FULL: u8 = 0x01;
const KB_STATUS_INPUT_FULL: u8 = 0x02;

// PIC ports for enabling keyboard IRQ
const PIC1_DATA_PORT: u16 = 0x21;

// Key codes
const KEY_ESC: u8 = 1;
const KEY_BACKSPACE: u8 = 14;
const KEY_TAB: u8 = 15;
const KEY_ENTER: u8 = 28;
const KEY_CTRL: u8 = 29;
const KEY_LSHIFT: u8 = 42;
const KEY_RSHIFT: u8 = 54;
const KEY_ALT: u8 = 56;
const KEY_SPACE: u8 = 57;
const KEY_CAPS: u8 = 58;
const KEY_F1: u8 = 59;
const KEY_F2: u8 = 60;
const KEY_F3: u8 = 61;
const KEY_F4: u8 = 62;
const KEY_F5: u8 = 63;
const KEY_F6: u8 = 64;
const KEY_F7: u8 = 65;
const KEY_F8: u8 = 66;
const KEY_F9: u8 = 67;
const KEY_F10: u8 = 68;
const KEY_HOME: u8 = 71;
const KEY_UP: u8 = 72;
const KEY_PGUP: u8 = 73;
const KEY_LEFT: u8 = 75;
const KEY_RIGHT: u8 = 77;
const KEY_END: u8 = 79;
const KEY_DOWN: u8 = 80;
const KEY_PGDOWN: u8 = 81;
const KEY_INSERT: u8 = 82;
const KEY_DELETE: u8 = 83;

/// Modifier state tracking
#[derive(Debug, Clone, Copy)]
struct ModifierState {
    shift: bool,
    ctrl: bool,
    alt: bool,
    caps_lock: bool,
    extended: bool,
}

impl Default for ModifierState {
    fn default() -> Self {
        ModifierState {
            shift: false,
            ctrl: false,
            alt: false,
            caps_lock: false,
            extended: false,
        }
    }
}

/// Scancode to key mapping table
static SCANCODE_TO_KEY: [u8; 128] = [
    0, KEY_ESC, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 
    KEY_BACKSPACE, KEY_TAB, b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', 
    b'[', b']', KEY_ENTER, KEY_CTRL, b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', 
    b'l', b';', b'\'', b'`', KEY_LSHIFT, b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', 
    b',', b'.', b'/', KEY_RSHIFT, b'*', KEY_ALT, KEY_SPACE, KEY_CAPS, KEY_F1, 
    KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10,
    0, 0, KEY_HOME, KEY_UP, KEY_PGUP, 0, KEY_LEFT, 0, KEY_RIGHT, 0, KEY_END,
    KEY_DOWN, KEY_PGDOWN, KEY_INSERT, KEY_DELETE,
    // Fill remaining entries to reach exactly 128
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Shift character mapping
static SHIFT_MAP: &[u8] = b"!@#$%^&*()_+{}|:\"~<>?";
static NORMAL_MAP: &[u8] = b"1234567890-=[]\\;'`,./";

pub struct KeyboardDriver {
    initialized: bool,
    modifiers: ModifierState,
}

impl KeyboardDriver {
    pub fn new() -> Self {
        KeyboardDriver {
            initialized: false,
            modifiers: ModifierState::default(),
        }
    }

    /// Initialize the PS/2 keyboard controller
    fn init_keyboard(&mut self) -> Result<(), DriverError> {
        unsafe {
            // Clear any pending keyboard data
            while (inb(KB_STATUS_PORT) & KB_STATUS_OUTPUT_FULL) != 0 {
                inb(KB_DATA_PORT);
            }

            // Wait for controller to be ready and enable keyboard
            while (inb(KB_STATUS_PORT) & KB_STATUS_INPUT_FULL) != 0 {}
            outb(KB_COMMAND_PORT, KB_ENABLE_KEYBOARD);
            while (inb(KB_STATUS_PORT) & KB_STATUS_INPUT_FULL) != 0 {}

            // Enable keyboard IRQ (IRQ 1) on PIC
            let mask = inb(PIC1_DATA_PORT);
            outb(PIC1_DATA_PORT, mask & !(1 << 1));
        }
        Ok(())
    }

    /// Convert a key to its character representation
    fn get_character(&self, key: u8) -> Option<char> {
        if key == 0 || key >= 128 {
            return None;
        }

        let c = key as char;

        // Handle alphabetic characters with caps lock and shift
        if c.is_ascii_lowercase() {
            if self.modifiers.caps_lock ^ self.modifiers.shift {
                return Some(c.to_ascii_uppercase());
            }
            return Some(c);
        }

        // Handle shifted punctuation
        if self.modifiers.shift {
            for (i, &normal_char) in NORMAL_MAP.iter().enumerate() {
                if normal_char == key && i < SHIFT_MAP.len() {
                    return Some(SHIFT_MAP[i] as char);
                }
            }
        }

        Some(c)
    }

    /// Handle key press events
    fn handle_key_press(&mut self, key: u8) {
        match key {
            KEY_LSHIFT | KEY_RSHIFT => {
                self.modifiers.shift = true;
                return;
            }
            KEY_CTRL => {
                self.modifiers.ctrl = true;
                return;
            }
            KEY_ALT => {
                self.modifiers.alt = true;
                return;
            }
            KEY_CAPS => {
                self.modifiers.caps_lock = !self.modifiers.caps_lock;
                return;
            }
            _ => {}
        }

        // Handle special keys
        match key {
            KEY_BACKSPACE => {
                crate::print!("\x08 \x08"); // Backspace, space, backspace
            }
            KEY_ENTER => {
                crate::print!("\n");
            }
            KEY_TAB => {
                crate::print!("    "); // 4 spaces for tab
            }
            KEY_ESC => {
                crate::print!("[ESC]");
            }
            _ => {
                // Handle printable characters
                if key >= 32 && key <= 126 {
                    if let Some(c) = self.get_character(key) {
                        if self.modifiers.ctrl {
                            // Handle Ctrl+key combinations
                            match c.to_ascii_lowercase() {
                                'l' => crate::print!("[Ctrl+L]"), // Clear screen
                                'c' => crate::print!("[Ctrl+C]"), // Cancel
                                'd' => crate::print!("[Ctrl+D]"), // EOF
                                _ => {}
                            }
                        } else {
                            crate::print!("{}", c);
                        }
                    }
                }
            }
        }
    }

    /// Handle key release events
    fn handle_key_release(&mut self, key: u8) {
        match key {
            KEY_LSHIFT | KEY_RSHIFT => {
                self.modifiers.shift = false;
            }
            KEY_CTRL => {
                self.modifiers.ctrl = false;
            }
            KEY_ALT => {
                self.modifiers.alt = false;
            }
            _ => {}
        }
    }

    /// Process keyboard interrupt and handle scancode
    pub fn process_interrupt(&mut self) -> bool {
        unsafe {
            if (inb(KB_STATUS_PORT) & KB_STATUS_OUTPUT_FULL) == 0 {
                return false;
            }

            let scancode_raw = inb(KB_DATA_PORT);

            // Handle extended scancode prefix
            if scancode_raw == 0xE0 {
                self.modifiers.extended = true;
                return true;
            }

            let scancode = scancode_raw & 0x7F;
            let key_pressed = (scancode_raw & 0x80) == 0;

            if scancode >= SCANCODE_TO_KEY.len() as u8 {
                self.modifiers.extended = false;
                return false;
            }

            let key = SCANCODE_TO_KEY[scancode as usize];

            if key_pressed {
                self.handle_key_press(key);
                
                // Also add to the executor's scancode queue for async processing
                crate::executor::keyboard::add_scancode(scancode_raw);
            } else {
                self.handle_key_release(key);
            }

            self.modifiers.extended = false;
            true
        }
    }
}

impl Driver for KeyboardDriver {
    fn name(&self) -> &'static str {
        "keyboard"
    }

    fn init(&mut self) -> Result<(), DriverError> {
        self.init_keyboard()?;
        self.initialized = true;
        crate::println!("PS/2 Keyboard driver initialized");
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), DriverError> {
        // Disable keyboard IRQ
        unsafe {
            let mask = inb(PIC1_DATA_PORT);
            outb(PIC1_DATA_PORT, mask | (1 << 1));
        }
        self.initialized = false;
        self.modifiers = ModifierState::default();
        crate::println!("PS/2 Keyboard driver shutdown");
        Ok(())
    }

    fn interrupt_handler(&mut self, irq: u8) -> bool {
        if irq == 1 || irq == 33 { // IRQ 1 (legacy) or IRQ 33 (remapped)
            self.process_interrupt()
        } else {
            false
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Global keyboard state and interface
pub struct KeyboardInterface {
    current_scancode: Option<u8>,
    last_key_pressed: Option<u8>,
}

impl KeyboardInterface {
    pub const fn new() -> Self {
        KeyboardInterface {
            current_scancode: None,
            last_key_pressed: None,
        }
    }

    /// Get the last pressed key (if any)
    pub fn get_last_key(&self) -> Option<u8> {
        self.last_key_pressed
    }

    /// Check if a specific key is currently pressed
    pub fn is_key_pressed(&self, key: u8) -> bool {
        // This would require maintaining key state - simplified for now
        self.last_key_pressed == Some(key)
    }

    /// Update with latest scancode
    pub fn update_scancode(&mut self, scancode: u8) {
        self.current_scancode = Some(scancode);
        if (scancode & 0x80) == 0 {
            // Key press
            if (scancode as usize) < SCANCODE_TO_KEY.len() {
                self.last_key_pressed = Some(SCANCODE_TO_KEY[scancode as usize]);
            }
        }
    }
}

/// Global keyboard interface instance
static KEYBOARD_INTERFACE: spin::Mutex<KeyboardInterface> = spin::Mutex::new(KeyboardInterface::new());

/// Get access to the global keyboard interface
pub fn keyboard_interface() -> &'static spin::Mutex<KeyboardInterface> {
    &KEYBOARD_INTERFACE
}

/// Initialize keyboard subsystem
pub fn init_keyboard_subsystem() {
    crate::println!("Keyboard subsystem initialized");
}
/* 
#include <ps2_keyboard.h>
#include <serial.h>
#include <io.h>
#include <interrupts.h>
#include <shell.h>
#include <framebuffer.h>
#include <util.h>

static modifier_state_t modifiers = {0};

static const uint8_t scancode_to_key[128] = {
    0, KEY_ESC, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', 
    KEY_BACKSPACE, KEY_TAB, 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', 
    '[', ']', KEY_ENTER, KEY_CTRL, 'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 
    'l', ';', '\'', '`', KEY_LSHIFT, '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', 
    ',', '.', '/', KEY_RSHIFT, '*', KEY_ALT, KEY_SPACE, KEY_CAPS, KEY_F1, 
    KEY_F2, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10,
    0, 0, KEY_HOME, KEY_UP, KEY_PGUP, 0, KEY_LEFT, 0, KEY_RIGHT, 0, KEY_END,
    KEY_DOWN, KEY_PGDOWN, KEY_INSERT, KEY_DELETE
};

static const char shift_map[] = "!@#$%^&*()_+{}|:\"~<>?";
static const char normal_map[] = "1234567890-=[]\\;'`,./";

void init_keyboard(void) {
    modifiers = (modifier_state_t){0};
    
    while (inb(KB_STATUS_PORT) & KB_STATUS_OUTPUT_FULL) {
        inb(KB_DATA_PORT);
    }
    
    while (inb(KB_STATUS_PORT) & KB_STATUS_INPUT_FULL);
    outb(KB_COMMAND_PORT, KB_ENABLE_KEYBOARD);
    while (inb(KB_STATUS_PORT) & KB_STATUS_INPUT_FULL);
    
    enable_keyboard_irq();
}

char get_character(uint8_t key) {
    if (key == 0 || key >= 128) return 0;
    
    char c = (char)key;
    
    if (c >= 'a' && c <= 'z') {
        if (modifiers.caps_lock ^ modifiers.shift) {
            c = c - 'a' + 'A';
        }
        return c;
    }
    
    if (modifiers.shift) {
        for (int i = 0; normal_map[i]; i++) {
            if (normal_map[i] == c) {
                return shift_map[i];
            }
        }
    }
    
    return c;
}

void handle_key_press(uint8_t key) {
    switch (key) {
        case KEY_LSHIFT:
        case KEY_RSHIFT:
            modifiers.shift = true;
            return;
        case KEY_CTRL:
            modifiers.ctrl = true;
            return;
        case KEY_ALT:
            modifiers.alt = true;
            return;
        case KEY_CAPS:
            modifiers.caps_lock = !modifiers.caps_lock;
            return;
    }
    
    if (key == KEY_BACKSPACE) {
        char *result = input('\b');
        if (result) parse_command();
        return;
    }
    
    if (key == KEY_ENTER) {
        char *command = input('\n');
        if (command) parse_command();
        return;
    }
    
    if (key == KEY_HOME) {
        shell_move_cursor_home();
        return;
    }
    
    if (key == KEY_END) {
        shell_move_cursor_end();
        return;
    }
    
    if (key == KEY_UP) {
        shell_history_up();
        return;
    }
    
    if (key == KEY_DOWN) {
        shell_history_down();
        return;
    }
    
    if (key == KEY_LEFT) {
        shell_move_cursor_left();
        return;
    }
    
    if (key == KEY_RIGHT) {
        shell_move_cursor_right();
        return;
    }
    
    if (key == KEY_DELETE) {
        shell_delete();
        return;
    }
    
    if (key >= 32 && key <= 126) {
        char c = get_character(key);
        if (c == 0) return;
        
        if (modifiers.ctrl) {
            switch (c | 0x20) {
                case 'l': input(12); break;
                case 'u': input(21); break;
                case 'd': input(4); break;
                case 'a': shell_move_cursor_home(); break;
                case 'e': shell_move_cursor_end(); break;
                case 'c': shell_cancel_input(); break;
            }
        } else {
            char *command = input(c);
            if (command) parse_command();
        }
    }
}

void handle_key_release(uint8_t key) {
    switch (key) {
        case KEY_LSHIFT:
        case KEY_RSHIFT:
            modifiers.shift = false;
            break;
        case KEY_CTRL:
            modifiers.ctrl = false;
            break;
        case KEY_ALT:
            modifiers.alt = false;
            break;
    }
}

void keyboard_handler(struct interrupt_registers *regs) {
    if (!(inb(KB_STATUS_PORT) & KB_STATUS_OUTPUT_FULL)) {
        return;
    }
    
    uint8_t scancode_raw = inb(KB_DATA_PORT);
    
    if (scancode_raw == 0xE0) {
        modifiers.extended = true;
        return;
    }
    
    uint8_t scancode = scancode_raw & 0x7F;
    bool key_pressed = !(scancode_raw & 0x80);
    
    if (scancode >= sizeof(scancode_to_key)) {
        modifiers.extended = false;
        return;
    }
    
    uint8_t key = scancode_to_key[scancode];
    
    if (key_pressed) {
        received_key = key;
        handle_key_press(key);
    } else {
        handle_key_release(key);
    }
    
    modifiers.extended = false;
}

void enable_keyboard_irq(void) {
    uint8_t mask = inb(PIC1_DATA_PORT);
    mask &= ~(1 << 1);
    outb(PIC1_DATA_PORT, mask);
}
*/