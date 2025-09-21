use alloc::{collections::VecDeque, sync::Arc};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use super::{process::*, SchedulingPolicy, ProcessState};
use crate::api::ProcessId;

pub struct CpuRunQueue {
    cpu_id: usize,
    ready_queue: Mutex<VecDeque<Arc<Process>>>,
    current_process: Mutex<Option<Arc<Process>>>,
    
    // Priority queues for different scheduling policies
    high_priority_queue: Mutex<VecDeque<Arc<Process>>>,
    normal_priority_queue: Mutex<VecDeque<Arc<Process>>>,
    low_priority_queue: Mutex<VecDeque<Arc<Process>>>,
    
    // Statistics
    load_average: AtomicU32,
    context_switches: AtomicU64,
    idle_ticks: AtomicU64,
    active_ticks: AtomicU64,
    last_tick: AtomicU64,
}

impl CpuRunQueue {
    pub fn new(cpu_id: usize) -> Self {
        CpuRunQueue {
            cpu_id,
            ready_queue: Mutex::new(VecDeque::new()),
            current_process: Mutex::new(None),
            high_priority_queue: Mutex::new(VecDeque::new()),
            normal_priority_queue: Mutex::new(VecDeque::new()),
            low_priority_queue: Mutex::new(VecDeque::new()),
            load_average: AtomicU32::new(0),
            context_switches: AtomicU64::new(0),
            idle_ticks: AtomicU64::new(0),
            active_ticks: AtomicU64::new(0),
            last_tick: AtomicU64::new(0),
        }
    }

    pub fn add_process(&self, process: Arc<Process>) {
        process.set_assigned_cpu(self.cpu_id);
        
        match process.priority() {
            0..=63 => self.low_priority_queue.lock().push_back(process),
            64..=127 => self.normal_priority_queue.lock().push_back(process),
            128..=255 => self.high_priority_queue.lock().push_back(process),
        }
        
        self.update_load();
    }

    pub fn remove_process(&self, pid: ProcessId) -> bool {
        // Remove from all queues
        let mut removed = false;
        
        let mut ready = self.ready_queue.lock();
        if let Some(pos) = ready.iter().position(|p| p.id() == pid) {
            ready.remove(pos);
            removed = true;
        }
        drop(ready);

        let mut high = self.high_priority_queue.lock();
        if let Some(pos) = high.iter().position(|p| p.id() == pid) {
            high.remove(pos);
            removed = true;
        }
        drop(high);

        let mut normal = self.normal_priority_queue.lock();
        if let Some(pos) = normal.iter().position(|p| p.id() == pid) {
            normal.remove(pos);
            removed = true;
        }
        drop(normal);

        let mut low = self.low_priority_queue.lock();
        if let Some(pos) = low.iter().position(|p| p.id() == pid) {
            low.remove(pos);
            removed = true;
        }
        drop(low);

        // Check if it's the currently running process
        let mut current = self.current_process.lock();
        if let Some(ref proc) = *current {
            if proc.id() == pid {
                *current = None;
                removed = true;
            }
        }

        if removed {
            self.update_load();
        }

        removed
    }

    pub fn get_next_process(&self, policy: &SchedulingPolicy, _current_tick: u64) -> Option<Arc<Process>> {
        match policy {
            SchedulingPolicy::RoundRobin => self.round_robin_schedule(),
            SchedulingPolicy::PriorityBased => self.priority_schedule(),
            SchedulingPolicy::RealTime => self.realtime_schedule(),
        }
    }

    pub fn preempt_current(&self) {
        let mut current = self.current_process.lock();
        if let Some(process) = current.take() {
            if process.state() == ProcessState::Running {
                process.set_state(ProcessState::Ready);
                self.add_process(process);
            }
        }
    }

    pub fn timer_tick(&self, tick: u64) {
        self.last_tick.store(tick, Ordering::Relaxed);
        
        // Update statistics
        if self.current_process.lock().is_some() {
            self.active_ticks.fetch_add(1, Ordering::Relaxed);
        } else {
            self.idle_ticks.fetch_add(1, Ordering::Relaxed);
        }
        
        // Age processes to prevent starvation
        self.age_processes();
    }

    pub fn steal_process(&self) -> Option<Arc<Process>> {
        // Try to steal from the lowest priority queue first
        if let Some(process) = self.low_priority_queue.lock().pop_front() {
            self.update_load();
            return Some(process);
        }
        
        if let Some(process) = self.normal_priority_queue.lock().pop_front() {
            self.update_load();
            return Some(process);
        }
        
        // Only steal from high priority if absolutely necessary
        if let Some(process) = self.high_priority_queue.lock().pop_front() {
            self.update_load();
            return Some(process);
        }
        
        None
    }

