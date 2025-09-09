use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::{Mutex, RwLock};
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{PhysFrame, PageTable},
    PhysAddr, VirtAddr,
};

pub mod process;
pub mod cpu;
pub mod smp;
pub mod switch;

use process::*;
use cpu::*;

/// Global scheduler instance
static SCHEDULER: Scheduler = Scheduler::new();

pub fn scheduler() -> &'static Scheduler {
    &SCHEDULER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    RoundRobin,
    PriorityBased,
    RealTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Zombie,
}

pub struct Scheduler {
    /// Per-CPU run queues for SMP support
    per_cpu_queues: RwLock<Vec<CpuRunQueue>>,
    
    /// Global process table
    processes: RwLock<Vec<Option<Arc<Process>>>>,
    
    /// Current tick counter for scheduling decisions
    tick_counter: AtomicU64,
    
    /// Number of active CPUs
    cpu_count: AtomicUsize,
    
    /// Scheduler configuration
    policy: RwLock<SchedulingPolicy>,
    time_slice_ticks: AtomicU32,
    
    /// Load balancing
    load_balance_enabled: AtomicBool,
    last_balance_tick: AtomicU64,
}

impl Scheduler {
    const fn new() -> Self {
        Scheduler {
            per_cpu_queues: RwLock::new(Vec::new()),
            processes: RwLock::new(Vec::new()),
            tick_counter: AtomicU64::new(0),
            cpu_count: AtomicUsize::new(1),
            policy: RwLock::new(SchedulingPolicy::RoundRobin),
            time_slice_ticks: AtomicU32::new(10), // 10ms default time slice
            load_balance_enabled: AtomicBool::new(true),
            last_balance_tick: AtomicU64::new(0),
        }
    }

    /// Initialize the scheduler for SMP
    pub fn init(&self, cpu_count: usize) {
        self.cpu_count.store(cpu_count, Ordering::Relaxed);
        
        let mut queues = self.per_cpu_queues.write();
        queues.clear();
        for cpu_id in 0..cpu_count {
            queues.push(CpuRunQueue::new(cpu_id));
        }
    }

    /// Create a new process
    pub fn create_process(&self, entry_point: VirtAddr, priority: u8) -> Result<ProcessId, SchedulerError> {
        let process = Arc::new(Process::new(entry_point, priority)?);
        let pid = process.id();
        
        let mut processes = self.processes.write();
        
        // Find empty slot or expand the table
        let slot_index = if let Some(index) = processes.iter().position(|p| p.is_none()) {
            processes[index] = Some(process.clone());
            index
        } else {
            let index = processes.len();
            processes.push(Some(process.clone()));
            index
        };

        // Add to appropriate CPU run queue (load balancing)
        let target_cpu = self.select_cpu_for_new_process();
        let mut queues = self.per_cpu_queues.write();
        queues[target_cpu].add_process(process);

        Ok(pid)
    }

    /// Schedule next process on current CPU
    pub fn schedule(&self) -> Option<Arc<Process>> {
        let cpu_id = self.current_cpu_id();
        let tick = self.tick_counter.fetch_add(1, Ordering::Relaxed);
        
        // Check if we need to do load balancing
        if self.should_balance_load(tick) {
            self.balance_load(cpu_id, tick);
        }

        let queues = self.per_cpu_queues.read();
        if cpu_id < queues.len() {
            queues[cpu_id].get_next_process(&*self.policy.read(), tick)
        } else {
            None
        }
    }

    /// Mark process as blocked (I/O wait, etc.)
    pub fn block_process(&self, pid: ProcessId, reason: BlockReason) {
        if let Some(process) = self.get_process(pid) {
            process.set_state(ProcessState::Blocked);
            process.set_block_reason(Some(reason));
            
            // Remove from run queue
            let cpu_id = process.assigned_cpu();
            let mut queues = self.per_cpu_queues.write();
            if cpu_id < queues.len() {
                queues[cpu_id].remove_process(pid);
            }
        }
    }

    /// Unblock process and add back to run queue
    pub fn unblock_process(&self, pid: ProcessId) {
        if let Some(process) = self.get_process(pid) {
            if process.state() == ProcessState::Blocked {
                process.set_state(ProcessState::Ready);
                process.set_block_reason(None);
                
                // Add back to run queue
                let target_cpu = self.select_cpu_for_process(pid);
                process.set_assigned_cpu(target_cpu);
                
                let mut queues = self.per_cpu_queues.write();
                if target_cpu < queues.len() {
                    queues[target_cpu].add_process(process);
                }
            }
        }
    }

    /// Terminate process
    pub fn terminate_process(&self, pid: ProcessId, exit_code: i32) {
        if let Some(process) = self.get_process(pid) {
            process.set_state(ProcessState::Zombie);
            process.set_exit_code(Some(exit_code));
            
            // Remove from all run queues
            let mut queues = self.per_cpu_queues.write();
            for queue in queues.iter_mut() {
                queue.remove_process(pid);
            }
            
            // Clean up resources
            // TODO: Free memory, close file descriptors, etc.
        }
    }

    /// Get process by ID
    pub fn get_process(&self, pid: ProcessId) -> Option<Arc<Process>> {
        let processes = self.processes.read();
        for process_slot in processes.iter() {
            if let Some(process) = process_slot {
                if process.id() == pid {
                    return Some(process.clone());
                }
            }
        }
        None
    }

    /// Timer tick handler - called on each timer interrupt
    pub fn timer_tick(&self, cpu_id: usize) {
        let tick = self.tick_counter.fetch_add(1, Ordering::Relaxed);
        
        // Update CPU-local statistics
        let queues = self.per_cpu_queues.read();
        if cpu_id < queues.len() {
            queues[cpu_id].timer_tick(tick);
        }
        
        // Trigger preemption if time slice expired
        if tick % self.time_slice_ticks.load(Ordering::Relaxed) as u64 == 0 {
            self.preempt_current_process(cpu_id);
        }
    }

