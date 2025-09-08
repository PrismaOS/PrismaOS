use core::sync::atomic::{AtomicU64, Ordering};

static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn current_tick() -> u64 {
    SYSTEM_TICKS.load(Ordering::Relaxed)
}

pub fn increment_tick() {
    SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn ticks_to_ms(ticks: u64) -> u64 {
    // Assuming 1000 ticks per second (1ms per tick)
    ticks
}

pub fn ms_to_ticks(ms: u64) -> u64 {
    ms
}