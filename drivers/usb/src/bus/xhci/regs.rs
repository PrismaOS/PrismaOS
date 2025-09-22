//! xHCI Register Block Definitions
//!
//! This file defines the memory-mapped register layout for the xHCI controller, as per the xHCI spec.
//! All offsets and bitfields are documented for OSDev and production use.

use xhci::registers as xhci_regs;

pub type CapabilityRegisters<M> = xhci_regs::capability::Capability<M>;
pub type OperationalRegisters<M> = xhci_regs::operational::Operational<M>;
pub type RuntimeRegisters<M> = xhci_regs::runtime::Runtime<M>;
pub type Doorbell<M> = xhci_regs::doorbell::Doorbell<M>;
