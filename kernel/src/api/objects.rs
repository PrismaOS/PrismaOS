use alloc::{sync::Arc, vec::Vec};
use core::any::Any;
use serde::{Deserialize, Serialize};
use spin::RwLock;
use volatile::Volatile;

use super::{InputEvent, ObjectHandle, PixelFormat};

pub trait KernelObject: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub buffer: RwLock<Option<ObjectHandle>>,
    pub committed: RwLock<bool>,
    pub damage_rects: RwLock<Vec<Rect>>,
}

impl Surface {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Surface {
            width,
            height,
            format,
            buffer: RwLock::new(None),
            committed: RwLock::new(false),
            damage_rects: RwLock::new(Vec::new()),
        }
    }

    pub fn attach_buffer(&self, buffer: ObjectHandle) {
        *self.buffer.write() = Some(buffer);
        *self.committed.write() = false;
    }

    pub fn commit(&self) {
        *self.committed.write() = true;
    }

    pub fn add_damage(&self, rect: Rect) {
        self.damage_rects.write().push(rect);
    }

    pub fn take_damage(&self) -> Vec<Rect> {
        let mut damage = self.damage_rects.write();
        let rects = damage.clone();
        damage.clear();
        rects
    }
}

impl KernelObject for Surface {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        "Surface"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct Buffer {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

impl Buffer {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bytes_per_pixel = match format {
            PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => 4,
            PixelFormat::Rgb888 | PixelFormat::Bgr888 => 3,
        };
        let stride = width * bytes_per_pixel;
        let size = (stride * height) as usize;

        Buffer {
            data: alloc::vec![0; size],
            width,
            height,
            stride,
            format,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl KernelObject for Buffer {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        "Buffer"
    }
}

#[derive(Debug)]
pub struct EventStream {
    events: RwLock<Vec<InputEvent>>,
}

impl EventStream {
    pub fn new() -> Self {
        EventStream {
            events: RwLock::new(Vec::new()),
        }
    }

    pub fn push_event(&self, event: InputEvent) {
        self.events.write().push(event);
    }

    pub fn poll_event(&self) -> Option<InputEvent> {
        self.events.write().pop()
    }

    pub fn has_events(&self) -> bool {
        !self.events.read().is_empty()
    }
}

impl KernelObject for EventStream {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        "EventStream"
    }
}

#[derive(Debug)]
pub struct Display {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub framebuffer: RwLock<Option<*mut u8>>,
    pub exclusive_owner: RwLock<Option<ObjectHandle>>,
    pub vsync_enabled: RwLock<bool>,
}

unsafe impl Send for Display {}
unsafe impl Sync for Display {}

impl Display {
    pub fn new(width: u32, height: u32, refresh_rate: u32, framebuffer: *mut u8) -> Self {
        Display {
            width,
            height,
            refresh_rate,
            framebuffer: RwLock::new(Some(framebuffer)),
            exclusive_owner: RwLock::new(None),
            vsync_enabled: RwLock::new(true),
        }
    }

    pub fn request_exclusive(&self, requestor: ObjectHandle) -> bool {
        let mut owner = self.exclusive_owner.write();
        if owner.is_none() {
            *owner = Some(requestor);
            true
        } else {
            false
        }
    }

    pub fn release_exclusive(&self, requestor: ObjectHandle) -> bool {
        let mut owner = self.exclusive_owner.write();
        if *owner == Some(requestor) {
            *owner = None;
            true
        } else {
            false
        }
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive_owner.read().is_some()
    }

    pub fn get_framebuffer(&self) -> Option<*mut u8> {
        *self.framebuffer.read()
    }
}

impl KernelObject for Display {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        "Display"
    }
}