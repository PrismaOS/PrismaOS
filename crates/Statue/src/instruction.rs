//! Comprehensive instruction decoder and interpreter for multiple architectures.

use crate::error::{ElfError, Result};
use crate::arch::{ExecutionState, AArch64ExecutionState, RiscVExecutionState};

/// Instruction execution result
#[derive(Debug, Clone, Copy)]
pub enum InstructionResult {
    /// Continue execution
    Continue,
    /// Exit with code
    Exit(u64),
    /// System call made
    SystemCall,
    /// Jump to address
    Jump(u64),
    /// Conditional jump
    ConditionalJump(u64, bool),
    /// Call function
    Call(u64),
    /// Return from function
    Return,
}

/// x86_64 instruction decoder and interpreter
pub struct X86_64Interpreter;

impl X86_64Interpreter {
    /// Decode and execute x86_64 instruction
    pub fn execute_instruction(state: &mut ExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.is_empty() {
            return Err(ElfError::ExecutionSetupFailed);
        }

        // Handle REX prefix
        let mut offset = 0;
        let mut rex_prefix = 0u8;
        if bytes[0] & 0xf0 == 0x40 {
            rex_prefix = bytes[0];
            offset += 1;
            if offset >= bytes.len() {
                return Err(ElfError::ExecutionSetupFailed);
            }
        }

        let opcode = bytes[offset];
        match opcode {
            // MOV instructions
            0x48 if offset + 1 < bytes.len() => {
                match bytes[offset + 1] {
                    // MOV r64, imm64 (REX.W + 0xC7 + ModR/M)
                    0xc7 if offset + 7 < bytes.len() => {
                        let reg = bytes[offset + 2] & 0x7;
                        let immediate = u32::from_le_bytes([
                            bytes[offset + 3], bytes[offset + 4],
                            bytes[offset + 5], bytes[offset + 6]
                        ]) as u64;

                        Self::set_register_64(state, reg, immediate);
                        state.rip += 7;
                        Ok(InstructionResult::Continue)
                    }
                    // MOV r64, r64 (REX.W + 0x89 + ModR/M)
                    0x89 if offset + 3 <= bytes.len() => {
                        let modrm = bytes[offset + 2];
                        let src_reg = (modrm >> 3) & 0x7;
                        let dst_reg = modrm & 0x7;

                        let src_value = Self::get_register_64(state, src_reg);
                        Self::set_register_64(state, dst_reg, src_value);
                        state.rip += 3;
                        Ok(InstructionResult::Continue)
                    }
                    // MOV r64, [r64] (REX.W + 0x8B + ModR/M)
                    0x8b if offset + 3 <= bytes.len() => {
                        // Memory access - simplified implementation
                        state.rip += 3;
                        Ok(InstructionResult::Continue)
                    }
                    _ => {
                        state.rip += 2;
                        Ok(InstructionResult::Continue)
                    }
                }
            }

            // ADD instructions
            0x01 if offset + 2 < bytes.len() => {
                let modrm = bytes[offset + 1];
                let src_reg = (modrm >> 3) & 0x7;
                let dst_reg = modrm & 0x7;

                let src_value = Self::get_register_64(state, src_reg);
                let dst_value = Self::get_register_64(state, dst_reg);
                let result = dst_value.wrapping_add(src_value);

                Self::set_register_64(state, dst_reg, result);
                Self::update_flags_add(state, dst_value, src_value, result);
                state.rip += 2;
                Ok(InstructionResult::Continue)
            }

            // SUB instructions
            0x29 if offset + 2 < bytes.len() => {
                let modrm = bytes[offset + 1];
                let src_reg = (modrm >> 3) & 0x7;
                let dst_reg = modrm & 0x7;

                let src_value = Self::get_register_64(state, src_reg);
                let dst_value = Self::get_register_64(state, dst_reg);
                let result = dst_value.wrapping_sub(src_value);

                Self::set_register_64(state, dst_reg, result);
                Self::update_flags_sub(state, dst_value, src_value, result);
                state.rip += 2;
                Ok(InstructionResult::Continue)
            }

            // CMP instruction
            0x39 if offset + 2 < bytes.len() => {
                let modrm = bytes[offset + 1];
                let src_reg = (modrm >> 3) & 0x7;
                let dst_reg = modrm & 0x7;

                let src_value = Self::get_register_64(state, src_reg);
                let dst_value = Self::get_register_64(state, dst_reg);
                let result = dst_value.wrapping_sub(src_value);

                Self::update_flags_sub(state, dst_value, src_value, result);
                state.rip += 2;
                Ok(InstructionResult::Continue)
            }

            // TEST instruction
            0x85 if offset + 2 < bytes.len() => {
                let modrm = bytes[offset + 1];
                let reg1 = (modrm >> 3) & 0x7;
                let reg2 = modrm & 0x7;

                let val1 = Self::get_register_64(state, reg1);
                let val2 = Self::get_register_64(state, reg2);
                let result = val1 & val2;

                Self::update_flags_logical(state, result);
                state.rip += 2;
                Ok(InstructionResult::Continue)
            }

            // JMP instructions
            0xe9 if offset + 5 <= bytes.len() => {
                // JMP rel32
                let displacement = i32::from_le_bytes([
                    bytes[offset + 1], bytes[offset + 2],
                    bytes[offset + 3], bytes[offset + 4]
                ]);
                let target = state.rip.wrapping_add(5).wrapping_add(displacement as u64);
                Ok(InstructionResult::Jump(target))
            }

            0xeb if offset + 2 <= bytes.len() => {
                // JMP rel8
                let displacement = bytes[offset + 1] as i8;
                let target = state.rip.wrapping_add(2).wrapping_add(displacement as u64);
                Ok(InstructionResult::Jump(target))
            }

            // Conditional jumps
            0x74 if offset + 2 <= bytes.len() => {
                // JE/JZ rel8
                let displacement = bytes[offset + 1] as i8;
                let target = state.rip.wrapping_add(2).wrapping_add(displacement as u64);
                let zero_flag = (state.rflags & 0x40) != 0;
                Ok(InstructionResult::ConditionalJump(target, zero_flag))
            }

            0x75 if offset + 2 <= bytes.len() => {
                // JNE/JNZ rel8
                let displacement = bytes[offset + 1] as i8;
                let target = state.rip.wrapping_add(2).wrapping_add(displacement as u64);
                let zero_flag = (state.rflags & 0x40) == 0;
                Ok(InstructionResult::ConditionalJump(target, zero_flag))
            }

            // CALL instruction
            0xe8 if offset + 5 <= bytes.len() => {
                let displacement = i32::from_le_bytes([
                    bytes[offset + 1], bytes[offset + 2],
                    bytes[offset + 3], bytes[offset + 4]
                ]);
                let target = state.rip.wrapping_add(5).wrapping_add(displacement as u64);
                Ok(InstructionResult::Call(target))
            }

            // PUSH instructions
            0x50..=0x57 => {
                // PUSH r64
                let reg = opcode & 0x7;
                let value = Self::get_register_64(state, reg);
                state.rsp = state.rsp.wrapping_sub(8);
                // In a real implementation, we'd write to memory at [RSP]
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }

            // POP instructions
            0x58..=0x5f => {
                // POP r64
                let reg = opcode & 0x7;
                // In a real implementation, we'd read from memory at [RSP]
                state.rsp = state.rsp.wrapping_add(8);
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }

            // SYSCALL (0x0F 0x05)
            0x0f if offset + 1 < bytes.len() && bytes[offset + 1] == 0x05 => {
                Ok(InstructionResult::SystemCall)
            }

            // NOP (0x90)
            0x90 => {
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }

            // RET (0xC3)
            0xc3 => {
                Ok(InstructionResult::Return)
            }

            // Multi-byte instructions starting with 0x0F
            0x0f if offset + 1 < bytes.len() => {
                match bytes[offset + 1] {
                    // MOVZX, MOVSX, etc.
                    0xb6 | 0xb7 | 0xbe | 0xbf if offset + 3 <= bytes.len() => {
                        state.rip += 3;
                        Ok(InstructionResult::Continue)
                    }
                    _ => {
                        state.rip += 2;
                        Ok(InstructionResult::Continue)
                    }
                }
            }

            // Default case - skip unknown instruction
            _ => {
                state.rip += 1;
                Ok(InstructionResult::Continue)
            }
        }
    }

