#![no_std]

extern crate alloc;

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, RwLock};

pub mod surface;
pub mod renderer;
pub mod input;
pub mod exclusive;

use surface::*;
use renderer::*;

pub struct Compositor {
    surfaces: RwLock<BTreeMap<SurfaceId, Arc<Surface>>>,
    display_width: u32,
    display_height: u32,
    framebuffer: *mut u8,
    frame_count: AtomicU64,
    vsync_enabled: AtomicU32,
}

unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}

impl Compositor {
    pub fn new(framebuffer: *mut u8, width: u32, height: u32) -> Self {
        Compositor {
            surfaces: RwLock::new(BTreeMap::new()),
            display_width: width,
            display_height: height,
            framebuffer,
            frame_count: AtomicU64::new(0),
            vsync_enabled: AtomicU32::new(1),
        }
    }

    pub fn create_surface(&self, width: u32, height: u32, format: PixelFormat) -> SurfaceId {
        let surface = Arc::new(Surface::new(width, height, format));
        let id = surface.id();
        self.surfaces.write().insert(id, surface);
        id
    }

    pub fn destroy_surface(&self, id: SurfaceId) {
        self.surfaces.write().remove(&id);
    }

    pub fn get_surface(&self, id: SurfaceId) -> Option<Arc<Surface>> {
        self.surfaces.read().get(&id).cloned()
    }

    pub fn composite_frame(&self) {
        let surfaces = self.surfaces.read();
        let mut renderer = SoftwareRenderer::new(
            self.framebuffer,
            self.display_width,
            self.display_height,
        );

        // Clear framebuffer with black
        renderer.clear(0x000000);

        // Composite all surfaces in Z-order
        for surface in surfaces.values() {
            if surface.is_visible() {
                let (x, y) = surface.get_position();
                renderer.blit_surface(surface.as_ref(), x, y);
            }
        }

        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }

    pub fn set_vsync(&self, enabled: bool) {
        self.vsync_enabled.store(enabled as u32, Ordering::Relaxed);
    }

    pub fn is_vsync_enabled(&self) -> bool {
        self.vsync_enabled.load(Ordering::Relaxed) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SurfaceId(u64);

impl SurfaceId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        SurfaceId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8888,
    Rgb888,
    Bgra8888,
    Bgr888,
}