    pub fn get_load(&self) -> u32 {
        self.load_average.load(Ordering::Relaxed)
    }

    pub fn get_context_switches(&self) -> u64 {
        self.context_switches.load(Ordering::Relaxed)
    }

    pub fn cpu_id(&self) -> usize {
        self.cpu_id
    }

    pub fn get_utilization(&self) -> f32 {
        let active = self.active_ticks.load(Ordering::Relaxed) as f32;
        let idle = self.idle_ticks.load(Ordering::Relaxed) as f32;
        let total = active + idle;
        
        if total > 0.0 {
            active / total
        } else {
            0.0
        }
    }

    // Private helper methods

    fn round_robin_schedule(&self) -> Option<Arc<Process>> {
        // Check current process first
        let mut current = self.current_process.lock();
        if let Some(ref process) = *current {
            if process.state() == ProcessState::Running {
                // Current process can continue running
                return Some(process.clone());
            }
        }

        // Get next process from ready queue
        let next_process = self.get_next_ready_process();
        if let Some(ref process) = next_process {
            process.set_state(ProcessState::Running);
            process.increment_context_switches();
            *current = Some(process.clone());
            self.context_switches.fetch_add(1, Ordering::Relaxed);
        }

        next_process
    }

    fn priority_schedule(&self) -> Option<Arc<Process>> {
        // Check high priority queue first
        if let Some(process) = self.high_priority_queue.lock().pop_front() {
            process.set_state(ProcessState::Running);
            process.increment_context_switches();
            *self.current_process.lock() = Some(process.clone());
            self.context_switches.fetch_add(1, Ordering::Relaxed);
            return Some(process);
        }

        // Then normal priority
        if let Some(process) = self.normal_priority_queue.lock().pop_front() {
            process.set_state(ProcessState::Running);
            process.increment_context_switches();
            *self.current_process.lock() = Some(process.clone());
            self.context_switches.fetch_add(1, Ordering::Relaxed);
            return Some(process);
        }

        // Finally low priority
        if let Some(process) = self.low_priority_queue.lock().pop_front() {
            process.set_state(ProcessState::Running);
            process.increment_context_switches();
            *self.current_process.lock() = Some(process.clone());
            self.context_switches.fetch_add(1, Ordering::Relaxed);
            return Some(process);
        }

        None
    }

    fn realtime_schedule(&self) -> Option<Arc<Process>> {
        // Real-time scheduling: high priority processes get immediate execution
        // and are not preempted by lower priority processes
        let mut current = self.current_process.lock();
        
        if let Some(ref current_proc) = *current {
            if current_proc.priority() >= 128 && current_proc.state() == ProcessState::Running {
                // High priority process continues
                return Some(current_proc.clone());
            }
        }

        // Check for higher priority processes
        if let Some(process) = self.high_priority_queue.lock().pop_front() {
            process.set_state(ProcessState::Running);
            process.increment_context_switches();
            *current = Some(process.clone());
            self.context_switches.fetch_add(1, Ordering::Relaxed);
            return Some(process);
        }

        // Continue with current if no higher priority available
        if let Some(ref current_proc) = *current {
            if current_proc.state() == ProcessState::Running {
                return Some(current_proc.clone());
            }
        }

        // Get next available process
        self.get_next_ready_process()
    }

    fn get_next_ready_process(&self) -> Option<Arc<Process>> {
        // Try each priority level
        if let Some(process) = self.high_priority_queue.lock().pop_front() {
            return Some(process);
        }
        
        if let Some(process) = self.normal_priority_queue.lock().pop_front() {
            return Some(process);
        }
        
        if let Some(process) = self.low_priority_queue.lock().pop_front() {
            return Some(process);
        }
        
        // Fallback to ready queue
        self.ready_queue.lock().pop_front()
    }

    fn update_load(&self) {
        let high_count = self.high_priority_queue.lock().len();
        let normal_count = self.normal_priority_queue.lock().len();
        let low_count = self.low_priority_queue.lock().len();
        let ready_count = self.ready_queue.lock().len();
        
        let total_load = high_count + normal_count + low_count + ready_count;
        let current_load = if self.current_process.lock().is_some() { 1 } else { 0 };
        
        self.load_average.store((total_load + current_load) as u32, Ordering::Relaxed);
    }

    fn age_processes(&self) {
        // Prevent starvation by occasionally promoting lower priority processes
        // This is a simplified aging mechanism
        let mut low = self.low_priority_queue.lock();
        let mut normal = self.normal_priority_queue.lock();
        
        // Promote one low priority process to normal every 100 ticks
        if self.last_tick.load(Ordering::Relaxed) % 100 == 0 {
            if let Some(process) = low.pop_front() {
                normal.push_back(process);
            }
        }
    }
}