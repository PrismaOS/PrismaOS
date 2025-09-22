//! USB Hub Support

use alloc::{vec::Vec, boxed::Box, sync::Arc};
use core::{
    fmt,
    sync::atomic::{AtomicU8, AtomicU16, Ordering},
};
use spin::Mutex;
use crate::{
    Result, UsbDriverError,
    device::{UsbDevice, DeviceSpeed},
    transfer::{Transfer, SetupPacket, TransferBuffer},
};

/// USB Hub Port Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortStatus {
    /// Current connect status
    pub connection: bool,
    /// Port enabled
    pub enabled: bool,
    /// Suspend status
    pub suspended: bool,
    /// Over-current indicator
    pub over_current: bool,
    /// Reset status
    pub reset: bool,
    /// Port power status
    pub power: bool,
    /// Low-speed device attached
    pub low_speed: bool,
    /// High-speed device attached
    pub high_speed: bool,
    /// Test mode
    pub test: bool,
    /// Port indicator control
    pub indicator: bool,
}

impl PortStatus {
    /// Create empty port status
    pub fn empty() -> Self {
        Self {
            connection: false,
            enabled: false,
            suspended: false,
            over_current: false,
            reset: false,
            power: false,
            low_speed: false,
            high_speed: false,
            test: false,
            indicator: false,
        }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.connection
    }

    /// Check if port is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get device speed
    pub fn device_speed(&self) -> DeviceSpeed {
        if self.high_speed {
            DeviceSpeed::High
        } else if self.low_speed {
            DeviceSpeed::Low
        } else {
            DeviceSpeed::Full
        }
    }
}

/// Port Status Change bits
#[derive(Debug, Clone, Copy)]
pub struct PortStatusChange {
    pub connection_change: bool,
    pub enable_change: bool,
    pub suspend_change: bool,
    pub over_current_change: bool,
    pub reset_change: bool,
}

impl PortStatusChange {
    pub fn empty() -> Self {
        Self {
            connection_change: false,
            enable_change: false,
            suspend_change: false,
            over_current_change: false,
            reset_change: false,
        }
    }

    pub fn has_changes(&self) -> bool {
        self.connection_change ||
        self.enable_change ||
        self.suspend_change ||
        self.over_current_change ||
        self.reset_change
    }
}

/// Hub Feature Selectors
pub mod hub_features {
    pub const C_HUB_LOCAL_POWER: u16 = 0;
    pub const C_HUB_OVER_CURRENT: u16 = 1;
    pub const PORT_CONNECTION: u16 = 0;
    pub const PORT_ENABLE: u16 = 1;
    pub const PORT_SUSPEND: u16 = 2;
    pub const PORT_OVER_CURRENT: u16 = 3;
    pub const PORT_RESET: u16 = 4;
    pub const PORT_POWER: u16 = 8;
    pub const PORT_LOW_SPEED: u16 = 9;
    pub const C_PORT_CONNECTION: u16 = 16;
    pub const C_PORT_ENABLE: u16 = 17;
    pub const C_PORT_SUSPEND: u16 = 18;
    pub const C_PORT_OVER_CURRENT: u16 = 19;
    pub const C_PORT_RESET: u16 = 20;
    pub const PORT_TEST: u16 = 21;
    pub const PORT_INDICATOR: u16 = 22;
}

/// Hub Class Requests
pub mod hub_requests {
    pub const GET_STATUS: u8 = 0x00;
    pub const CLEAR_FEATURE: u8 = 0x01;
    pub const SET_FEATURE: u8 = 0x03;
    pub const GET_DESCRIPTOR: u8 = 0x06;
    pub const SET_DESCRIPTOR: u8 = 0x07;
    pub const GET_BUS_STATE: u8 = 0x02;
    pub const SET_HUB_DEPTH: u8 = 0x0C;
}

/// Hub Descriptor
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HubDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub num_ports: u8,
    pub characteristics: u16,
    pub power_on_to_power_good: u8,
    pub hub_control_current: u8,
    // Variable length fields follow
}

