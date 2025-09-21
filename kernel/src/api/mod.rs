use alloc::{sync::Arc, vec::Vec};
use conquer_once::spin::Lazy;
use core::{
    sync::atomic::{AtomicU64, Ordering},
};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use spin::RwLock;

pub mod objects;
pub mod syscalls;
pub mod syscall_entry;
pub mod commands;

use objects::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectHandle(u64);

impl ObjectHandle {
    pub fn new() -> Self {
        static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
        ObjectHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

impl ProcessId {
    pub fn new() -> Self {
        static NEXT_PID: AtomicU64 = AtomicU64::new(1);
        ProcessId(NEXT_PID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub handle: ObjectHandle,
    pub rights: Rights,
    pub owner: ProcessId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u64);

impl Rights {
    pub const READ: Rights = Rights(1 << 0);
    pub const WRITE: Rights = Rights(1 << 1);
    pub const EXECUTE: Rights = Rights(1 << 2);
    pub const DELETE: Rights = Rights(1 << 3);
    pub const SHARE: Rights = Rights(1 << 4);
    pub const ALL: Rights = Rights(u64::MAX);

    pub fn has(&self, right: Rights) -> bool {
        self.0 & right.0 != 0
    }

    pub fn with(&self, right: Rights) -> Rights {
        Rights(self.0 | right.0)
    }

    pub fn without(&self, right: Rights) -> Rights {
        Rights(self.0 & !right.0)
    }
}

pub struct ObjectRegistry {
    objects: RwLock<HashMap<ObjectHandle, Arc<dyn KernelObject>>>,
    capabilities: RwLock<HashMap<ProcessId, Vec<Capability>>>,
}

impl ObjectRegistry {
    pub fn new() -> Self {
        ObjectRegistry {
            objects: RwLock::new(HashMap::new()),
            capabilities: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_object(
        &self,
        object: Arc<dyn KernelObject>,
        owner: ProcessId,
        rights: Rights,
    ) -> Result<ObjectHandle, RegistryError> {
        let handle = ObjectHandle::new();
        let capability = Capability {
            handle,
            rights,
            owner,
        };

        let mut objects = self.objects.write();
        let mut capabilities = self.capabilities.write();

        objects.insert(handle, object);

        match capabilities.entry(owner) {
            hashbrown::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(capability);
            }
            hashbrown::hash_map::Entry::Vacant(entry) => {
                let mut caps = Vec::new();
                caps.push(capability);
                entry.insert(caps);
            }
        }

        Ok(handle)
    }

    pub fn get_object(
        &self,
        handle: ObjectHandle,
        process: ProcessId,
        required_rights: Rights,
    ) -> Result<Arc<dyn KernelObject>, RegistryError> {
        let capabilities = self.capabilities.read();
        let process_caps = capabilities
            .get(&process)
            .ok_or(RegistryError::ProcessNotFound)?;

        let capability = process_caps
            .iter()
            .find(|cap| cap.handle == handle)
            .ok_or(RegistryError::HandleNotFound)?;

        if !capability.rights.has(required_rights) {
            return Err(RegistryError::InsufficientRights);
        }

        let objects = self.objects.read();
        objects
            .get(&handle)
            .cloned()
            .ok_or(RegistryError::ObjectNotFound)
    }

    pub fn transfer_capability(
        &self,
        handle: ObjectHandle,
        from: ProcessId,
        to: ProcessId,
        rights: Rights,
    ) -> Result<(), RegistryError> {
        let mut capabilities = self.capabilities.write();
        
        let from_caps = capabilities
            .get_mut(&from)
            .ok_or(RegistryError::ProcessNotFound)?;

        let cap_index = from_caps
            .iter()
            .position(|cap| cap.handle == handle)
            .ok_or(RegistryError::HandleNotFound)?;

        let original_cap = &from_caps[cap_index];
        if !original_cap.rights.has(Rights::SHARE) {
            return Err(RegistryError::InsufficientRights);
        }

        let new_capability = Capability {
            handle,
            rights: rights,
            owner: to,
        };

        match capabilities.entry(to) {
            hashbrown::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(new_capability);
            }
            hashbrown::hash_map::Entry::Vacant(entry) => {
                let mut caps = Vec::new();
                caps.push(new_capability);
                entry.insert(caps);
            }
        }

        Ok(())
    }

    pub fn revoke_capability(&self, handle: ObjectHandle, process: ProcessId) -> Result<(), RegistryError> {
        let mut capabilities = self.capabilities.write();
        let process_caps = capabilities
            .get_mut(&process)
            .ok_or(RegistryError::ProcessNotFound)?;

        let cap_index = process_caps
            .iter()
            .position(|cap| cap.handle == handle)
            .ok_or(RegistryError::HandleNotFound)?;

        process_caps.remove(cap_index);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    RegistryFull,
    ProcessNotFound,
    HandleNotFound,
    ObjectNotFound,
    InsufficientRights,
}

static OBJECT_REGISTRY: Lazy<RwLock<ObjectRegistry>> = Lazy::new(|| RwLock::new(ObjectRegistry::new()));

pub fn get_registry() -> &'static RwLock<ObjectRegistry> {
    &*OBJECT_REGISTRY
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcMessage {
    CreateSurface {
        width: u32,
        height: u32,
        format: PixelFormat,
    },
    AttachBuffer {
        surface: ObjectHandle,
        buffer: ObjectHandle,
    },
    CommitSurface {
        surface: ObjectHandle,
    },
    CreateEventStream,
    PollEvent {
        stream: ObjectHandle,
    },
    SetExclusive {
        display: ObjectHandle,
        exclusive: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Success,
    ObjectCreated { handle: ObjectHandle },
    Event { event: InputEvent },
    Error { code: u32, message: &'static str },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PixelFormat {
    Rgba8888,
    Rgb888,
    Bgra8888,
    Bgr888,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    KeyPress { key: u32, modifiers: u32 },
    KeyRelease { key: u32, modifiers: u32 },
    MouseMove { x: i32, y: i32 },
    MousePress { button: u32, x: i32, y: i32 },
    MouseRelease { button: u32, x: i32, y: i32 },
}