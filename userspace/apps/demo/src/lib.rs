#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};
use compositor::{
    Compositor, PixelFormat, SurfaceId,
    exclusive::{ExclusiveManager, ExclusiveMode},
    input::{InputEvent, InputManager},
    surface::Surface,
};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub struct DemoApplication {
    compositor: &'static Compositor,
    exclusive_manager: &'static ExclusiveManager,
    input_manager: &'static InputManager,
    main_surface: Option<SurfaceId>,
    exclusive_surface: Option<SurfaceId>,
    frame_counter: AtomicU32,
    running: AtomicBool,
    demo_mode: AtomicU32, // 0: windowed, 1: exclusive fullscreen, 2: direct plane
}

impl DemoApplication {
    pub fn new(
        compositor: &'static Compositor,
        exclusive_manager: &'static ExclusiveManager,
        input_manager: &'static InputManager,
    ) -> Self {
        DemoApplication {
            compositor,
            exclusive_manager,
            input_manager,
            main_surface: None,
            exclusive_surface: None,
            frame_counter: AtomicU32::new(0),
            running: AtomicBool::new(true),
            demo_mode: AtomicU32::new(0),
        }
    }

    pub fn initialize(&mut self) -> Result<(), &'static str> {
        // Create main demo surface
        let surface_id = self.compositor.create_surface(800, 600, PixelFormat::Rgba8888);
        self.main_surface = Some(surface_id);

        // Register surface with input manager
        self.input_manager.update_surface_geometry(surface_id, 100, 100, 800, 600);