impl HubDescriptor {
    pub const LENGTH: u8 = 7; // Minimum length without variable fields
    pub const DESCRIPTOR_TYPE: u8 = 0x29;

    /// Hub characteristics bits
    pub const LOGICAL_POWER_SWITCHING_MODE: u16 = 0x0003;
    pub const COMPOUND_DEVICE: u16 = 0x0004;
    pub const OVER_CURRENT_PROTECTION_MODE: u16 = 0x0018;
    pub const TT_THINK_TIME: u16 = 0x0060;
    pub const PORT_INDICATORS: u16 = 0x0080;

    pub fn new(num_ports: u8, characteristics: u16) -> Self {
        Self {
            length: Self::LENGTH,
            descriptor_type: Self::DESCRIPTOR_TYPE,
            num_ports,
            characteristics,
            power_on_to_power_good: 50, // Default 100ms (2ms units)
            hub_control_current: 0,
        }
    }
}

/// USB Hub Implementation
pub struct UsbHub {
    /// Hub device (None for root hub)
    device: Option<Arc<Mutex<UsbDevice>>>,
    /// Number of downstream ports
    port_count: u8,
    /// Port status array
    port_status: Vec<AtomicU16>,
    /// Port status change array
    port_changes: Vec<AtomicU16>,
    /// Hub descriptor
    hub_descriptor: HubDescriptor,
    /// Hub is root hub
    is_root_hub: bool,
    /// Hub state
    state: AtomicU8,
}

/// Hub State
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubState {
    Uninitialized = 0,
    Initializing = 1,
    Running = 2,
    Error = 3,
}

impl From<u8> for HubState {
    fn from(value: u8) -> Self {
        match value {
            0 => HubState::Uninitialized,
            1 => HubState::Initializing,
            2 => HubState::Running,
            _ => HubState::Error,
        }
    }
}

impl UsbHub {
    /// Create a new root hub
    pub fn new_root_hub() -> Result<Self> {
        let port_count = 16; // Typical root hub port count
        let hub_descriptor = HubDescriptor::new(
            port_count,
            HubDescriptor::LOGICAL_POWER_SWITCHING_MODE | HubDescriptor::PORT_INDICATORS,
        );

        Ok(Self {
            device: None,
            port_count,
            port_status: (0..port_count).map(|_| AtomicU16::new(0)).collect(),
            port_changes: (0..port_count).map(|_| AtomicU16::new(0)).collect(),
            hub_descriptor,
            is_root_hub: true,
            state: AtomicU8::new(HubState::Uninitialized as u8),
        })
    }

    /// Create a new external hub
    pub fn new_external_hub(device: Arc<Mutex<UsbDevice>>) -> Result<Self> {
        // Would read hub descriptor from device to get port count
        let port_count = 4; // Default for external hub

        let hub_descriptor = HubDescriptor::new(port_count, 0);

        Ok(Self {
            device: Some(device),
            port_count,
            port_status: (0..port_count).map(|_| AtomicU16::new(0)).collect(),
            port_changes: (0..port_count).map(|_| AtomicU16::new(0)).collect(),
            hub_descriptor,
            is_root_hub: false,
            state: AtomicU8::new(HubState::Uninitialized as u8),
        })
    }

    /// Initialize the hub
    pub async fn initialize(&mut self) -> Result<()> {
        self.set_state(HubState::Initializing);

        if !self.is_root_hub {
            // Configure external hub
            self.configure_external_hub().await?;
        }

        // Power on all ports
        for port in 0..self.port_count {
            self.set_port_power(port, true).await?;
        }

        self.set_state(HubState::Running);
        Ok(())
    }

    /// Configure external hub
    async fn configure_external_hub(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            let device = device.lock();

            // Get hub descriptor
            let setup = SetupPacket::new(
                0xA0, // Device to host, class, device
                hub_requests::GET_DESCRIPTOR,
                (HubDescriptor::DESCRIPTOR_TYPE as u16) << 8,
                0,
                HubDescriptor::LENGTH as u16,
            );

            let buffer = TransferBuffer::new(HubDescriptor::LENGTH as usize);
            let transfer = Transfer::control(device.address(), setup, buffer);

            // Submit transfer (this is simplified)
            // In practice, you'd submit this through the controller
        }

