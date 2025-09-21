use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::RwLock;

use crate::{PixelFormat, SurfaceId};

pub struct Surface {
    id: SurfaceId,
    width: u32,
    height: u32,
    format: PixelFormat,
    buffer: RwLock<Option<Vec<u8>>>,
    position: RwLock<(i32, i32)>,
    visible: AtomicBool,
    z_order: AtomicU32,
    committed: AtomicBool,
}

impl Surface {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Surface {
            id: SurfaceId::new(),
            width,
            height,
            format,
            buffer: RwLock::new(None),
            position: RwLock::new((0, 0)),
            visible: AtomicBool::new(true),
            z_order: AtomicU32::new(0),
            committed: AtomicBool::new(false),
        }
    }

    pub fn id(&self) -> SurfaceId {
        self.id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    pub fn attach_buffer(&self, buffer: Vec<u8>) -> Result<(), &'static str> {
        let expected_size = self.calculate_buffer_size();
        if buffer.len() != expected_size {
            return Err("Buffer size mismatch");
        }

        *self.buffer.write() = Some(buffer);
        self.committed.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn commit(&self) {
        self.committed.store(true, Ordering::Relaxed);
    }

    pub fn is_committed(&self) -> bool {
        self.committed.load(Ordering::Relaxed)
    }

    pub fn get_buffer(&self) -> Option<Vec<u8>> {
        self.buffer.read().clone()
    }

    pub fn set_position(&self, x: i32, y: i32) {
        *self.position.write() = (x, y);
    }

    pub fn get_position(&self) -> (i32, i32) {
        *self.position.read()
    }

    pub fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Relaxed);
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Relaxed) && self.is_committed()
    }

    pub fn set_z_order(&self, z: u32) {
        self.z_order.store(z, Ordering::Relaxed);
    }

    pub fn get_z_order(&self) -> u32 {
        self.z_order.load(Ordering::Relaxed)
    }

    fn calculate_buffer_size(&self) -> usize {
        let bytes_per_pixel = match self.format {
            PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => 4,
            PixelFormat::Rgb888 | PixelFormat::Bgr888 => 3,
        };
        (self.width * self.height * bytes_per_pixel) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Rect { x, y, width, height }
    }

    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width as i32).min(other.x + other.width as i32);
        let y2 = (self.y + self.height as i32).min(other.y + other.height as i32);

        if x1 < x2 && y1 < y2 {
            Some(Rect::new(x1, y1, (x2 - x1) as u32, (y2 - y1) as u32))
        } else {
            None
        }
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width as i32 &&
        y >= self.y && y < self.y + self.height as i32
    }
}