    /// Get value from 64-bit register
    fn get_register_64(state: &ExecutionState, reg: u8) -> u64 {
        match reg {
            0 => state.rax,
            1 => state.rcx,
            2 => state.rdx,
            3 => state.rbx,
            4 => state.rsp,
            5 => state.rbp,
            6 => state.rsi,
            7 => state.rdi,
            _ => 0,
        }
    }

    /// Set value in 64-bit register
    fn set_register_64(state: &mut ExecutionState, reg: u8, value: u64) {
        match reg {
            0 => state.rax = value,
            1 => state.rcx = value,
            2 => state.rdx = value,
            3 => state.rbx = value,
            4 => state.rsp = value,
            5 => state.rbp = value,
            6 => state.rsi = value,
            7 => state.rdi = value,
            _ => {}
        }
    }

    /// Update flags after addition
    fn update_flags_add(state: &mut ExecutionState, op1: u64, op2: u64, result: u64) {
        // Zero flag
        if result == 0 {
            state.rflags |= 0x40;
        } else {
            state.rflags &= !0x40;
        }

        // Sign flag
        if result & 0x8000000000000000 != 0 {
            state.rflags |= 0x80;
        } else {
            state.rflags &= !0x80;
        }

        // Carry flag
        if result < op1 {
            state.rflags |= 0x01;
        } else {
            state.rflags &= !0x01;
        }

        // Overflow flag
        let sign1 = op1 & 0x8000000000000000;
        let sign2 = op2 & 0x8000000000000000;
        let sign_result = result & 0x8000000000000000;
        if sign1 == sign2 && sign1 != sign_result {
            state.rflags |= 0x800;
        } else {
            state.rflags &= !0x800;
        }
    }