        self.render_demo_content(surface_id, 0)?;
        Ok(())
    }

    pub fn run_frame(&self) -> Result<bool, &'static str> {
        let frame = self.frame_counter.fetch_add(1, Ordering::Relaxed);
        
        // Handle input events (would come from kernel in real implementation)
        self.handle_demo_input(frame);

        // Update demo content based on current mode
        let current_mode = self.demo_mode.load(Ordering::Relaxed);
        match current_mode {
            0 => self.run_windowed_mode(frame)?,
            1 => self.run_exclusive_fullscreen(frame)?,
            2 => self.run_direct_plane_mode(frame)?,
            _ => {},
        }

        Ok(self.running.load(Ordering::Relaxed))
    }

    fn run_windowed_mode(&self, frame: u32) -> Result<(), &'static str> {
        if let Some(surface_id) = self.main_surface {
            self.render_demo_content(surface_id, frame)?;
            
            // Compositor will handle the rendering
            self.compositor.composite_frame();
        }
        Ok(())
    }

    fn run_exclusive_fullscreen(&self, frame: u32) -> Result<(), &'static str> {
        if let Some(surface_id) = self.exclusive_surface {
            self.render_high_performance_content(surface_id, frame)?;
            
            // Direct rendering bypass compositor for maximum performance
            if self.exclusive_manager.is_low_latency_active() {
                // Render directly to framebuffer
                self.render_direct_to_framebuffer(frame)?;
            }
        }
        Ok(())
    }

    fn run_direct_plane_mode(&self, frame: u32) -> Result<(), &'static str> {
        if let Some(surface_id) = self.exclusive_surface {
            // Ultra-low latency mode with direct hardware plane access
            unsafe {
                if let Some(direct_fb) = self.exclusive_manager.get_direct_framebuffer(surface_id) {
                    self.render_direct_plane_content(direct_fb, frame)?;
                }
            }
        }
        Ok(())
    }

    fn handle_demo_input(&self, frame: u32) {
        // Simulate some input events for demo purposes
        match frame % 300 {
            0 => {
                // Switch to windowed mode
                self.switch_to_windowed_mode();
            }
            100 => {
                // Switch to exclusive fullscreen
                self.switch_to_exclusive_fullscreen();
            }
            200 => {
                // Switch to direct plane mode  
                self.switch_to_direct_plane();
            }
            _ => {}
        }
    }

    fn switch_to_windowed_mode(&self) {
        if let Some(surface_id) = self.exclusive_surface {
            let _ = self.exclusive_manager.release_exclusive(surface_id);
            self.exclusive_surface = None;
        }
        self.demo_mode.store(0, Ordering::Relaxed);
    }

    fn switch_to_exclusive_fullscreen(&self) {
        // Create exclusive surface for fullscreen rendering
        let surface_id = self.compositor.create_surface(1920, 1080, PixelFormat::Rgba8888);
        
        if let Ok(_) = self.exclusive_manager.request_exclusive(
            surface_id, 
            ExclusiveMode::Fullscreen, 
            100 // High priority
        ) {
            self.exclusive_surface = Some(surface_id);
            self.demo_mode.store(1, Ordering::Relaxed);
            
            // Configure for maximum performance
            let _ = self.exclusive_manager.set_vsync_bypass(surface_id, true);
            let _ = self.exclusive_manager.set_refresh_rate(surface_id, 120);
        }
    }

    fn switch_to_direct_plane(&self) {
        let surface_id = self.compositor.create_surface(1920, 1080, PixelFormat::Rgba8888);
        
        if let Ok(_) = self.exclusive_manager.request_exclusive(
            surface_id,
            ExclusiveMode::DirectPlane,
            200 // Highest priority
        ) {
            self.exclusive_surface = Some(surface_id);
            self.demo_mode.store(2, Ordering::Relaxed);
        }
    }

    fn render_demo_content(&self, surface_id: SurfaceId, frame: u32) -> Result<(), &'static str> {
        if let Some(surface) = self.compositor.get_surface(surface_id) {
            // Create animated gradient pattern
            let width = surface.width();
            let height = surface.height();
            let mut buffer = vec![0u8; (width * height * 4) as usize];
            
            for y in 0..height {
                for x in 0..width {
                    let idx = ((y * width + x) * 4) as usize;
                    
                    // Animated rainbow gradient
                    let phase = (frame as f32 * 0.1) + (x as f32 * 0.01) + (y as f32 * 0.01);
                    let r = ((phase.sin() * 127.0) + 128.0) as u8;
                    let g = (((phase + 2.0).sin() * 127.0) + 128.0) as u8;
                    let b = (((phase + 4.0).sin() * 127.0) + 128.0) as u8;
                    
                    buffer[idx] = r;     // R
                    buffer[idx + 1] = g; // G
                    buffer[idx + 2] = b; // B
                    buffer[idx + 3] = 255; // A
                }
            }
            
            surface.attach_buffer(buffer).map_err(|_| "Failed to attach buffer")?;
            surface.commit();
        }
        Ok(())
    }

    fn render_high_performance_content(&self, surface_id: SurfaceId, frame: u32) -> Result<(), &'static str> {
        // High-performance rendering for exclusive fullscreen mode
        // This would include optimized drawing routines, reduced allocations, etc.
        self.render_demo_content(surface_id, frame * 2) // Faster animation
    }

    fn render_direct_to_framebuffer(&self, frame: u32) -> Result<(), &'static str> {
        // Direct framebuffer access for ultra-low latency
        // This bypasses all compositor overhead
        Ok(())
    }

    fn render_direct_plane_content(&self, framebuffer: *mut u8, frame: u32) -> Result<(), &'static str> {
        // Direct hardware plane rendering - maximum performance
        unsafe {
            let fb_pixels = framebuffer as *mut u32;
            let width = 1920u32;
            let height = 1080u32;
            
            // Ultra-fast solid color fill for demo
            let color = match (frame / 60) % 3 {
                0 => 0xFF_FF_00_00, // Red
                1 => 0xFF_00_FF_00, // Green
                _ => 0xFF_00_00_FF, // Blue
            };
            
            for i in 0..(width * height) {
                *fb_pixels.add(i as usize) = color;
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
        
        // Clean up surfaces
        if let Some(surface_id) = self.main_surface {
            self.compositor.destroy_surface(surface_id);
        }
        
        if let Some(surface_id) = self.exclusive_surface {
            let _ = self.exclusive_manager.release_exclusive(surface_id);
            self.compositor.destroy_surface(surface_id);
        }
    }

    pub fn get_performance_stats(&self) -> DemoStats {
        DemoStats {
            frames_rendered: self.frame_counter.load(Ordering::Relaxed),
            current_mode: match self.demo_mode.load(Ordering::Relaxed) {
                0 => "Windowed",
                1 => "Exclusive Fullscreen", 
                2 => "Direct Plane",
                _ => "Unknown",
            },
            is_exclusive: self.exclusive_surface.is_some(),
            latency_stats: self.exclusive_manager.get_frame_latency_stats(),
        }
    }
}

pub struct DemoStats {
    pub frames_rendered: u32,
    pub current_mode: &'static str,
    pub is_exclusive: bool,
    pub latency_stats: compositor::exclusive::FrameLatencyStats,
}

// Entry point for the demo application
#[no_mangle]
pub extern "C" fn demo_main(
    compositor: *const Compositor,
    exclusive_manager: *const ExclusiveManager,
    input_manager: *const InputManager,
) -> i32 {
    unsafe {
        let compositor = &*compositor;
        let exclusive_manager = &*exclusive_manager;
        let input_manager = &*input_manager;
        
        let mut demo = DemoApplication::new(compositor, exclusive_manager, input_manager);
        
        if demo.initialize().is_err() {
            return -1;
        }

        // Run demo for 1000 frames
        for _ in 0..1000 {
            if !demo.run_frame().unwrap_or(false) {
                break;
            }
        }

        demo.shutdown();
        0
    }
}