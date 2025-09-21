//! Transaction system for GalleonFS
//! 
//! Provides atomic operations and consistency guarantees

use alloc::{vec::Vec, collections::BTreeMap};
use alloc::format;
use core::sync::atomic::{AtomicU64, Ordering};
use super::{Result, ObjectId, Inode};

/// Transaction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(pub u64);

impl TransactionId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        TransactionId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
    pub fn get(&self) -> u64 {
        self.0
    }
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Active,
    Preparing,
    Committed,
    Aborted,
}

/// Transaction operation type
#[derive(Debug, Clone)]
pub enum TransactionOperation {
    CreateInode { id: ObjectId, inode: Inode },
    UpdateInode { id: ObjectId, old_inode: Inode, new_inode: Inode },
    DeleteInode { id: ObjectId, inode: Inode },
    WriteData { id: ObjectId, offset: u64, data: Vec<u8> },
    TruncateData { id: ObjectId, old_size: u64, new_size: u64 },
    AllocateSpace { offset: u64, size: u64 },
    DeallocateSpace { offset: u64, size: u64 },
}

/// Transaction implementation
#[derive(Debug, Clone)]
pub struct Transaction {
    id: TransactionId,
    state: TransactionState,
    operations: Vec<TransactionOperation>,
    locks: Vec<ObjectId>,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            id: TransactionId::new(),
            state: TransactionState::Active,
            operations: Vec::new(),
            locks: Vec::new(),
        }
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    pub fn state(&self) -> TransactionState {
        self.state
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, TransactionState::Active)
    }

    pub fn add_operation(&mut self, operation: TransactionOperation) {
        if self.is_active() {
            self.operations.push(operation);
        }
    }

    pub fn add_lock(&mut self, object_id: ObjectId) {
        if self.is_active() && !self.locks.contains(&object_id) {
            self.locks.push(object_id);
        }
    }

    pub fn operations(&self) -> &[TransactionOperation] {
        &self.operations
    }

    pub fn locks(&self) -> &[ObjectId] {
        &self.locks
    }

    /// Prepare the transaction for commit
    pub async fn prepare(&mut self) -> Result<()> {
        if !self.is_active() {
            return Err(super::GalleonError::InvalidState(
                "Transaction is not active".into()
            ));
        }

        self.state = TransactionState::Preparing;
        
        // TODO: Implement two-phase commit preparation
        // - Validate all operations
        // - Acquire all necessary locks
        // - Prepare storage backends
        
        Ok(())
    }

    /// Commit the transaction
    pub async fn commit(mut self) -> Result<()> {
        match self.state {
            TransactionState::Active => {
                self.prepare().await?;
            }
            TransactionState::Preparing => {
                // Already prepared
            }
            _ => {
                return Err(super::GalleonError::InvalidStateDynamic(
                    format!("Cannot commit transaction in state {:?}", self.state)
                ));
            }
        }

        self.state = TransactionState::Committed;
        
        // TODO: Implement actual commit logic
        // - Apply all operations atomically
        // - Update storage backends
        // - Release locks
        
        Ok(())
    }

    /// Abort the transaction
    pub async fn abort(mut self) -> Result<()> {
        if matches!(self.state, TransactionState::Committed) {
            return Err(super::GalleonError::InvalidState(
                "Cannot abort committed transaction".into()
            ));
        }

        self.state = TransactionState::Aborted;
        
        // TODO: Implement rollback logic
        // - Undo all operations
        // - Release locks
        // - Clean up resources
        
        Ok(())
    }
}

