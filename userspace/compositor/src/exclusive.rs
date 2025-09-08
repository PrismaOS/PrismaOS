use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::{Mutex, RwLock};

use crate::{surface::Surface, SurfaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusiveMode {
    None,
    Fullscreen,
    DirectPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExclusiveRequest {
    pub surface_id: SurfaceId,
    pub mode: ExclusiveMode,
    pub priority: u32,
    pub timestamp: u64,
}

pub struct ExclusiveManager {
    current_exclusive: RwLock<Option<ExclusiveRequest>>,
    pending_requests: Mutex<Vec<ExclusiveRequest>>,
    revocation_policy: RwLock<RevocationPolicy>,
    frame_counter: AtomicU64,
    low_latency_mode: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct RevocationPolicy {
    pub allow_user_override: bool,
    pub allow_emergency_revoke: bool,
    pub timeout_ms: u32,
    pub required_permissions: u32,
}

impl Default for RevocationPolicy {
    fn default() -> Self {
        RevocationPolicy {
            allow_user_override: true,
            allow_emergency_revoke: true,
            timeout_ms: 5000, // 5 second timeout
            required_permissions: 0x1, // Basic display permission
        }
    }
}

impl ExclusiveManager {
    pub fn new() -> Self {
        ExclusiveManager {
            current_exclusive: RwLock::new(None),
            pending_requests: Mutex::new(Vec::new()),
            revocation_policy: RwLock::new(RevocationPolicy::default()),
            frame_counter: AtomicU64::new(0),
            low_latency_mode: AtomicBool::new(false),
        }
    }

    /// Request exclusive access to the display for low-latency rendering
    pub fn request_exclusive(&self, surface_id: SurfaceId, mode: ExclusiveMode, 
                           priority: u32) -> Result<(), ExclusiveError> {
        let timestamp = self.frame_counter.load(Ordering::Relaxed);
        let request = ExclusiveRequest {
            surface_id,
            mode,
            priority,
            timestamp,
        };

        // Check if there's a current exclusive owner
        let current = *self.current_exclusive.read();
        
        match current {
            None => {
                // No current owner, grant immediately
                *self.current_exclusive.write() = Some(request);
                self.activate_exclusive_mode(request);
                Ok(())
            }
            Some(current_request) => {
                // Check priority and policy
                if priority > current_request.priority {
                    // Higher priority, preempt current owner
                    self.revoke_current_exclusive("Preempted by higher priority request");
                    *self.current_exclusive.write() = Some(request);
                    self.activate_exclusive_mode(request);
                    Ok(())
                } else {
                    // Lower priority, queue the request
                    self.pending_requests.lock().push(request);
                    Err(ExclusiveError::AlreadyOwned)
                }
            }
        }
    }

    /// Release exclusive access
    pub fn release_exclusive(&self, surface_id: SurfaceId) -> Result<(), ExclusiveError> {
        let mut current = self.current_exclusive.write();
        
        match *current {
            Some(request) if request.surface_id == surface_id => {
                *current = None;
                self.deactivate_exclusive_mode();
                
                // Process pending requests
                drop(current);
                self.process_pending_requests();
                Ok(())
            }
            Some(_) => Err(ExclusiveError::NotOwner),
            None => Err(ExclusiveError::NotExclusive),
        }
    }

    /// Force revocation of exclusive access (emergency situations)
    pub fn force_revoke(&self, reason: &str) -> bool {
        let policy = self.revocation_policy.read();
        if !policy.allow_emergency_revoke {
            return false;
        }
        drop(policy);

        if let Some(_) = *self.current_exclusive.read() {
            self.revoke_current_exclusive(reason);
            *self.current_exclusive.write() = None;
            self.deactivate_exclusive_mode();
            self.process_pending_requests();
            true
        } else {
            false
        }
    }

    /// Check if a surface has exclusive access
    pub fn is_exclusive(&self, surface_id: SurfaceId) -> bool {
        self.current_exclusive.read()
            .map(|req| req.surface_id == surface_id)
            .unwrap_or(false)
    }

    /// Get current exclusive owner
    pub fn get_exclusive_owner(&self) -> Option<SurfaceId> {
        self.current_exclusive.read().map(|req| req.surface_id)
    }

    /// Check if system is in low-latency mode
    pub fn is_low_latency_active(&self) -> bool {
        self.low_latency_mode.load(Ordering::Relaxed)
    }

    /// Direct framebuffer access for exclusive surface (bypass compositor)
    pub unsafe fn get_direct_framebuffer(&self, surface_id: SurfaceId) -> Option<*mut u8> {
        if !self.is_exclusive(surface_id) {
            return None;
        }

        let current = self.current_exclusive.read();
        if let Some(request) = *current {
            if request.surface_id == surface_id && 
               matches!(request.mode, ExclusiveMode::DirectPlane | ExclusiveMode::Fullscreen) {
                // Return direct access to framebuffer
                // This would be implementation-specific based on graphics driver
                Some(0xFD000000 as *mut u8) // Example direct framebuffer address
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Enable vsync bypass for exclusive surface
    pub fn set_vsync_bypass(&self, surface_id: SurfaceId, bypass: bool) -> Result<(), ExclusiveError> {
        if !self.is_exclusive(surface_id) {
            return Err(ExclusiveError::NotOwner);
        }

        // Configure hardware to bypass vsync for maximum performance
        // This would interface with actual display hardware
        Ok(())
    }

    /// Set custom refresh rate for exclusive surface
    pub fn set_refresh_rate(&self, surface_id: SurfaceId, hz: u32) -> Result<(), ExclusiveError> {
        if !self.is_exclusive(surface_id) {
            return Err(ExclusiveError::NotOwner);
        }

        // Validate refresh rate
        if hz < 30 || hz > 240 {
            return Err(ExclusiveError::InvalidParameter);
        }

        // Configure display controller for custom refresh rate
        // This would interface with display timing controller
        Ok(())
    }

    fn activate_exclusive_mode(&self, request: ExclusiveRequest) {
        self.low_latency_mode.store(true, Ordering::Relaxed);
        
        match request.mode {
            ExclusiveMode::Fullscreen => {
                // Configure display for fullscreen exclusive mode
                // Disable compositor blending, direct surface to scanout
            }
            ExclusiveMode::DirectPlane => {
                // Configure hardware overlay plane for direct rendering
                // Bypass compositor entirely for this surface
            }
            ExclusiveMode::None => {
                // Should not reach here
            }
        }
    }

    fn deactivate_exclusive_mode(&self) {
        self.low_latency_mode.store(false, Ordering::Relaxed);
        
        // Re-enable compositor blending
        // Restore normal display pipeline
    }

    fn revoke_current_exclusive(&self, reason: &str) {
        // Notify the current exclusive owner that access is being revoked
        // This would typically send an IPC message to the owning process
    }

    fn process_pending_requests(&self) {
        let mut pending = self.pending_requests.lock();
        if let Some(highest_priority_idx) = self.find_highest_priority_request(&pending) {
            let request = pending.remove(highest_priority_idx);
            drop(pending);
            
            *self.current_exclusive.write() = Some(request);
            self.activate_exclusive_mode(request);
        }
    }

    fn find_highest_priority_request(&self, requests: &[ExclusiveRequest]) -> Option<usize> {
        requests.iter()
            .enumerate()
            .max_by_key(|(_, req)| req.priority)
            .map(|(idx, _)| idx)
    }

    pub fn update_frame_counter(&self) {
        self.frame_counter.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_revocation_policy(&self, policy: RevocationPolicy) {
        *self.revocation_policy.write() = policy;
    }

    pub fn get_frame_latency_stats(&self) -> FrameLatencyStats {
        // Return frame timing statistics for performance monitoring
        FrameLatencyStats {
            average_frame_time_ns: 16_666_667, // 60 FPS
            min_frame_time_ns: 15_000_000,
            max_frame_time_ns: 18_000_000,
            dropped_frames: 0,
            total_frames: self.frame_counter.load(Ordering::Relaxed),
        }
    }
}

use core::sync::atomic::AtomicU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusiveError {
    AlreadyOwned,
    NotOwner,
    NotExclusive,
    InvalidParameter,
    HardwareUnsupported,
    InsufficientPermissions,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameLatencyStats {
    pub average_frame_time_ns: u64,
    pub min_frame_time_ns: u64,
    pub max_frame_time_ns: u64,
    pub dropped_frames: u64,
    pub total_frames: u64,
}

/// Security manager for exclusive display access
pub struct ExclusiveSecurityManager {
    allowed_processes: RwLock<BTreeMap<u32, ExclusivePermissions>>, // PID -> permissions
    audit_log: Mutex<Vec<SecurityEvent>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExclusivePermissions {
    pub can_request_exclusive: bool,
    pub can_set_refresh_rate: bool,
    pub can_bypass_vsync: bool,
    pub priority_level: u32,
}

#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub timestamp: u64,
    pub pid: u32,
    pub event_type: SecurityEventType,
    pub surface_id: Option<SurfaceId>,
}

#[derive(Debug, Clone)]
pub enum SecurityEventType {
    ExclusiveRequested,
    ExclusiveGranted,
    ExclusiveRevoked,
    UnauthorizedAccess,
    PolicyViolation,
}

impl ExclusiveSecurityManager {
    pub fn new() -> Self {
        ExclusiveSecurityManager {
            allowed_processes: RwLock::new(BTreeMap::new()),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    pub fn grant_permissions(&self, pid: u32, permissions: ExclusivePermissions) {
        self.allowed_processes.write().insert(pid, permissions);
        self.log_event(SecurityEvent {
            timestamp: 0, // Would use actual timestamp
            pid,
            event_type: SecurityEventType::ExclusiveGranted,
            surface_id: None,
        });
    }

    pub fn check_permission(&self, pid: u32, required: &str) -> bool {
        if let Some(perms) = self.allowed_processes.read().get(&pid) {
            match required {
                "request_exclusive" => perms.can_request_exclusive,
                "set_refresh_rate" => perms.can_set_refresh_rate,
                "bypass_vsync" => perms.can_bypass_vsync,
                _ => false,
            }
        } else {
            false
        }
    }

    fn log_event(&self, event: SecurityEvent) {
        let mut log = self.audit_log.lock();
        log.push(event);
        
        // Keep log size bounded
        if log.len() > 1000 {
            log.drain(0..100); // Remove oldest 100 entries
        }
    }
}