    /// Update flags after subtraction
    fn update_flags_sub(state: &mut ExecutionState, op1: u64, op2: u64, result: u64) {
        // Zero flag
        if result == 0 {
            state.rflags |= 0x40;
        } else {
            state.rflags &= !0x40;
        }

        // Sign flag
        if result & 0x8000000000000000 != 0 {
            state.rflags |= 0x80;
        } else {
            state.rflags &= !0x80;
        }

        // Carry flag (borrow)
        if op1 < op2 {
            state.rflags |= 0x01;
        } else {
            state.rflags &= !0x01;
        }
    }

    /// Update flags after logical operation
    fn update_flags_logical(state: &mut ExecutionState, result: u64) {
        // Zero flag
        if result == 0 {
            state.rflags |= 0x40;
        } else {
            state.rflags &= !0x40;
        }

        // Sign flag
        if result & 0x8000000000000000 != 0 {
            state.rflags |= 0x80;
        } else {
            state.rflags &= !0x80;
        }

        // Clear carry and overflow flags
        state.rflags &= !0x01;
        state.rflags &= !0x800;
    }
}

/// AArch64 instruction interpreter
pub struct AArch64Interpreter;

impl AArch64Interpreter {
    /// Decode and execute AArch64 instruction
    pub fn execute_instruction(state: &mut AArch64ExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.len() < 4 {
            return Err(ElfError::ExecutionSetupFailed);
        }

        let instruction = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        match instruction {
            // MOV (immediate)
            i if (i & 0xff800000) == 0xd2800000 => {
                let rd = (i & 0x1f) as usize;
                let imm = ((i >> 5) & 0xffff) as u64;
                if rd < 31 {
                    state.x[rd] = imm;
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            // ADD (immediate)
            i if (i & 0xff000000) == 0x91000000 => {
                let rd = (i & 0x1f) as usize;
                let rn = ((i >> 5) & 0x1f) as usize;
                let imm = ((i >> 10) & 0xfff) as u64;

                if rd < 31 && rn < 31 {
                    state.x[rd] = state.x[rn].wrapping_add(imm);
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            // SUB (immediate)
            i if (i & 0xff000000) == 0xd1000000 => {
                let rd = (i & 0x1f) as usize;
                let rn = ((i >> 5) & 0x1f) as usize;
                let imm = ((i >> 10) & 0xfff) as u64;

                if rd < 31 && rn < 31 {
                    state.x[rd] = state.x[rn].wrapping_sub(imm);
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            // B (unconditional branch)
            i if (i & 0xfc000000) == 0x14000000 => {
                let imm = ((i & 0x03ffffff) as i32) << 6 >> 6; // Sign extend
                let target = state.pc.wrapping_add((imm * 4) as u64);
                Ok(InstructionResult::Jump(target))
            }

            // BL (branch with link)
            i if (i & 0xfc000000) == 0x94000000 => {
                let imm = ((i & 0x03ffffff) as i32) << 6 >> 6; // Sign extend
                let target = state.pc.wrapping_add((imm * 4) as u64);
                state.x[30] = state.pc + 4; // Link register
                Ok(InstructionResult::Call(target))
            }

            // RET
            0xd65f03c0 => {
                let target = state.x[30]; // Return to link register
                Ok(InstructionResult::Jump(target))
            }

            // SVC (system call)
            i if (i & 0xffe0001f) == 0xd4000001 => {
                Ok(InstructionResult::SystemCall)
            }

            // NOP
            0xd503201f => {
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            _ => {
                // Unknown instruction - skip it
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
        }
    }
}

/// RISC-V instruction interpreter
pub struct RiscVInterpreter;

impl RiscVInterpreter {
    /// Decode and execute RISC-V instruction
    pub fn execute_instruction(state: &mut RiscVExecutionState, bytes: &[u8]) -> Result<InstructionResult> {
        if bytes.len() < 4 {
            return Err(ElfError::ExecutionSetupFailed);
        }

        let instruction = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let opcode = instruction & 0x7f;

        match opcode {
            // ECALL/EBREAK
            0x73 => {
                if instruction == 0x73 {
                    Ok(InstructionResult::SystemCall)
                } else {
                    state.pc += 4;
                    Ok(InstructionResult::Continue)
                }
            }

            // ADDI, SLTI, SLTIU, XORI, ORI, ANDI
            0x13 => {
                let rd = ((instruction >> 7) & 0x1f) as usize;
                let rs1 = ((instruction >> 15) & 0x1f) as usize;
                let imm = ((instruction as i32) >> 20) as i64 as u64;
                let funct3 = (instruction >> 12) & 0x7;

                if rd != 0 && rs1 < 32 {
                    match funct3 {
                        0 => state.x[rd] = state.x[rs1].wrapping_add(imm), // ADDI
                        4 => state.x[rd] = state.x[rs1] ^ imm,             // XORI
                        6 => state.x[rd] = state.x[rs1] | imm,             // ORI
                        7 => state.x[rd] = state.x[rs1] & imm,             // ANDI
                        _ => {}
                    }
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            // LUI
            0x37 => {
                let rd = ((instruction >> 7) & 0x1f) as usize;
                let imm = (instruction & 0xfffff000) as u64;

                if rd != 0 {
                    state.x[rd] = imm;
                }
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }

            // JAL
            0x6f => {
                let rd = ((instruction >> 7) & 0x1f) as usize;
                let imm = Self::decode_jal_immediate(instruction);

                if rd != 0 {
                    state.x[rd] = state.pc + 4;
                }
                let target = state.pc.wrapping_add(imm as u64);
                Ok(InstructionResult::Jump(target))
            }

            // JALR
            0x67 => {
                let rd = ((instruction >> 7) & 0x1f) as usize;
                let rs1 = ((instruction >> 15) & 0x1f) as usize;
                let imm = ((instruction as i32) >> 20) as i64;

                let target = state.x[rs1].wrapping_add(imm as u64) & !1;
                if rd != 0 {
                    state.x[rd] = state.pc + 4;
                }
                Ok(InstructionResult::Jump(target))
            }

            _ => {
                // Unknown instruction - skip it
                state.pc += 4;
                Ok(InstructionResult::Continue)
            }
        }
    }

    /// Decode JAL immediate field
    fn decode_jal_immediate(instruction: u32) -> i32 {
        let bit_20 = (instruction >> 31) & 1;
        let bit_10_1 = (instruction >> 21) & 0x3ff;
        let bit_11 = (instruction >> 20) & 1;
        let bit_19_12 = (instruction >> 12) & 0xff;

        let imm = (bit_20 << 20) | (bit_19_12 << 12) | (bit_11 << 11) | (bit_10_1 << 1);

        // Sign extend
        if bit_20 != 0 {
            (imm | 0xffe00000) as i32
        } else {
            imm as i32
        }
    }
}