/// Transaction manager for coordinating transactions
pub struct TransactionManager {
    active_transactions: spin::Mutex<BTreeMap<TransactionId, Transaction>>,
    lock_manager: LockManager,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            active_transactions: spin::Mutex::new(BTreeMap::new()),
            lock_manager: LockManager::new(),
        }
    }

    pub fn begin_transaction(&self) -> Transaction {
        let transaction = Transaction::new();
        let mut active = self.active_transactions.lock();
    active.insert(transaction.id(), transaction.clone());
        transaction
    }

    pub async fn commit_transaction(&self, transaction: Transaction) -> Result<()> {
        let tid = transaction.id();
        let locks = transaction.locks().to_vec();
        let result = transaction.commit().await;
        // Remove from active transactions
        let mut active = self.active_transactions.lock();
        active.remove(&tid);
        // Release locks
        for object_id in locks {
            self.lock_manager.release_lock(object_id, tid).await?;
        }
        result
    }

    pub async fn abort_transaction(&self, transaction: Transaction) -> Result<()> {
        let tid = transaction.id();
        let locks = transaction.locks().to_vec();
        let result = transaction.abort().await;
        // Remove from active transactions
        let mut active = self.active_transactions.lock();
        active.remove(&tid);
        // Release locks
        for object_id in locks {
            self.lock_manager.release_lock(object_id, tid).await?;
        }
        result
    }

    pub async fn acquire_lock(&self, object_id: ObjectId, transaction_id: TransactionId, lock_type: LockType) -> Result<()> {
        self.lock_manager.acquire_lock(object_id, transaction_id, lock_type).await
    }
}

/// Lock types for concurrency control
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockType {
    Shared,
    Exclusive,
}

/// Lock information
#[derive(Debug, Clone)]
struct LockInfo {
    transaction_id: TransactionId,
    lock_type: LockType,
}

/// Lock manager for handling concurrent access
pub struct LockManager {
    locks: spin::Mutex<BTreeMap<ObjectId, Vec<LockInfo>>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: spin::Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn acquire_lock(&self, object_id: ObjectId, transaction_id: TransactionId, lock_type: LockType) -> Result<()> {
        loop {
            {
                let mut locks = self.locks.lock();
                let object_locks = locks.entry(object_id).or_insert_with(Vec::new);
                
                // Check if lock can be acquired
                if self.can_acquire_lock(object_locks, lock_type) {
                    object_locks.push(LockInfo {
                        transaction_id,
                        lock_type,
                    });
                    return Ok(());
                }
            }
            
            // TODO: Implement proper waiting mechanism instead of busy loop
            // In a real implementation, this would use async waiting
            core::future::ready(()).await;
        }
    }

    pub async fn release_lock(&self, object_id: ObjectId, transaction_id: TransactionId) -> Result<()> {
        let mut locks = self.locks.lock();
        
        if let Some(object_locks) = locks.get_mut(&object_id) {
            object_locks.retain(|lock| lock.transaction_id != transaction_id);
            
            // Remove empty entries
            if object_locks.is_empty() {
                locks.remove(&object_id);
            }
        }
        
        Ok(())
    }

    fn can_acquire_lock(&self, existing_locks: &[LockInfo], requested_type: LockType) -> bool {
        if existing_locks.is_empty() {
            return true;
        }

        match requested_type {
            LockType::Shared => {
                // Shared locks are compatible with other shared locks
                existing_locks.iter().all(|lock| matches!(lock.lock_type, LockType::Shared))
            }
            LockType::Exclusive => {
                // Exclusive locks are incompatible with any other locks
                false
            }
        }
    }

