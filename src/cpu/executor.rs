use crate::error::CpuError;
use crate::memory::Bus;

use super::decoder::{
    AccessSize, BitfieldOp, Branch, BranchRegOp, DataProcessingImm, DataProcessingReg, Instruction,
    LoadStore, LogicalOp, MoveWideOp, ShiftType, SystemInstr, WritebackMode,
};
use super::registers::Cpu;

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &self,
        cpu: &mut Cpu,
        instr: &Instruction,
        bus: &mut dyn Bus,
    ) -> Result<(), CpuError> {
        match instr {
            Instruction::DataProcessingImm(dp) => self.execute_dp_imm(cpu, dp),
            Instruction::DataProcessingReg(dp) => self.execute_dp_reg(cpu, dp),
            Instruction::LoadStore(ls) => self.execute_load_store(cpu, ls, bus),
            Instruction::Branch(br) => self.execute_branch(cpu, br),
            Instruction::System(sys) => self.execute_system(cpu, sys),
            Instruction::SimdFp => Err(CpuError::UndefinedInstruction { address: cpu.regs.pc }),
            Instruction::Undefined { opcode: _ } => {
                Err(CpuError::UndefinedInstruction { address: cpu.regs.pc })
            }
        }
    }

    fn execute_dp_imm(&self, cpu: &mut Cpu, instr: &DataProcessingImm) -> Result<(), CpuError> {
        match instr {
            DataProcessingImm::AddSubImm {
                is_64bit,
                is_subtract,
                set_flags,
                dest,
                src,
                immediate,
                shift_by_12,
            } => {
                let operand1 = cpu.get_reg_or_sp(*src, true);
                let operand2 =
                    if *shift_by_12 { (*immediate as u64) << 12 } else { *immediate as u64 };

                let (result, carry, overflow) = if *is_64bit {
                    if *is_subtract {
                        sub_with_flags_64(operand1, operand2)
                    } else {
                        add_with_flags_64(operand1, operand2)
                    }
                } else {
                    let (r, c, o) = if *is_subtract {
                        sub_with_flags_32(operand1 as u32, operand2 as u32)
                    } else {
                        add_with_flags_32(operand1 as u32, operand2 as u32)
                    };
                    (r as u64, c, o)
                };

                if *set_flags {
                    cpu.pstate.n = is_negative(result, *is_64bit);
                    cpu.pstate.z = is_zero(result, *is_64bit);
                    cpu.pstate.c = carry;
                    cpu.pstate.v = overflow;
                    cpu.set_reg_or_sp(*dest, result, false);
                } else {
                    cpu.set_reg_or_sp(*dest, result, true);
                }

                cpu.advance_pc();
                Ok(())
            }

            DataProcessingImm::MoveWide { is_64bit, operation, shift_amount, dest, immediate } => {
                let shifted = (*immediate as u64) << (*shift_amount as u64);

                let value = match operation {
                    MoveWideOp::Negate => {
                        let inverted = !shifted;
                        if *is_64bit { inverted } else { inverted & 0xffffffff }
                    }
                    MoveWideOp::Zero => shifted,
                    MoveWideOp::Keep => {
                        let current = cpu.regs.get_x(*dest);
                        let clear_mask = !(0xffffu64 << (*shift_amount as u64));
                        (current & clear_mask) | shifted
                    }
                };

                cpu.regs.set_x(*dest, value);
                cpu.advance_pc();
                Ok(())
            }

            DataProcessingImm::LogicalImm { is_64bit, operation, dest, src, bitmask } => {
                let operand = cpu.get_reg_or_sp(*src, true);
                let result = apply_logical_op(*operation, operand, *bitmask);
                let result = truncate_result(result, *is_64bit);

                if matches!(operation, LogicalOp::AndSetFlags) {
                    cpu.pstate.n = is_negative(result, *is_64bit);
                    cpu.pstate.z = result == 0;
                    cpu.pstate.c = false;
                    cpu.pstate.v = false;
                    cpu.set_reg_or_sp(*dest, result, false);
                } else {
                    cpu.set_reg_or_sp(*dest, result, matches!(operation, LogicalOp::Or));
                }

                cpu.advance_pc();
                Ok(())
            }

            DataProcessingImm::PcRelAddr { is_page_relative, dest, offset } => {
                let base = if *is_page_relative { cpu.regs.pc & !0xfff } else { cpu.regs.pc };
                let result = (base as i64).wrapping_add(*offset) as u64;
                cpu.regs.set_x(*dest, result);
                cpu.advance_pc();
                Ok(())
            }

            DataProcessingImm::Bitfield { is_64bit, operation, dest, src, rotate, width } => {
                let source = cpu.get_reg_or_sp(*src, false);
                let dest_value = cpu.regs.get_x(*dest);
                let datasize = if *is_64bit { 64 } else { 32 };

                let wmask = create_bitmask(*width as u32, datasize);
                let tmask = create_bitmask(*width as u32, datasize);
                let rotated_src = rotate_right(source, *rotate as u32, datasize);

                let result = match operation {
                    BitfieldOp::SignedExtract => {
                        let extracted = rotated_src & wmask;
                        if (*width as u32) < (*rotate as u32) {
                            extracted
                        } else {
                            sign_extend_from_bit(extracted, *width)
                        }
                    }
                    BitfieldOp::Insert => (dest_value & !tmask) | (rotated_src & wmask),
                    BitfieldOp::UnsignedExtract => rotated_src & wmask,
                };

                cpu.regs.set_x(*dest, truncate_result(result, *is_64bit));
                cpu.advance_pc();
                Ok(())
            }
        }
    }

    fn execute_dp_reg(&self, cpu: &mut Cpu, instr: &DataProcessingReg) -> Result<(), CpuError> {
        match instr {
            DataProcessingReg::LogicalShifted {
                is_64bit,
                operation,
                invert_second,
                dest,
                first_src,
                second_src,
                shift_type,
                shift_amount,
            } => {
                let operand1 = cpu.regs.get_x(*first_src);
                let mut operand2 = cpu.regs.get_x(*second_src);

                operand2 = apply_shift(operand2, *shift_type, *shift_amount, *is_64bit);
                if *invert_second {
                    operand2 = !operand2;
                }

                let result = apply_logical_op(*operation, operand1, operand2);
                let result = truncate_result(result, *is_64bit);

                if matches!(operation, LogicalOp::AndSetFlags) {
                    cpu.pstate.n = is_negative(result, *is_64bit);
                    cpu.pstate.z = result == 0;
                    cpu.pstate.c = false;
                    cpu.pstate.v = false;
                }

                cpu.regs.set_x(*dest, result);
                cpu.advance_pc();
                Ok(())
            }

            DataProcessingReg::AddSubShifted {
                is_64bit,
                is_subtract,
                set_flags,
                dest,
                first_src,
                second_src,
                shift_type,
                shift_amount,
            } => {
                let operand1 = cpu.get_reg_or_sp(*first_src, true);
                let operand2 =
                    apply_shift(cpu.regs.get_x(*second_src), *shift_type, *shift_amount, *is_64bit);

                let (result, carry, overflow) = if *is_64bit {
                    if *is_subtract {
                        sub_with_flags_64(operand1, operand2)
                    } else {
                        add_with_flags_64(operand1, operand2)
                    }
                } else {
                    let (r, c, o) = if *is_subtract {
                        sub_with_flags_32(operand1 as u32, operand2 as u32)
                    } else {
                        add_with_flags_32(operand1 as u32, operand2 as u32)
                    };
                    (r as u64, c, o)
                };

                if *set_flags {
                    cpu.pstate.n = is_negative(result, *is_64bit);
                    cpu.pstate.z = is_zero(result, *is_64bit);
                    cpu.pstate.c = carry;
                    cpu.pstate.v = overflow;
                }

                cpu.set_reg_or_sp(*dest, result, !*set_flags);
                cpu.advance_pc();
                Ok(())
            }

            DataProcessingReg::ConditionalSelect {
                is_64bit,
                is_negate,
                else_operation,
                dest,
                if_src,
                else_src,
                condition,
            } => {
                let condition_met = cpu.check_condition(*condition)?;

                let result = if condition_met {
                    cpu.regs.get_x(*if_src)
                } else {
                    let val = cpu.regs.get_x(*else_src);
                    match (is_negate, else_operation) {
                        (false, 0) => val,
                        (false, 1) => val.wrapping_add(1),
                        (true, 0) => !val,
                        (true, 1) => (!val).wrapping_add(1),
                        _ => val,
                    }
                };

                cpu.regs.set_x(*dest, truncate_result(result, *is_64bit));
                cpu.advance_pc();
                Ok(())
            }

            DataProcessingReg::MulAdd {
                is_64bit,
                is_subtract,
                dest,
                multiplicand,
                multiplier,
                addend,
            } => {
                let a = cpu.regs.get_x(*multiplicand);
                let b = cpu.regs.get_x(*multiplier);
                let c = cpu.regs.get_x(*addend);

                let product = a.wrapping_mul(b);
                let result =
                    if *is_subtract { c.wrapping_sub(product) } else { c.wrapping_add(product) };

                cpu.regs.set_x(*dest, truncate_result(result, *is_64bit));
                cpu.advance_pc();
                Ok(())
            }
        }
    }

    fn execute_load_store(
        &self,
        cpu: &mut Cpu,
        instr: &LoadStore,
        bus: &mut dyn Bus,
    ) -> Result<(), CpuError> {
        match instr {
            LoadStore::ImmediateOffset {
                access_size,
                is_load,
                is_signed,
                data_reg,
                base_reg,
                byte_offset,
                writeback,
            } => {
                let base = cpu.get_reg_or_sp(*base_reg, true);
                let effective_address = match writeback {
                    WritebackMode::PreIndex => (base as i64).wrapping_add(*byte_offset) as u64,
                    _ => base,
                };

                if *is_load {
                    let value =
                        self.load_memory(bus, effective_address, *access_size, *is_signed)?;
                    cpu.regs.set_x(*data_reg, value);
                } else {
                    let value = cpu.regs.get_x(*data_reg);
                    self.store_memory(bus, effective_address, *access_size, value)?;
                }

                if *writeback != WritebackMode::None {
                    let final_address = (base as i64).wrapping_add(*byte_offset) as u64;
                    cpu.set_reg_or_sp(*base_reg, final_address, true);
                }

                cpu.advance_pc();
                Ok(())
            }

            LoadStore::RegisterOffset {
                access_size,
                is_load,
                data_reg,
                base_reg,
                offset_reg,
                extend,
                scale_by_size,
            } => {
                let base = cpu.get_reg_or_sp(*base_reg, true);
                let mut offset = cpu.regs.get_x(*offset_reg);

                offset = match extend {
                    super::decoder::ExtendType::Uxtw => offset as u32 as u64,
                    super::decoder::ExtendType::Sxtw => (offset as i32) as i64 as u64,
                    _ => offset,
                };

                if *scale_by_size {
                    offset <<= *access_size as u8;
                }

                let effective_address = base.wrapping_add(offset);

                if *is_load {
                    let value = self.load_memory(bus, effective_address, *access_size, false)?;
                    cpu.regs.set_x(*data_reg, value);
                } else {
                    let value = cpu.regs.get_x(*data_reg);
                    self.store_memory(bus, effective_address, *access_size, value)?;
                }

                cpu.advance_pc();
                Ok(())
            }

            LoadStore::Pair {
                is_64bit,
                is_load,
                first_reg,
                second_reg,
                base_reg,
                byte_offset,
                writeback,
            } => {
                let element_size = if *is_64bit { 8u64 } else { 4 };
                let access_size = if *is_64bit { AccessSize::Doubleword } else { AccessSize::Word };
                let base = cpu.get_reg_or_sp(*base_reg, true);

                let effective_address = match writeback {
                    WritebackMode::PreIndex => (base as i64).wrapping_add(*byte_offset) as u64,
                    _ => base,
                };

                if *is_load {
                    let first_value =
                        self.load_memory(bus, effective_address, access_size, false)?;
                    let second_value = self.load_memory(
                        bus,
                        effective_address.wrapping_add(element_size),
                        access_size,
                        false,
                    )?;
                    cpu.regs.set_x(*first_reg, first_value);
                    cpu.regs.set_x(*second_reg, second_value);
                } else {
                    let first_value = cpu.regs.get_x(*first_reg);
                    let second_value = cpu.regs.get_x(*second_reg);
                    self.store_memory(bus, effective_address, access_size, first_value)?;
                    self.store_memory(
                        bus,
                        effective_address.wrapping_add(element_size),
                        access_size,
                        second_value,
                    )?;
                }

                if *writeback != WritebackMode::None {
                    let final_address = (base as i64).wrapping_add(*byte_offset) as u64;
                    cpu.set_reg_or_sp(*base_reg, final_address, true);
                }

                cpu.advance_pc();
                Ok(())
            }

            LoadStore::PcRelativeLiteral { is_64bit, data_reg, byte_offset } => {
                let address = (cpu.regs.pc as i64).wrapping_add(*byte_offset) as u64;
                let access_size = if *is_64bit { AccessSize::Doubleword } else { AccessSize::Word };
                let value = self.load_memory(bus, address, access_size, false)?;
                cpu.regs.set_x(*data_reg, value);
                cpu.advance_pc();
                Ok(())
            }
        }
    }

    fn execute_branch(&self, cpu: &mut Cpu, instr: &Branch) -> Result<(), CpuError> {
        match instr {
            Branch::Unconditional { with_link, byte_offset } => {
                if *with_link {
                    cpu.regs.x[30] = cpu.regs.pc.wrapping_add(4);
                }
                cpu.regs.pc = (cpu.regs.pc as i64).wrapping_add(*byte_offset) as u64;
                Ok(())
            }

            Branch::Conditional { condition, byte_offset } => {
                if cpu.check_condition(*condition)? {
                    cpu.regs.pc = (cpu.regs.pc as i64).wrapping_add(*byte_offset) as u64;
                } else {
                    cpu.advance_pc();
                }
                Ok(())
            }

            Branch::CompareZero { is_64bit, branch_if_nonzero, test_reg, byte_offset } => {
                let value = if *is_64bit {
                    cpu.regs.get_x(*test_reg)
                } else {
                    cpu.regs.get_x(*test_reg) as u32 as u64
                };

                let should_branch = if *branch_if_nonzero { value != 0 } else { value == 0 };

                if should_branch {
                    cpu.regs.pc = (cpu.regs.pc as i64).wrapping_add(*byte_offset) as u64;
                } else {
                    cpu.advance_pc();
                }
                Ok(())
            }

            Branch::TestBit { branch_if_set, test_reg, bit_position, byte_offset } => {
                let value = cpu.regs.get_x(*test_reg);
                let bit_is_set = (value >> *bit_position) & 1 != 0;

                let should_branch = *branch_if_set == bit_is_set;

                if should_branch {
                    cpu.regs.pc = (cpu.regs.pc as i64).wrapping_add(*byte_offset) as u64;
                } else {
                    cpu.advance_pc();
                }
                Ok(())
            }

            Branch::ToRegister { operation, target_reg } => {
                let target = cpu.regs.get_x(*target_reg);

                match operation {
                    BranchRegOp::BranchWithLink => {
                        cpu.regs.x[30] = cpu.regs.pc.wrapping_add(4);
                        cpu.regs.pc = target;
                    }
                    BranchRegOp::Branch | BranchRegOp::Return => {
                        cpu.regs.pc = target;
                    }
                }
                Ok(())
            }
        }
    }

    fn execute_system(&self, cpu: &mut Cpu, instr: &SystemInstr) -> Result<(), CpuError> {
        match instr {
            SystemInstr::Nop | SystemInstr::Yield => {
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::WaitForInterrupt | SystemInstr::WaitForEvent => {
                cpu.halted = true;
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::SendEvent | SystemInstr::SendEventLocal => {
                cpu.halted = false;
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::ReadSystemReg { dest, sys_reg: _ } => {
                cpu.regs.set_x(*dest, 0);
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::WriteSystemReg { src: _, sys_reg: _ } => {
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::Barrier { .. } => {
                cpu.advance_pc();
                Ok(())
            }

            SystemInstr::SupervisorCall { immediate } => {
                Err(CpuError::Exception(format!("svc #{immediate}")))
            }

            SystemInstr::HypervisorCall { immediate } => {
                Err(CpuError::Exception(format!("hvc #{immediate}")))
            }

            SystemInstr::SecureMonitorCall { immediate } => {
                Err(CpuError::Exception(format!("smc #{immediate}")))
            }

            SystemInstr::Breakpoint { immediate } => {
                Err(CpuError::Exception(format!("breakpoint #{immediate}")))
            }

            SystemInstr::Halt { immediate: _ } => {
                cpu.halted = true;
                Ok(())
            }
        }
    }

    fn load_memory(
        &self,
        bus: &mut dyn Bus,
        address: u64,
        size: AccessSize,
        signed: bool,
    ) -> Result<u64, CpuError> {
        let result = match size {
            AccessSize::Byte => bus
                .read_u8(address)
                .map(|v| if signed { (v as i8) as i64 as u64 } else { v as u64 }),
            AccessSize::Halfword => bus
                .read_u16(address)
                .map(|v| if signed { (v as i16) as i64 as u64 } else { v as u64 }),
            AccessSize::Word => bus
                .read_u32(address)
                .map(|v| if signed { (v as i32) as i64 as u64 } else { v as u64 }),
            AccessSize::Doubleword => bus.read_u64(address),
        };

        result.map_err(|e| CpuError::Exception(format!("load at {address:#x} failed: {e}")))
    }

    fn store_memory(
        &self,
        bus: &mut dyn Bus,
        address: u64,
        size: AccessSize,
        value: u64,
    ) -> Result<(), CpuError> {
        let result = match size {
            AccessSize::Byte => bus.write_u8(address, value as u8),
            AccessSize::Halfword => bus.write_u16(address, value as u16),
            AccessSize::Word => bus.write_u32(address, value as u32),
            AccessSize::Doubleword => bus.write_u64(address, value),
        };

        result.map_err(|e| CpuError::Exception(format!("store at {address:#x} failed: {e}")))
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

fn add_with_flags_64(a: u64, b: u64) -> (u64, bool, bool) {
    let result = a.wrapping_add(b);
    let carry = result < a;
    let overflow = ((a ^ !b) & (a ^ result)) >> 63 != 0;
    (result, carry, overflow)
}

fn sub_with_flags_64(a: u64, b: u64) -> (u64, bool, bool) {
    let result = a.wrapping_sub(b);
    let carry = a >= b;
    let overflow = ((a ^ b) & (a ^ result)) >> 63 != 0;
    (result, carry, overflow)
}

fn add_with_flags_32(a: u32, b: u32) -> (u32, bool, bool) {
    let result = a.wrapping_add(b);
    let carry = result < a;
    let overflow = ((a ^ !b) & (a ^ result)) >> 31 != 0;
    (result, carry, overflow)
}

fn sub_with_flags_32(a: u32, b: u32) -> (u32, bool, bool) {
    let result = a.wrapping_sub(b);
    let carry = a >= b;
    let overflow = ((a ^ b) & (a ^ result)) >> 31 != 0;
    (result, carry, overflow)
}

fn apply_shift(value: u64, shift_type: ShiftType, amount: u8, is_64bit: bool) -> u64 {
    let amount = amount as u32;
    let datasize = if is_64bit { 64 } else { 32 };

    if amount >= datasize {
        return match shift_type {
            ShiftType::LogicalLeft | ShiftType::LogicalRight => 0,
            ShiftType::ArithmeticRight => {
                if is_64bit {
                    ((value as i64) >> 63) as u64
                } else {
                    ((value as i32) >> 31) as u64
                }
            }
            ShiftType::RotateRight => value,
        };
    }

    match shift_type {
        ShiftType::LogicalLeft => value << amount,
        ShiftType::LogicalRight => {
            if is_64bit {
                value >> amount
            } else {
                ((value as u32) >> amount) as u64
            }
        }
        ShiftType::ArithmeticRight => {
            if is_64bit {
                ((value as i64) >> amount) as u64
            } else {
                (((value as u32) as i32) >> amount) as u64
            }
        }
        ShiftType::RotateRight => {
            if is_64bit {
                value.rotate_right(amount)
            } else {
                ((value as u32).rotate_right(amount)) as u64
            }
        }
    }
}

fn rotate_right(value: u64, amount: u32, datasize: u32) -> u64 {
    if datasize == 64 {
        value.rotate_right(amount)
    } else {
        ((value as u32).rotate_right(amount)) as u64
    }
}

fn create_bitmask(width: u32, datasize: u32) -> u64 {
    let len = width + 1;
    if len >= datasize {
        if datasize == 64 { u64::MAX } else { (1u64 << datasize) - 1 }
    } else {
        (1u64 << len) - 1
    }
}

fn sign_extend_from_bit(value: u64, bit: u8) -> u64 {
    let sign_bit = 1u64 << bit;
    if (value & sign_bit) != 0 { value | !((1u64 << (bit + 1)) - 1) } else { value }
}

fn apply_logical_op(op: LogicalOp, a: u64, b: u64) -> u64 {
    match op {
        LogicalOp::And | LogicalOp::AndSetFlags => a & b,
        LogicalOp::Or => a | b,
        LogicalOp::Xor => a ^ b,
    }
}

fn truncate_result(value: u64, is_64bit: bool) -> u64 {
    if is_64bit { value } else { value & 0xffffffff }
}

fn is_negative(value: u64, is_64bit: bool) -> bool {
    if is_64bit { (value as i64) < 0 } else { (value as u32 as i32) < 0 }
}

fn is_zero(value: u64, is_64bit: bool) -> bool {
    if is_64bit { value == 0 } else { (value as u32) == 0 }
}