        Ok(())
    }

    /// Get hub state
    pub fn state(&self) -> HubState {
        HubState::from(self.state.load(Ordering::Acquire))
    }

    /// Set hub state
    fn set_state(&self, state: HubState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Get port count
    pub fn port_count(&self) -> u8 {
        self.port_count
    }

    /// Get port status
    pub fn get_port_status(&self, port: u8) -> Result<PortStatus> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let status_value = self.port_status[port as usize].load(Ordering::Acquire);

        Ok(PortStatus {
            connection: (status_value & 0x0001) != 0,
            enabled: (status_value & 0x0002) != 0,
            suspended: (status_value & 0x0004) != 0,
            over_current: (status_value & 0x0008) != 0,
            reset: (status_value & 0x0010) != 0,
            power: (status_value & 0x0100) != 0,
            low_speed: (status_value & 0x0200) != 0,
            high_speed: (status_value & 0x0400) != 0,
            test: (status_value & 0x0800) != 0,
            indicator: (status_value & 0x1000) != 0,
        })
    }

    /// Get port status change
    pub fn get_port_status_change(&self, port: u8) -> Result<PortStatusChange> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let change_value = self.port_changes[port as usize].load(Ordering::Acquire);

        Ok(PortStatusChange {
            connection_change: (change_value & 0x0001) != 0,
            enable_change: (change_value & 0x0002) != 0,
            suspend_change: (change_value & 0x0004) != 0,
            over_current_change: (change_value & 0x0008) != 0,
            reset_change: (change_value & 0x0010) != 0,
        })
    }

    /// Set port feature
    pub async fn set_port_feature(&mut self, port: u8, feature: u16) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        match feature {
            hub_features::PORT_RESET => {
                self.reset_port(port).await?;
            },
            hub_features::PORT_POWER => {
                self.set_port_power(port, true).await?;
            },
            hub_features::PORT_SUSPEND => {
                self.suspend_port(port).await?;
            },
            _ => return Err(UsbDriverError::NotSupported),
        }

        Ok(())
    }

    /// Clear port feature
    pub async fn clear_port_feature(&mut self, port: u8, feature: u16) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        match feature {
            hub_features::C_PORT_CONNECTION => {
                self.clear_port_change(port, 0x0001);
            },
            hub_features::C_PORT_ENABLE => {
                self.clear_port_change(port, 0x0002);
            },
            hub_features::C_PORT_SUSPEND => {
                self.clear_port_change(port, 0x0004);
            },
            hub_features::C_PORT_OVER_CURRENT => {
                self.clear_port_change(port, 0x0008);
            },
            hub_features::C_PORT_RESET => {
                self.clear_port_change(port, 0x0010);
            },
            hub_features::PORT_ENABLE => {
                self.disable_port(port).await?;
            },
            hub_features::PORT_SUSPEND => {
                self.resume_port(port).await?;
            },
            hub_features::PORT_POWER => {
                self.set_port_power(port, false).await?;
            },
            _ => return Err(UsbDriverError::NotSupported),
        }

        Ok(())
    }

    /// Reset port
    pub async fn reset_port(&mut self, port: u8) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        // Set reset bit
        let status_index = port as usize;
        self.port_status[status_index].fetch_or(0x0010, Ordering::AcqRel);

        // Wait for reset to complete (simplified)
        // In practice, this would be handled by the controller

        // Clear reset bit and set change bit
        self.port_status[status_index].fetch_and(!0x0010, Ordering::AcqRel);
        self.port_changes[status_index].fetch_or(0x0010, Ordering::AcqRel);

        Ok(())
    }

    /// Set port power
    pub async fn set_port_power(&mut self, port: u8, powered: bool) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let status_index = port as usize;
        if powered {
            self.port_status[status_index].fetch_or(0x0100, Ordering::AcqRel);
        } else {
            self.port_status[status_index].fetch_and(!0x0100, Ordering::AcqRel);
        }

        Ok(())
    }

    /// Suspend port
    pub async fn suspend_port(&mut self, port: u8) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let status_index = port as usize;
        self.port_status[status_index].fetch_or(0x0004, Ordering::AcqRel);
        self.port_changes[status_index].fetch_or(0x0004, Ordering::AcqRel);

        Ok(())
    }

    /// Resume port
    pub async fn resume_port(&mut self, port: u8) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let status_index = port as usize;
        self.port_status[status_index].fetch_and(!0x0004, Ordering::AcqRel);
        self.port_changes[status_index].fetch_or(0x0004, Ordering::AcqRel);

        Ok(())
    }

    /// Disable port
    pub async fn disable_port(&mut self, port: u8) -> Result<()> {
        if port >= self.port_count {
            return Err(UsbDriverError::InvalidParameter);
        }

        let status_index = port as usize;
        self.port_status[status_index].fetch_and(!0x0002, Ordering::AcqRel);
        self.port_changes[status_index].fetch_or(0x0002, Ordering::AcqRel);

        Ok(())
    }

    /// Clear port status change
    fn clear_port_change(&self, port: u8, change_bit: u16) {
        if port < self.port_count {
            self.port_changes[port as usize].fetch_and(!change_bit, Ordering::AcqRel);
        }
    }

    /// Probe port for device
    pub async fn probe_port(&mut self, port: u8) -> Result<Option<UsbDevice>> {
        let status = self.get_port_status(port)?;

        if !status.is_connected() {
            return Ok(None);
        }

        // Reset and enable port
        self.reset_port(port).await?;

        // Wait for reset completion
        // In practice, you'd wait for the reset change bit

        // Determine device speed
        let status = self.get_port_status(port)?;
        let speed = status.device_speed();

        // Create device placeholder
        // In practice, you'd create a proper device descriptor
        let device_descriptor = Box::new(crate::descriptor::DeviceDescriptor::new(
            0x0200, // USB 2.0
            0x00,   // Class in interface
            0x00,   // Subclass
            0x00,   // Protocol
            64,     // Max packet size
            0x0000, // Vendor ID
            0x0000, // Product ID
            0x0100, // Device version
            0,      // Manufacturer string
            0,      // Product string
            0,      // Serial number string
            1,      // Number of configurations
        ));

        let device = UsbDevice::new(speed, port, 0, device_descriptor);

        Ok(Some(device))
    }

    /// Check for port status changes
    pub fn check_port_changes(&self) -> Vec<u8> {
        let mut changed_ports = Vec::new();

        for port in 0..self.port_count {
            let changes = self.port_changes[port as usize].load(Ordering::Acquire);
            if changes != 0 {
                changed_ports.push(port);
            }
        }

        changed_ports
    }

    /// Simulate device connection (for testing)
    pub fn simulate_device_connection(&self, port: u8, speed: DeviceSpeed) {
        if port < self.port_count {
            let mut status = 0x0101; // Connected and powered

            match speed {
                DeviceSpeed::Low => status |= 0x0200,
                DeviceSpeed::High => status |= 0x0400,
                _ => {}, // Full speed has no special bit
            }

            self.port_status[port as usize].store(status, Ordering::Release);
            self.port_changes[port as usize].store(0x0001, Ordering::Release); // Connection change
        }
    }

    /// Simulate device disconnection (for testing)
    pub fn simulate_device_disconnection(&self, port: u8) {
        if port < self.port_count {
            self.port_status[port as usize].store(0x0100, Ordering::Release); // Powered only
            self.port_changes[port as usize].store(0x0001, Ordering::Release); // Connection change
        }
    }
}

impl fmt::Debug for UsbHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsbHub")
            .field("is_root_hub", &self.is_root_hub)
            .field("port_count", &self.port_count)
            .field("state", &self.state())
            .finish()
    }
}

unsafe impl Send for UsbHub {}
unsafe impl Sync for UsbHub {}