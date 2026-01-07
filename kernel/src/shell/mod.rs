//! Interactive kernel shell with built-in commands
//!
//! A simple but functional shell for PrismaOS with:
//! - Zero heap allocation (static buffers only)
//! - Built-in commands (ls, cat, echo, help, clear, etc.)
//! - Command history
//! - Tab completion (future)
//! - Command aliasing (future)

use lib_kernel::{kprintln, kprint};
use pc_keyboard::{DecodedKey, KeyCode};

/// Maximum command line length
const MAX_COMMAND_LEN: usize = 256;

/// Maximum number of arguments per command
const MAX_ARGS: usize = 16;

/// Command history size
const HISTORY_SIZE: usize = 32;

/// Static input buffer (NO heap allocation!)
static mut INPUT_BUFFER: [u8; MAX_COMMAND_LEN] = [0; MAX_COMMAND_LEN];
static mut INPUT_POS: usize = 0;

/// Command history buffer (NO heap allocation!)
static mut COMMAND_HISTORY: [[u8; MAX_COMMAND_LEN]; HISTORY_SIZE] = [[0; MAX_COMMAND_LEN]; HISTORY_SIZE];
static mut HISTORY_LEN: [usize; HISTORY_SIZE] = [0; HISTORY_SIZE];
static mut HISTORY_POS: usize = 0;
static mut HISTORY_INDEX: usize = 0;

/// Shell state
static mut SHELL_RUNNING: bool = false;

/// Built-in command function type
type CommandFn = fn(&[&str]) -> CommandResult;

/// Command execution result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandResult {
    Success,
    Error,
    Exit,
}

/// Built-in command definition
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
    pub handler: CommandFn,
}

/// Initialize the shell
pub fn init() {
    unsafe {
        SHELL_RUNNING = true;
        INPUT_POS = 0;
        HISTORY_POS = 0;
        HISTORY_INDEX = 0;
    }

    // Register shell as keyboard handler
    lib_kernel::executor::keyboard::set_keyboard_handler(handle_key);

    kprintln!();
    kprintln!("===========================================");
    kprintln!("    PrismaOS Interactive Shell v0.1");
    kprintln!("===========================================");
    kprintln!("Type 'help' for available commands");
    kprintln!();
    print_prompt();
}

/// Print the shell prompt
fn print_prompt() {
    kprint!("prisma> ");
}

