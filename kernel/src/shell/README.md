# PrismaOS Interactive Shell

A zero-allocation interactive shell for PrismaOS with built-in commands and command history.

## Features

- **Zero heap allocation** - All buffers are compile-time static
- **Built-in commands** - 12 commands including help, ls, cat, drivers, etc.
- **Command history** - 32-command history with arrow key navigation
- **Extensible** - Easy to add new commands
- **Safe** - Proper bounds checking and error handling

## Architecture

### Memory Layout
```
Static buffers (NO runtime allocation!):
├── INPUT_BUFFER: [u8; 256]              - Current command line
├── COMMAND_HISTORY: [[u8; 256]; 32]     - 32 previous commands
└── State variables: positions, flags     - Tracking state
                                Total: ~8.5 KB static data
```

### Integration

The shell registers itself as the keyboard input handler during initialization:
1. `shell::init()` is called after kernel subsystems are ready
2. Shell registers `handle_key()` as the keyboard callback
3. All keyboard input is routed to the shell
4. Commands are parsed and executed synchronously

## Built-in Commands

| Command | Description | Usage |
|---------|-------------|-------|
| `help` | Display available commands | `help` |
| `echo` | Print arguments to screen | `echo [args...]` |
| `clear` | Clear the screen | `clear` |
| `uname` | Print system information | `uname [-a]` |
| `uptime` | Show system uptime | `uptime` |
| `meminfo` | Display memory information | `meminfo` |
| `drivers` | List loaded drivers | `drivers` |
| `ls` | List directory contents | `ls [path]` |
| `cat` | Display file contents | `cat <file>` |
| `exit` | Exit the shell | `exit` |
| `reboot` | Reboot the system | `reboot` |
| `panic` | Trigger kernel panic (testing) | `panic` |

## Adding New Commands

To add a new command:

1. Create a command handler function:
```rust
fn cmd_mycommand(args: &[&str]) -> CommandResult {
    kprintln!("My command executed!");
    CommandResult::Success
}
```

2. Add it to the `BUILTIN_COMMANDS` array:
```rust
const BUILTIN_COMMANDS: &[Command] = &[
    // ... existing commands ...
    Command {
        name: "mycommand",
        description: "Does something useful",
        usage: "mycommand [args]",
        handler: cmd_mycommand,
    },
];
```

That's it! The command is now available in the shell.

## Command History

- **Arrow Up**: Navigate to previous command
- **Arrow Down**: Navigate to next command (or clear if at end)
- **32 command buffer**: Oldest commands are automatically overwritten

## Design Principles

1. **No heap allocation**: All buffers are compile-time static
2. **No panics**: Proper bounds checking prevents buffer overflows
3. **Simple parsing**: Whitespace-separated arguments (max 16 args)
4. **Extensible**: Easy to add new commands via const array

## Future Enhancements

- [ ] Tab completion for commands and file paths
- [ ] Command aliasing
- [ ] Pipe and redirection support
- [ ] Background jobs
- [ ] Script execution
- [ ] Environment variables
