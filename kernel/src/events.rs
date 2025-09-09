use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use crate::api::{InputEvent, ObjectHandle, ProcessId, get_registry};

/// Global event dispatcher for routing events to userspace
pub struct EventDispatcher {
    /// Registered event streams that should receive events
    streams: RwLock<Vec<EventStreamRegistration>>,
    
    /// Global event counter for debugging
    event_counter: AtomicU64,
    
    /// Pending events that haven't been delivered yet
    pending_events: Mutex<VecDeque<PendingEvent>>,
}

#[derive(Debug, Clone)]
struct EventStreamRegistration {
    stream_handle: ObjectHandle,
    process_id: ProcessId,
    event_types: EventTypeFilter,
}

#[derive(Debug, Clone)]
struct PendingEvent {
    event: InputEvent,
    target_processes: Vec<ProcessId>,
    timestamp: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct EventTypeFilter {
    keyboard: bool,
    mouse: bool,
    display: bool,
}

impl EventTypeFilter {
    pub const ALL: Self = EventTypeFilter {
        keyboard: true,
        mouse: true,
        display: true,
    };
    
    pub const KEYBOARD_ONLY: Self = EventTypeFilter {
        keyboard: true,
        mouse: false,
        display: false,
    };
    
    pub const MOUSE_ONLY: Self = EventTypeFilter {
        keyboard: false,
        mouse: true,
        display: false,
    };

    pub fn matches(&self, event: &InputEvent) -> bool {
        match event {
            InputEvent::KeyPress { .. } | InputEvent::KeyRelease { .. } => self.keyboard,
            InputEvent::MouseMove { .. } | InputEvent::MousePress { .. } | InputEvent::MouseRelease { .. } => self.mouse,
        }
    }
}

impl EventDispatcher {
    pub const fn new() -> Self {
        EventDispatcher {
            streams: RwLock::new(Vec::new()),
            event_counter: AtomicU64::new(0),
            pending_events: Mutex::new(VecDeque::new()),
        }
    }

    /// Register an event stream to receive events
    pub fn register_stream(
        &self,
        stream_handle: ObjectHandle,
        process_id: ProcessId,
        filter: EventTypeFilter,
    ) {
        let registration = EventStreamRegistration {
            stream_handle,
            process_id,
            event_types: filter,
        };
        
        self.streams.write().push(registration);
    }

    /// Unregister an event stream
    pub fn unregister_stream(&self, stream_handle: ObjectHandle, process_id: ProcessId) {
        let mut streams = self.streams.write();
        streams.retain(|reg| reg.stream_handle != stream_handle || reg.process_id != process_id);
    }

    /// Dispatch an event to all registered streams that match the filter
    pub fn dispatch_event(&self, event: InputEvent) {
        let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
        
        let streams = self.streams.read();
        let mut target_processes = Vec::new();
        
        // Find all processes that should receive this event
        for registration in streams.iter() {
            if registration.event_types.matches(&event) {
                target_processes.push(registration.process_id);
            }
        }
        
        drop(streams);
        
        if !target_processes.is_empty() {
            // Add to pending events for async delivery
            let pending = PendingEvent {
                event: event.clone(),
                target_processes: target_processes.clone(),
                timestamp: crate::time::current_tick(),
            };
            
            self.pending_events.lock().push_back(pending);
            
            // Immediately try to deliver to event streams
            self.deliver_to_processes(&event, &target_processes);
        }
    }

    /// Deliver event to specific processes immediately
    fn deliver_to_processes(&self, event: &InputEvent, target_processes: &[ProcessId]) {
        let registry = get_registry();
        let streams = self.streams.read();
        
        for &process_id in target_processes {
            // Find the event stream for this process
            for registration in streams.iter() {
                if registration.process_id == process_id {
                    // Get the event stream object
                    let api_process_id = process_id;
                    if let Ok(stream_obj) = registry.get_object(
                        registration.stream_handle,
                        api_process_id,
                        crate::api::Rights::WRITE,
                    ) {
                        // Cast to EventStream and push the event
                        if let Some(event_stream) = stream_obj.as_any().downcast_ref::<crate::api::objects::EventStream>() {
                            event_stream.push_event(event.clone());
                        }
                    }
                    break;
                }
            }
        }
    }

    /// Process pending events (called periodically)
    pub fn process_pending_events(&self) {
        let mut pending = self.pending_events.lock();
        
        // For now, just clear pending events since we deliver immediately
        // In a real implementation, this would retry failed deliveries
        pending.clear();
    }

    /// Get event dispatcher statistics
    pub fn get_stats(&self) -> EventStats {
        let streams = self.streams.read();
        let pending = self.pending_events.lock();
        
        EventStats {
            total_events_dispatched: self.event_counter.load(Ordering::Relaxed),
            registered_streams: streams.len(),
            pending_events: pending.len(),
        }
    }
}

#[derive(Debug)]
pub struct EventStats {
    pub total_events_dispatched: u64,
    pub registered_streams: usize,
    pub pending_events: usize,
}

/// Global event dispatcher instance
static EVENT_DISPATCHER: EventDispatcher = EventDispatcher::new();

pub fn event_dispatcher() -> &'static EventDispatcher {
    &EVENT_DISPATCHER
}

/// Convenience functions for common event operations

pub fn register_keyboard_events(stream_handle: ObjectHandle, process_id: ProcessId) {
    event_dispatcher().register_stream(stream_handle, process_id, EventTypeFilter::KEYBOARD_ONLY);
}

pub fn register_mouse_events(stream_handle: ObjectHandle, process_id: ProcessId) {
    event_dispatcher().register_stream(stream_handle, process_id, EventTypeFilter::MOUSE_ONLY);
}

pub fn register_all_input_events(stream_handle: ObjectHandle, process_id: ProcessId) {
    event_dispatcher().register_stream(stream_handle, process_id, EventTypeFilter::ALL);
}

pub fn dispatch_key_press(key: u32, modifiers: u32) {
    let event = InputEvent::KeyPress { key, modifiers };
    event_dispatcher().dispatch_event(event);
}

pub fn dispatch_key_release(key: u32, modifiers: u32) {
    let event = InputEvent::KeyRelease { key, modifiers };
    event_dispatcher().dispatch_event(event);
}

pub fn dispatch_mouse_move(x: i32, y: i32) {
    let event = InputEvent::MouseMove { x, y };
    event_dispatcher().dispatch_event(event);
}

pub fn dispatch_mouse_press(button: u32, x: i32, y: i32) {
    let event = InputEvent::MousePress { button, x, y };
    event_dispatcher().dispatch_event(event);
}

pub fn dispatch_mouse_release(button: u32, x: i32, y: i32) {
    let event = InputEvent::MouseRelease { button, x, y };
    event_dispatcher().dispatch_event(event);
}