/// Handle keyboard input
pub fn handle_key(key: DecodedKey) {
    unsafe {
        if !SHELL_RUNNING {
            return;
        }

        match key {
            DecodedKey::Unicode(character) => {
                match character {
                    '\n' => {
                        // Execute command
                        kprintln!();
                        execute_command();
                        INPUT_POS = 0;
                        print_prompt();
                    }
                    '\x08' | '\x7f' => {
                        // Backspace
                        if INPUT_POS > 0 {
                            INPUT_POS -= 1;
                            kprint!("\x08 \x08"); // Erase character on screen
                        }
                    }
                    c if c.is_ascii_graphic() || c == ' ' => {
                        // Printable character
                        if INPUT_POS < MAX_COMMAND_LEN - 1 {
                            INPUT_BUFFER[INPUT_POS] = c as u8;
                            INPUT_POS += 1;
                            kprint!("{}", c);
                        }
                    }
                    _ => {}
                }
            }
            DecodedKey::RawKey(keycode) => {
                match keycode {
                    KeyCode::ArrowUp => {
                        // Previous command in history
                        load_history_prev();
                    }
                    KeyCode::ArrowDown => {
                        // Next command in history
                        load_history_next();
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Execute the current command
fn execute_command() {
    unsafe {
        if INPUT_POS == 0 {
            return;
        }

        // Add to history
        save_to_history();

        // Parse command and arguments
        let input = core::str::from_utf8(&INPUT_BUFFER[..INPUT_POS]).unwrap_or("");
        let mut args_storage: [&str; MAX_ARGS] = [""; MAX_ARGS];
        let mut arg_count = 0;

        for part in input.split_whitespace() {
            if arg_count < MAX_ARGS {
                args_storage[arg_count] = part;
                arg_count += 1;
            }
        }

        if arg_count == 0 {
            return;
        }

        let args = &args_storage[..arg_count];
        let cmd_name = args[0];
        let cmd_args = if args.len() > 1 { &args[1..] } else { &[] };

        // Find and execute command
        let mut found = false;
        for cmd in BUILTIN_COMMANDS {
            if cmd.name == cmd_name {
                found = true;
                match (cmd.handler)(cmd_args) {
                    CommandResult::Success => {}
                    CommandResult::Error => {
                        kprintln!("Command failed");
                    }
                    CommandResult::Exit => {
                        SHELL_RUNNING = false;
                        kprintln!("Shell terminated");
                    }
                }
                break;
            }
        }

        if !found {
            kprintln!("Unknown command: {}", cmd_name);
            kprintln!("Type 'help' for available commands");
        }
    }
}

/// Save current command to history
fn save_to_history() {
    unsafe {
        let pos = HISTORY_POS % HISTORY_SIZE;
        let len = INPUT_POS.min(MAX_COMMAND_LEN);

        COMMAND_HISTORY[pos][..len].copy_from_slice(&INPUT_BUFFER[..len]);
        HISTORY_LEN[pos] = len;
        HISTORY_POS = (HISTORY_POS + 1) % HISTORY_SIZE;
        HISTORY_INDEX = HISTORY_POS;
    }
}

/// Load previous command from history
fn load_history_prev() {
    unsafe {
        if HISTORY_POS == 0 {
            return;
        }

        if HISTORY_INDEX > 0 {
            HISTORY_INDEX -= 1;
        }

        let idx = HISTORY_INDEX % HISTORY_SIZE;
        let len = HISTORY_LEN[idx];

        if len > 0 {
            // Clear current line
            clear_input_line();

            // Load from history
            INPUT_BUFFER[..len].copy_from_slice(&COMMAND_HISTORY[idx][..len]);
            INPUT_POS = len;

            // Display
            kprint!("{}", core::str::from_utf8(&INPUT_BUFFER[..INPUT_POS]).unwrap_or(""));
        }
    }
}

/// Load next command from history
fn load_history_next() {
    unsafe {
        if HISTORY_INDEX < HISTORY_POS {
            HISTORY_INDEX += 1;
        }

        if HISTORY_INDEX == HISTORY_POS {
            // Clear input
            clear_input_line();
            INPUT_POS = 0;
        } else {
            let idx = HISTORY_INDEX % HISTORY_SIZE;
            let len = HISTORY_LEN[idx];

            if len > 0 {
                clear_input_line();
                INPUT_BUFFER[..len].copy_from_slice(&COMMAND_HISTORY[idx][..len]);
                INPUT_POS = len;
                kprint!("{}", core::str::from_utf8(&INPUT_BUFFER[..INPUT_POS]).unwrap_or(""));
            }
        }
    }
}

/// Clear the current input line on screen
fn clear_input_line() {
    unsafe {
        for _ in 0..INPUT_POS {
            kprint!("\x08 \x08");
        }
    }
}

// ============================================================================
// BUILT-IN COMMANDS
// ============================================================================

fn cmd_help(_args: &[&str]) -> CommandResult {
    kprintln!();
    kprintln!("Available commands:");
    kprintln!();

    for cmd in BUILTIN_COMMANDS {
        kprintln!("  {:12} - {}", cmd.name, cmd.description);
    }

    kprintln!();
    kprintln!("Use '<command> --help' for detailed usage information");
    kprintln!();

    CommandResult::Success
}

fn cmd_echo(args: &[&str]) -> CommandResult {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            kprint!(" ");
        }
        kprint!("{}", arg);
    }
    kprintln!();
    CommandResult::Success
}

fn cmd_clear(_args: &[&str]) -> CommandResult {
    // Clear screen (VT100 escape sequence)
    kprint!("\x1b[2J\x1b[H");
    CommandResult::Success
}

fn cmd_uname(args: &[&str]) -> CommandResult {
    if args.len() > 0 && args[0] == "-a" {
        kprintln!("PrismaOS 0.1.0 x86_64 (Rust kernel)");
    } else {
        kprintln!("PrismaOS");
    }
    CommandResult::Success
}

fn cmd_uptime(_args: &[&str]) -> CommandResult {
    let ticks = lib_kernel::time::current_tick();
    let ms = lib_kernel::time::get_timestamp();
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    kprintln!("Uptime: {}h {}m {}s ({} ticks, {} ms)",
              hours, minutes % 60, seconds % 60, ticks, ms);
    CommandResult::Success
}

fn cmd_meminfo(_args: &[&str]) -> CommandResult {
    kprintln!("Memory Information:");
    kprintln!("  (Memory stats not yet implemented)");
    CommandResult::Success
}

fn cmd_drivers(_args: &[&str]) -> CommandResult {
    kprintln!("Loaded drivers:");

    let dm = lib_kernel::drivers::device_manager();
    let count = dm.driver_count();
    kprintln!("  Total: {} drivers", count);

    let names = dm.list_drivers();
    for name in names {
        kprintln!("    • {}", name);
    }

    CommandResult::Success
}

fn cmd_ls(args: &[&str]) -> CommandResult {
    let path = if args.len() > 0 { args[0] } else { "/" };
    kprintln!("Listing directory: {}", path);
    kprintln!("  (Filesystem listing not yet implemented)");
    CommandResult::Success
}

fn cmd_cat(args: &[&str]) -> CommandResult {
    if args.len() == 0 {
        kprintln!("Usage: cat <file>");
        return CommandResult::Error;
    }

    kprintln!("Reading file: {}", args[0]);
    kprintln!("  (File reading not yet implemented)");
    CommandResult::Success
}

fn cmd_exit(_args: &[&str]) -> CommandResult {
    kprintln!("Exiting shell...");
    CommandResult::Exit
}

fn cmd_reboot(_args: &[&str]) -> CommandResult {
    kprintln!("Rebooting system...");
    unsafe {
        // Triple fault to reboot
        core::arch::asm!("int3");
    }
    CommandResult::Success
}

fn cmd_panic(_args: &[&str]) -> CommandResult {
    panic!("User-initiated panic from shell");
}

/// All built-in commands (static, compile-time constant)
const BUILTIN_COMMANDS: &[Command] = &[
    Command {
        name: "help",
        description: "Display available commands",
        usage: "help",
        handler: cmd_help,
    },
    Command {
        name: "echo",
        description: "Print arguments to screen",
        usage: "echo [args...]",
        handler: cmd_echo,
    },
    Command {
        name: "clear",
        description: "Clear the screen",
        usage: "clear",
        handler: cmd_clear,
    },
    Command {
        name: "uname",
        description: "Print system information",
        usage: "uname [-a]",
        handler: cmd_uname,
    },
    Command {
        name: "uptime",
        description: "Show system uptime",
        usage: "uptime",
        handler: cmd_uptime,
    },
    Command {
        name: "meminfo",
        description: "Display memory information",
        usage: "meminfo",
        handler: cmd_meminfo,
    },
    Command {
        name: "drivers",
        description: "List loaded drivers",
        usage: "drivers",
        handler: cmd_drivers,
    },
    Command {
        name: "ls",
        description: "List directory contents",
        usage: "ls [path]",
        handler: cmd_ls,
    },
    Command {
        name: "cat",
        description: "Display file contents",
        usage: "cat <file>",
        handler: cmd_cat,
    },
    Command {
        name: "exit",
        description: "Exit the shell",
        usage: "exit",
        handler: cmd_exit,
    },
    Command {
        name: "reboot",
        description: "Reboot the system",
        usage: "reboot",
        handler: cmd_reboot,
    },
    Command {
        name: "panic",
        description: "Trigger a kernel panic (for testing)",
        usage: "panic",
        handler: cmd_panic,
    },
];