    pub fn get_lock_holders(&self, object_id: ObjectId) -> Vec<TransactionId> {
        let locks = self.locks.lock();
        
        locks.get(&object_id)
            .map(|object_locks| {
                object_locks.iter()
                    .map(|lock| lock.transaction_id)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Savepoint for nested transactions
#[derive(Debug, Clone)]
pub struct Savepoint {
    id: u64,
    transaction_id: TransactionId,
    operation_count: usize,
}

impl Savepoint {
    pub fn new(transaction_id: TransactionId, operation_count: usize) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            transaction_id,
            operation_count,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    pub fn operation_count(&self) -> usize {
        self.operation_count
    }
}

/// Extended transaction with savepoint support
pub struct ExtendedTransaction {
    base_transaction: Transaction,
    savepoints: Vec<Savepoint>,
}

impl ExtendedTransaction {
    pub fn new() -> Self {
        Self {
            base_transaction: Transaction::new(),
            savepoints: Vec::new(),
        }
    }

    pub fn create_savepoint(&mut self) -> Savepoint {
        let savepoint = Savepoint::new(
            self.base_transaction.id(),
            self.base_transaction.operations().len()
        );
        self.savepoints.push(savepoint.clone());
        savepoint
    }

    pub async fn rollback_to_savepoint(&mut self, savepoint_id: u64) -> Result<()> {
        // Find the savepoint
        let savepoint_index = self.savepoints.iter()
            .position(|sp| sp.id() == savepoint_id)
            .ok_or_else(|| super::GalleonError::InvalidArgument("Savepoint not found".into()))?;

        let savepoint = &self.savepoints[savepoint_index];
        
        // Truncate operations to savepoint
        self.base_transaction.operations.truncate(savepoint.operation_count());
        
        // Remove newer savepoints
        self.savepoints.truncate(savepoint_index + 1);
        
        Ok(())
    }

    pub async fn release_savepoint(&mut self, savepoint_id: u64) -> Result<()> {
        let savepoint_index = self.savepoints.iter()
            .position(|sp| sp.id() == savepoint_id)
            .ok_or_else(|| super::GalleonError::InvalidArgument("Savepoint not found".into()))?;

        self.savepoints.remove(savepoint_index);
        Ok(())
    }

    pub fn transaction(&self) -> &Transaction {
        &self.base_transaction
    }

    pub fn transaction_mut(&mut self) -> &mut Transaction {
        &mut self.base_transaction
    }
}

/// Deadlock detection and resolution
pub struct DeadlockDetector {
    wait_graph: spin::Mutex<BTreeMap<TransactionId, Vec<TransactionId>>>,
}

impl DeadlockDetector {
    pub fn new() -> Self {
        Self {
            wait_graph: spin::Mutex::new(BTreeMap::new()),
        }
    }

    pub fn add_wait_edge(&self, waiting: TransactionId, holding: TransactionId) {
        let mut graph = self.wait_graph.lock();
        graph.entry(waiting).or_insert_with(Vec::new).push(holding);
    }

    pub fn remove_wait_edge(&self, waiting: TransactionId, holding: TransactionId) {
        let mut graph = self.wait_graph.lock();
        if let Some(waiters) = graph.get_mut(&waiting) {
            waiters.retain(|&id| id != holding);
            if waiters.is_empty() {
                graph.remove(&waiting);
            }
        }
    }

    pub fn detect_deadlock(&self) -> Option<Vec<TransactionId>> {
        let graph = self.wait_graph.lock();
        
        // Simple cycle detection using DFS
        for &start_node in graph.keys() {
            if let Some(cycle) = self.find_cycle(&graph, start_node) {
                return Some(cycle);
            }
        }
        
        None
    }

    fn find_cycle(&self, graph: &BTreeMap<TransactionId, Vec<TransactionId>>, start: TransactionId) -> Option<Vec<TransactionId>> {
        let mut visited = BTreeMap::new();
        let mut path = Vec::new();
        
        self.dfs_cycle_detection(graph, start, &mut visited, &mut path, start)
    }

    fn dfs_cycle_detection(
        &self,
        graph: &BTreeMap<TransactionId, Vec<TransactionId>>,
        current: TransactionId,
        visited: &mut BTreeMap<TransactionId, bool>,
        path: &mut Vec<TransactionId>,
        target: TransactionId,
    ) -> Option<Vec<TransactionId>> {
        if path.len() > 1 && current == target {
            return Some(path.clone());
        }

        if visited.get(&current).copied().unwrap_or(false) {
            return None;
        }

        visited.insert(current, true);
        path.push(current);

        if let Some(neighbors) = graph.get(&current) {
            for &neighbor in neighbors {
                if let Some(cycle) = self.dfs_cycle_detection(graph, neighbor, visited, path, target) {
                    return Some(cycle);
                }
            }
        }

        path.pop();
        None
    }
}