    /// Force preemption of current process on CPU
    pub fn preempt_current_process(&self, cpu_id: usize) {
        let queues = self.per_cpu_queues.read();
        if cpu_id < queues.len() {
            queues[cpu_id].preempt_current();
        }
    }

    /// Get scheduling statistics
    pub fn get_stats(&self) -> SchedulerStats {
        let processes = self.processes.read();
        let mut ready_count = 0;
        let mut running_count = 0;
        let mut blocked_count = 0;
        let mut zombie_count = 0;

        for process_slot in processes.iter() {
            if let Some(process) = process_slot {
                match process.state() {
                    ProcessState::Ready => ready_count += 1,
                    ProcessState::Running => running_count += 1,
                    ProcessState::Blocked => blocked_count += 1,
                    ProcessState::Zombie => zombie_count += 1,
                }
            }
        }

        let queues = self.per_cpu_queues.read();
        let mut cpu_loads = Vec::new();
        for queue in queues.iter() {
            cpu_loads.push(queue.get_load());
        }

        SchedulerStats {
            total_processes: processes.len(),
            ready_processes: ready_count,
            running_processes: running_count,
            blocked_processes: blocked_count,
            zombie_processes: zombie_count,
            cpu_loads,
            total_ticks: self.tick_counter.load(Ordering::Relaxed),
            context_switches: self.get_total_context_switches(),
        }
    }

    /// Configure scheduler policy and parameters
    pub fn configure(&self, policy: SchedulingPolicy, time_slice_ms: u32) {
        *self.policy.write() = policy;
        self.time_slice_ticks.store(time_slice_ms, Ordering::Relaxed);
    }

    // Private helper methods

    fn current_cpu_id(&self) -> usize {
        // Get current CPU ID from local APIC or similar
        // For now, return 0 for single-core
        0
    }

    fn select_cpu_for_new_process(&self) -> usize {
        if !self.load_balance_enabled.load(Ordering::Relaxed) {
            return 0;
        }

        // Find CPU with lowest load
        let queues = self.per_cpu_queues.read();
        let mut min_load = u32::MAX;
        let mut best_cpu = 0;

        for (cpu_id, queue) in queues.iter().enumerate() {
            let load = queue.get_load();
            if load < min_load {
                min_load = load;
                best_cpu = cpu_id;
            }
        }

        best_cpu
    }

    fn select_cpu_for_process(&self, pid: ProcessId) -> usize {
        // For now, simple round-robin CPU assignment
        // In a real implementation, consider CPU affinity, cache locality, etc.
        pid.as_u64() as usize % self.cpu_count.load(Ordering::Relaxed)
    }

    fn should_balance_load(&self, current_tick: u64) -> bool {
        if !self.load_balance_enabled.load(Ordering::Relaxed) {
            return false;
        }

        let last_balance = self.last_balance_tick.load(Ordering::Relaxed);
        current_tick.saturating_sub(last_balance) >= 1000 // Balance every 1000 ticks
    }

    fn balance_load(&self, current_cpu: usize, current_tick: u64) {
        self.last_balance_tick.store(current_tick, Ordering::Relaxed);
        
        let mut queues = self.per_cpu_queues.write();
        if current_cpu >= queues.len() {
            return;
        }

        // Simple load balancing: move processes from overloaded CPUs to underloaded ones
        let mut loads: Vec<(usize, u32)> = queues
            .iter()
            .enumerate()
            .map(|(cpu_id, queue)| (cpu_id, queue.get_load()))
            .collect();

        loads.sort_by_key(|(_, load)| *load);

        // Move processes from most loaded to least loaded CPU
        if loads.len() >= 2 {
            let (least_loaded_cpu, min_load) = loads[0];
            let (most_loaded_cpu, max_load) = loads[loads.len() - 1];

            if max_load > min_load + 2 {
                // Move one process from most loaded to least loaded
                if let Some(process) = queues[most_loaded_cpu].steal_process() {
                    process.set_assigned_cpu(least_loaded_cpu);
                    queues[least_loaded_cpu].add_process(process);
                }
            }
        }
    }

    fn get_total_context_switches(&self) -> u64 {
        let queues = self.per_cpu_queues.read();
        queues.iter().map(|q| q.get_context_switches()).sum()
    }
}

#[derive(Debug)]
pub struct SchedulerStats {
    pub total_processes: usize,
    pub ready_processes: usize,
    pub running_processes: usize,
    pub blocked_processes: usize,
    pub zombie_processes: usize,
    pub cpu_loads: Vec<u32>,
    pub total_ticks: u64,
    pub context_switches: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    OutOfMemory,
    InvalidCpuId,
    ProcessNotFound,
    InvalidState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    IoWait,
    MutexWait,
    SemaphoreWait,
    Sleep,
    PageFault,
    IpcWait,
}

/// Initialize the global scheduler
pub fn init_scheduler(cpu_count: usize) {
    scheduler().init(cpu_count);
}

/// Create a new process
pub fn spawn_process(entry_point: VirtAddr, priority: u8) -> Result<ProcessId, SchedulerError> {
    scheduler().create_process(entry_point, priority)
}

/// Schedule next process (called from timer interrupt)
pub fn schedule_next() -> Option<Arc<Process>> {
    scheduler().schedule()
}

/// Timer tick handler
pub fn scheduler_tick(cpu_id: usize) {
    scheduler().timer_tick(cpu_id);
}