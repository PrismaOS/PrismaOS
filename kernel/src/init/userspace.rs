//! Userspace components launcher
//!
//! Best-effort launching of userspace components such as the boot GUI.

/// Launch userspace components such as the boot GUI. This is a best-effort
/// operation: failures are logged but do not stop the remainder of the boot
/// process.
pub fn launch_userspace_components() {
    lib_kernel::kprintln!("");
    lib_kernel::kprintln!("=== Launching PrismaOS Desktop Environment ===");
    // We intentionally ignore detailed errors here to keep the boot robust
    //   if let Err(e) = crate::boot_userspace::launch_boot_gui(&mut crate::memory::dummy_mapper(), &mut crate::memory::dummy_frame_allocator()) {
    //       crate::kprintln!("ERROR: Failed to launch boot GUI: {}", e);
    //       crate::kprintln!("Continuing without GUI...");
    //   } else {
    //       crate::kprintln!("[OK] Boot GUI launched successfully");
    //   }
}
