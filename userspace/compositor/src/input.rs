use alloc::{collections::BTreeMap, vec::Vec, vec};
use core::sync::atomic::{AtomicU32, Ordering};
use spin::RwLock;

use crate::SurfaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    KeyPress { scancode: u8, modifiers: u32 },
    KeyRelease { scancode: u8, modifiers: u32 },
    MouseMove { x: i32, y: i32 },
    MousePress { button: u8, x: i32, y: i32 },
    MouseRelease { button: u8, x: i32, y: i32 },
    MouseWheel { delta_x: i16, delta_y: i16, x: i32, y: i32 },
}

pub struct InputManager {
    mouse_x: AtomicU32,
    mouse_y: AtomicU32,
    mouse_buttons: AtomicU32,
    modifier_keys: AtomicU32,
    focused_surface: RwLock<Option<SurfaceId>>,
    surface_positions: RwLock<BTreeMap<SurfaceId, (i32, i32, u32, u32)>>, // x, y, width, height
}

impl InputManager {
    pub fn new() -> Self {
        InputManager {
            mouse_x: AtomicU32::new(0),
            mouse_y: AtomicU32::new(0),
            mouse_buttons: AtomicU32::new(0),
            modifier_keys: AtomicU32::new(0),
            focused_surface: RwLock::new(None),
            surface_positions: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn handle_input_event(&self, event: InputEvent) -> Vec<(SurfaceId, InputEvent)> {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.mouse_x.store(x as u32, Ordering::Relaxed);
                self.mouse_y.store(y as u32, Ordering::Relaxed);
                
                // Find surface under cursor
                if let Some(surface_id) = self.surface_at_point(x, y) {
                    vec![(surface_id, event)]
                } else {
                    Vec::new()
                }
            }
            InputEvent::MousePress { button, x, y } => {
                let current_buttons = self.mouse_buttons.load(Ordering::Relaxed);
                self.mouse_buttons.store(current_buttons | (1 << button), Ordering::Relaxed);
                
                // Find surface to focus
                if let Some(surface_id) = self.surface_at_point(x, y) {
                    *self.focused_surface.write() = Some(surface_id);
                    vec![(surface_id, event)]
                } else {
                    *self.focused_surface.write() = None;
                    Vec::new()
                }
            }
            InputEvent::MouseRelease { button, x, y } => {
                let current_buttons = self.mouse_buttons.load(Ordering::Relaxed);
                self.mouse_buttons.store(current_buttons & !(1 << button), Ordering::Relaxed);
                
                if let Some(surface_id) = self.surface_at_point(x, y) {
                    vec![(surface_id, event)]
                } else {
                    Vec::new()
                }
            }
            InputEvent::KeyPress { scancode, modifiers } => {
                self.modifier_keys.store(modifiers, Ordering::Relaxed);
                
                if let Some(surface_id) = *self.focused_surface.read() {
                    vec![(surface_id, event)]
                } else {
                    Vec::new()
                }
            }
            InputEvent::KeyRelease { scancode, modifiers } => {
                self.modifier_keys.store(modifiers, Ordering::Relaxed);
                
                if let Some(surface_id) = *self.focused_surface.read() {
                    vec![(surface_id, event)]
                } else {
                    Vec::new()
                }
            }
            InputEvent::MouseWheel { delta_x, delta_y, x, y } => {
                if let Some(surface_id) = self.surface_at_point(x, y) {
                    vec![(surface_id, event)]
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub fn update_surface_geometry(&self, surface_id: SurfaceId, x: i32, y: i32, width: u32, height: u32) {
        self.surface_positions.write().insert(surface_id, (x, y, width, height));
    }

    pub fn remove_surface(&self, surface_id: SurfaceId) {
        self.surface_positions.write().remove(&surface_id);
        
        let mut focused = self.focused_surface.write();
        if *focused == Some(surface_id) {
            *focused = None;
        }
    }

    fn surface_at_point(&self, x: i32, y: i32) -> Option<SurfaceId> {
        let positions = self.surface_positions.read();
        
        // Find topmost surface at point (reverse iteration for Z-order)
        for (&surface_id, &(sx, sy, width, height)) in positions.iter().rev() {
            if x >= sx && x < sx + width as i32 && y >= sy && y < sy + height as i32 {
                return Some(surface_id);
            }
        }
        
        None
    }

    pub fn get_mouse_position(&self) -> (i32, i32) {
        let x = self.mouse_x.load(Ordering::Relaxed) as i32;
        let y = self.mouse_y.load(Ordering::Relaxed) as i32;
        (x, y)
    }

    pub fn get_mouse_buttons(&self) -> u32 {
        self.mouse_buttons.load(Ordering::Relaxed)
    }

    pub fn get_modifiers(&self) -> u32 {
        self.modifier_keys.load(Ordering::Relaxed)
    }

    pub fn get_focused_surface(&self) -> Option<SurfaceId> {
        *self.focused_surface.read()
    }
}

// Modifier key constants
pub mod modifiers {
    pub const SHIFT: u32 = 1 << 0;
    pub const CTRL: u32 = 1 << 1;
    pub const ALT: u32 = 1 << 2;
    pub const SUPER: u32 = 1 << 3;
}

// Mouse button constants
pub mod buttons {
    pub const LEFT: u8 = 0;
    pub const RIGHT: u8 = 1;
    pub const MIDDLE: u8 = 2;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMapping {
    pub scancode: u8,
    pub key_code: u32,
    pub character: Option<char>,
}

pub struct KeyboardLayout {
    mappings: [Option<KeyMapping>; 256],
}

impl KeyboardLayout {
    pub fn us_qwerty() -> Self {
        let mut layout = KeyboardLayout {
            mappings: [None; 256],
        };

        // Basic US QWERTY mappings (scancode -> character)
        layout.add_mapping(0x02, 0x02, Some('1'));
        layout.add_mapping(0x03, 0x03, Some('2'));
        layout.add_mapping(0x04, 0x04, Some('3'));
        layout.add_mapping(0x05, 0x05, Some('4'));
        layout.add_mapping(0x06, 0x06, Some('5'));
        layout.add_mapping(0x07, 0x07, Some('6'));
        layout.add_mapping(0x08, 0x08, Some('7'));
        layout.add_mapping(0x09, 0x09, Some('8'));
        layout.add_mapping(0x0A, 0x0A, Some('9'));
        layout.add_mapping(0x0B, 0x0B, Some('0'));

        layout.add_mapping(0x10, 0x10, Some('q'));
        layout.add_mapping(0x11, 0x11, Some('w'));
        layout.add_mapping(0x12, 0x12, Some('e'));
        layout.add_mapping(0x13, 0x13, Some('r'));
        layout.add_mapping(0x14, 0x14, Some('t'));
        layout.add_mapping(0x15, 0x15, Some('y'));
        layout.add_mapping(0x16, 0x16, Some('u'));
        layout.add_mapping(0x17, 0x17, Some('i'));
        layout.add_mapping(0x18, 0x18, Some('o'));
        layout.add_mapping(0x19, 0x19, Some('p'));

        layout.add_mapping(0x1E, 0x1E, Some('a'));
        layout.add_mapping(0x1F, 0x1F, Some('s'));
        layout.add_mapping(0x20, 0x20, Some('d'));
        layout.add_mapping(0x21, 0x21, Some('f'));
        layout.add_mapping(0x22, 0x22, Some('g'));
        layout.add_mapping(0x23, 0x23, Some('h'));
        layout.add_mapping(0x24, 0x24, Some('j'));
        layout.add_mapping(0x25, 0x25, Some('k'));
        layout.add_mapping(0x26, 0x26, Some('l'));

        layout.add_mapping(0x2C, 0x2C, Some('z'));
        layout.add_mapping(0x2D, 0x2D, Some('x'));
        layout.add_mapping(0x2E, 0x2E, Some('c'));
        layout.add_mapping(0x2F, 0x2F, Some('v'));
        layout.add_mapping(0x30, 0x30, Some('b'));
        layout.add_mapping(0x31, 0x31, Some('n'));
        layout.add_mapping(0x32, 0x32, Some('m'));

        layout.add_mapping(0x39, 0x39, Some(' ')); // Space
        layout.add_mapping(0x1C, 0x1C, Some('\n')); // Enter

        layout
    }

    fn add_mapping(&mut self, scancode: u8, key_code: u32, character: Option<char>) {
        self.mappings[scancode as usize] = Some(KeyMapping {
            scancode,
            key_code,
            character,
        });
    }

    pub fn get_mapping(&self, scancode: u8) -> Option<&KeyMapping> {
        self.mappings.get(scancode as usize)?.as_ref()
    }
}