use crate::error::CpuError;

#[derive(Debug, Clone)]
pub enum Instruction {
    DataProcessingImm(DataProcessingImm),
    DataProcessingReg(DataProcessingReg),
    LoadStore(LoadStore),
    Branch(Branch),
    System(SystemInstr),
    SimdFp,
    Undefined { opcode: u32 },
}

#[derive(Debug, Clone)]
pub enum DataProcessingImm {
    AddSubImm {
        is_64bit: bool,
        is_subtract: bool,
        set_flags: bool,
        dest: u8,
        src: u8,
        immediate: u16,
        shift_by_12: bool,
    },
    MoveWide {
        is_64bit: bool,
        operation: MoveWideOp,
        shift_amount: u8,
        dest: u8,
        immediate: u16,
    },
    LogicalImm {
        is_64bit: bool,
        operation: LogicalOp,
        dest: u8,
        src: u8,
        bitmask: u64,
    },
    PcRelAddr {
        is_page_relative: bool,
        dest: u8,
        offset: i64,
    },
    Bitfield {
        is_64bit: bool,
        operation: BitfieldOp,
        dest: u8,
        src: u8,
        rotate: u8,
        width: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveWideOp {
    Negate,
    Zero,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
    Xor,
    AndSetFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitfieldOp {
    SignedExtract,
    Insert,
    UnsignedExtract,
}

#[derive(Debug, Clone)]
pub enum DataProcessingReg {
    LogicalShifted {
        is_64bit: bool,
        operation: LogicalOp,
        invert_second: bool,
        dest: u8,
        first_src: u8,
        second_src: u8,
        shift_type: ShiftType,
        shift_amount: u8,
    },
    AddSubShifted {
        is_64bit: bool,
        is_subtract: bool,
        set_flags: bool,
        dest: u8,
        first_src: u8,
        second_src: u8,
        shift_type: ShiftType,
        shift_amount: u8,
    },
    ConditionalSelect {
        is_64bit: bool,
        is_negate: bool,
        else_operation: u8,
        dest: u8,
        if_src: u8,
        else_src: u8,
        condition: u8,
    },
    MulAdd {
        is_64bit: bool,
        is_subtract: bool,
        dest: u8,
        multiplicand: u8,
        multiplier: u8,
        addend: u8,
    },
}

#[derive(Debug, Clone)]
pub enum LoadStore {
    ImmediateOffset {
        access_size: AccessSize,
        is_load: bool,
        is_signed: bool,
        data_reg: u8,
        base_reg: u8,
        byte_offset: i64,
        writeback: WritebackMode,
    },
    RegisterOffset {
        access_size: AccessSize,
        is_load: bool,
        data_reg: u8,
        base_reg: u8,
        offset_reg: u8,
        extend: ExtendType,
        scale_by_size: bool,
    },
    Pair {
        is_64bit: bool,
        is_load: bool,
        first_reg: u8,
        second_reg: u8,
        base_reg: u8,
        byte_offset: i64,
        writeback: WritebackMode,
    },
    PcRelativeLiteral {
        is_64bit: bool,
        data_reg: u8,
        byte_offset: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessSize {
    Byte = 0,
    Halfword = 1,
    Word = 2,
    Doubleword = 3,
}

impl AccessSize {
    fn from_encoding(value: u8) -> Self {
        match value {
            0 => Self::Byte,
            1 => Self::Halfword,
            2 => Self::Word,
            _ => Self::Doubleword,
        }
    }

    fn byte_count(&self) -> u64 {
        1 << (*self as u64)
    }
}

#[derive(Debug, Clone)]
pub enum Branch {
    Unconditional { with_link: bool, byte_offset: i64 },
    Conditional { condition: u8, byte_offset: i64 },
    CompareZero { is_64bit: bool, branch_if_nonzero: bool, test_reg: u8, byte_offset: i64 },
    TestBit { branch_if_set: bool, test_reg: u8, bit_position: u8, byte_offset: i64 },
    ToRegister { operation: BranchRegOp, target_reg: u8 },
}

#[derive(Debug, Clone)]
pub enum SystemInstr {
    SupervisorCall { immediate: u16 },
    HypervisorCall { immediate: u16 },
    SecureMonitorCall { immediate: u16 },
    Breakpoint { immediate: u16 },
    Halt { immediate: u16 },
    ReadSystemReg { dest: u8, sys_reg: u16 },
    WriteSystemReg { src: u8, sys_reg: u16 },
    Barrier { operation: BarrierOp },
    WaitForInterrupt,
    WaitForEvent,
    SendEvent,
    SendEventLocal,
    Yield,
    Nop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftType {
    LogicalLeft = 0,
    LogicalRight = 1,
    ArithmeticRight = 2,
    RotateRight = 3,
}

impl ShiftType {
    fn from_encoding(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::LogicalLeft),
            1 => Some(Self::LogicalRight),
            2 => Some(Self::ArithmeticRight),
            3 => Some(Self::RotateRight),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendType {
    Uxtb = 0,
    Uxth = 1,
    Uxtw = 2,
    Uxtx = 3,
    Sxtb = 4,
    Sxth = 5,
    Sxtw = 6,
    Sxtx = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritebackMode {
    None,
    PreIndex,
    PostIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchRegOp {
    Branch,
    BranchWithLink,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierOp {
    DataSync(u8),
    DataMemory(u8),
    InstructionSync,
}

pub struct Decoder;

impl Decoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, opcode: u32, pc: u64) -> Result<Instruction, CpuError> {
        let main_group = extract_bits(opcode, 25, 4);

        match main_group {
            0b1000 | 0b1001 => self.decode_data_processing_imm(opcode, pc),
            0b1010 | 0b1011 => self.decode_branch_system(opcode, pc),
            0b0100 | 0b0110 | 0b1100 | 0b1110 => self.decode_load_store(opcode, pc),
            0b0101 | 0b1101 => self.decode_data_processing_reg(opcode, pc),
            0b0111 | 0b1111 => Ok(Instruction::SimdFp),
            _ => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_data_processing_imm(&self, opcode: u32, _pc: u64) -> Result<Instruction, CpuError> {
        let subgroup = extract_bits(opcode, 23, 3);

        match subgroup {
            0b000 | 0b001 => self.decode_pc_relative_addr(opcode),
            0b010 => self.decode_add_sub_immediate(opcode),
            0b100 => self.decode_logical_immediate(opcode),
            0b101 => self.decode_move_wide(opcode),
            0b110 => self.decode_bitfield(opcode),
            _ => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_pc_relative_addr(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let is_page_relative = bit_set(opcode, 31);
        let dest = extract_bits(opcode, 0, 5) as u8;
        let imm_low = extract_bits(opcode, 29, 2);
        let imm_high = extract_bits(opcode, 5, 19);
        let raw_imm = (imm_high << 2) | imm_low;
        let signed_imm = sign_extend(raw_imm, 21);

        let offset = if is_page_relative { signed_imm * 4096 } else { signed_imm };

        Ok(Instruction::DataProcessingImm(DataProcessingImm::PcRelAddr {
            is_page_relative,
            dest,
            offset,
        }))
    }

    fn decode_add_sub_immediate(&self, opcode: u32) -> Result<Instruction, CpuError> {
        Ok(Instruction::DataProcessingImm(DataProcessingImm::AddSubImm {
            is_64bit: bit_set(opcode, 31),
            is_subtract: bit_set(opcode, 30),
            set_flags: bit_set(opcode, 29),
            shift_by_12: bit_set(opcode, 22),
            immediate: extract_bits(opcode, 10, 12) as u16,
            src: extract_bits(opcode, 5, 5) as u8,
            dest: extract_bits(opcode, 0, 5) as u8,
        }))
    }

    fn decode_logical_immediate(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let is_64bit = bit_set(opcode, 31);
        let opc = extract_bits(opcode, 29, 2) as u8;
        let n_bit = bit_set(opcode, 22);
        let rotate = extract_bits(opcode, 16, 6) as u8;
        let width = extract_bits(opcode, 10, 6) as u8;
        let src = extract_bits(opcode, 5, 5) as u8;
        let dest = extract_bits(opcode, 0, 5) as u8;

        let operation = match opc {
            0 => LogicalOp::And,
            1 => LogicalOp::Or,
            2 => LogicalOp::Xor,
            _ => LogicalOp::AndSetFlags,
        };

        match decode_bitmask(n_bit, width, rotate, is_64bit) {
            Some(bitmask) => Ok(Instruction::DataProcessingImm(DataProcessingImm::LogicalImm {
                is_64bit,
                operation,
                dest,
                src,
                bitmask,
            })),
            None => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_move_wide(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let opc = extract_bits(opcode, 29, 2) as u8;
        let operation = match opc {
            0 => MoveWideOp::Negate,
            2 => MoveWideOp::Zero,
            _ => MoveWideOp::Keep,
        };

        Ok(Instruction::DataProcessingImm(DataProcessingImm::MoveWide {
            is_64bit: bit_set(opcode, 31),
            operation,
            shift_amount: (extract_bits(opcode, 21, 2) as u8) * 16,
            dest: extract_bits(opcode, 0, 5) as u8,
            immediate: extract_bits(opcode, 5, 16) as u16,
        }))
    }

    fn decode_bitfield(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let opc = extract_bits(opcode, 29, 2) as u8;
        let operation = match opc {
            0 => BitfieldOp::SignedExtract,
            1 => BitfieldOp::Insert,
            _ => BitfieldOp::UnsignedExtract,
        };

        Ok(Instruction::DataProcessingImm(DataProcessingImm::Bitfield {
            is_64bit: bit_set(opcode, 31),
            operation,
            dest: extract_bits(opcode, 0, 5) as u8,
            src: extract_bits(opcode, 5, 5) as u8,
            rotate: extract_bits(opcode, 16, 6) as u8,
            width: extract_bits(opcode, 10, 6) as u8,
        }))
    }

    fn decode_branch_system(&self, opcode: u32, _pc: u64) -> Result<Instruction, CpuError> {
        if self.is_unconditional_branch(opcode) {
            return self.decode_unconditional_branch(opcode);
        }
        if self.is_conditional_branch(opcode) {
            return self.decode_conditional_branch(opcode);
        }
        if self.is_compare_and_branch(opcode) {
            return self.decode_compare_and_branch(opcode);
        }
        if self.is_test_and_branch(opcode) {
            return self.decode_test_and_branch(opcode);
        }
        if self.is_exception_instruction(opcode) {
            return self.decode_exception_instruction(opcode);
        }
        if self.is_branch_to_register(opcode) {
            return self.decode_branch_to_register(opcode);
        }
        if self.is_system_instruction(opcode) {
            return self.decode_system_instruction(opcode);
        }
        Ok(Instruction::Undefined { opcode })
    }

    fn is_unconditional_branch(&self, opcode: u32) -> bool {
        extract_bits(opcode, 26, 5) == 0b00101
    }

    fn decode_unconditional_branch(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let imm26 = opcode & 0x3ffffff;
        let byte_offset = sign_extend(imm26, 26) * 4;
        Ok(Instruction::Branch(Branch::Unconditional {
            with_link: bit_set(opcode, 31),
            byte_offset,
        }))
    }

    fn is_conditional_branch(&self, opcode: u32) -> bool {
        extract_bits(opcode, 24, 8) == 0b01010100 && !bit_set(opcode, 4)
    }

    fn decode_conditional_branch(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let imm19 = extract_bits(opcode, 5, 19);
        let byte_offset = sign_extend(imm19, 19) * 4;
        Ok(Instruction::Branch(Branch::Conditional {
            condition: (opcode & 0xf) as u8,
            byte_offset,
        }))
    }

    fn is_compare_and_branch(&self, opcode: u32) -> bool {
        extract_bits(opcode, 25, 6) == 0b011010
    }

    fn decode_compare_and_branch(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let imm19 = extract_bits(opcode, 5, 19);
        let byte_offset = sign_extend(imm19, 19) * 4;
        Ok(Instruction::Branch(Branch::CompareZero {
            is_64bit: bit_set(opcode, 31),
            branch_if_nonzero: bit_set(opcode, 24),
            test_reg: (opcode & 0x1f) as u8,
            byte_offset,
        }))
    }

    fn is_test_and_branch(&self, opcode: u32) -> bool {
        extract_bits(opcode, 25, 6) == 0b011011
    }

    fn decode_test_and_branch(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let bit_high = extract_bits(opcode, 31, 1);
        let bit_low = extract_bits(opcode, 19, 5);
        let imm14 = extract_bits(opcode, 5, 14);
        let byte_offset = sign_extend(imm14, 14) * 4;
        Ok(Instruction::Branch(Branch::TestBit {
            branch_if_set: bit_set(opcode, 24),
            test_reg: (opcode & 0x1f) as u8,
            bit_position: ((bit_high << 5) | bit_low) as u8,
            byte_offset,
        }))
    }

    fn is_exception_instruction(&self, opcode: u32) -> bool {
        extract_bits(opcode, 24, 8) == 0b11010100
    }

    fn decode_exception_instruction(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let exception_type = extract_bits(opcode, 21, 3);
        let exception_level = opcode & 0x3;
        let immediate = extract_bits(opcode, 5, 16) as u16;

        match (exception_type, exception_level) {
            (0b000, 0b01) => Ok(Instruction::System(SystemInstr::SupervisorCall { immediate })),
            (0b000, 0b10) => Ok(Instruction::System(SystemInstr::HypervisorCall { immediate })),
            (0b000, 0b11) => Ok(Instruction::System(SystemInstr::SecureMonitorCall { immediate })),
            (0b001, 0b00) => Ok(Instruction::System(SystemInstr::Breakpoint { immediate })),
            (0b010, 0b00) => Ok(Instruction::System(SystemInstr::Halt { immediate })),
            _ => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn is_branch_to_register(&self, opcode: u32) -> bool {
        extract_bits(opcode, 25, 7) == 0b1101011
    }

    fn decode_branch_to_register(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let op_field = extract_bits(opcode, 21, 4);
        let target_reg = extract_bits(opcode, 5, 5) as u8;

        let operation = match op_field {
            0b0000 => BranchRegOp::Branch,
            0b0001 => BranchRegOp::BranchWithLink,
            0b0010 => BranchRegOp::Return,
            _ => return Ok(Instruction::Undefined { opcode }),
        };

        Ok(Instruction::Branch(Branch::ToRegister { operation, target_reg }))
    }

    fn is_system_instruction(&self, opcode: u32) -> bool {
        extract_bits(opcode, 22, 10) == 0b1101010100
    }

    fn decode_system_instruction(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let is_read = bit_set(opcode, 21);
        let op0 = extract_bits(opcode, 19, 2);
        let op1 = extract_bits(opcode, 16, 3);
        let crn = extract_bits(opcode, 12, 4);
        let crm = extract_bits(opcode, 8, 4);
        let op2 = extract_bits(opcode, 5, 3);
        let register = (opcode & 0x1f) as u8;

        if op0 == 0 && op1 == 0b011 && crn == 0b0010 && register == 0b11111 {
            return self.decode_hint_instruction(crm, op2, opcode);
        }

        if op0 == 0 && op1 == 0b011 && crn == 0b0011 {
            return self.decode_barrier_instruction(op2, crm, opcode);
        }

        if op0 >= 2 {
            let sys_reg = ((op0 << 14) | (op1 << 11) | (crn << 7) | (crm << 3) | op2) as u16;
            return if is_read {
                Ok(Instruction::System(SystemInstr::ReadSystemReg { dest: register, sys_reg }))
            } else {
                Ok(Instruction::System(SystemInstr::WriteSystemReg { src: register, sys_reg }))
            };
        }

        Ok(Instruction::Undefined { opcode })
    }

    fn decode_hint_instruction(
        &self,
        crm: u32,
        op2: u32,
        _opcode: u32,
    ) -> Result<Instruction, CpuError> {
        match (crm, op2) {
            (0, 0) => Ok(Instruction::System(SystemInstr::Nop)),
            (0, 1) => Ok(Instruction::System(SystemInstr::Yield)),
            (0, 2) => Ok(Instruction::System(SystemInstr::WaitForEvent)),
            (0, 3) => Ok(Instruction::System(SystemInstr::WaitForInterrupt)),
            (0, 4) => Ok(Instruction::System(SystemInstr::SendEvent)),
            (0, 5) => Ok(Instruction::System(SystemInstr::SendEventLocal)),
            _ => Ok(Instruction::System(SystemInstr::Nop)),
        }
    }

    fn decode_barrier_instruction(
        &self,
        op2: u32,
        crm: u32,
        opcode: u32,
    ) -> Result<Instruction, CpuError> {
        match op2 {
            0b100 => Ok(Instruction::System(SystemInstr::Barrier {
                operation: BarrierOp::DataSync(crm as u8),
            })),
            0b101 => Ok(Instruction::System(SystemInstr::Barrier {
                operation: BarrierOp::DataMemory(crm as u8),
            })),
            0b110 => Ok(Instruction::System(SystemInstr::Barrier {
                operation: BarrierOp::InstructionSync,
            })),
            _ => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_load_store(&self, opcode: u32, _pc: u64) -> Result<Instruction, CpuError> {
        let op0 = extract_bits(opcode, 28, 4);
        let op1 = extract_bits(opcode, 26, 1);
        let op2 = extract_bits(opcode, 23, 2);
        let op4 = extract_bits(opcode, 10, 2);

        if self.is_load_literal(op0, op1) {
            return self.decode_load_literal(opcode);
        }

        if self.is_load_store_pair(op0, op1) {
            return self.decode_load_store_pair(opcode, op2);
        }

        if bit_set(opcode, 26) {
            return Ok(Instruction::SimdFp);
        }

        self.decode_load_store_register(opcode, op2, op4)
    }

    fn is_load_literal(&self, op0: u32, op1: u32) -> bool {
        (op0 & 0b0011) == 0 && op1 == 0
    }

    fn decode_load_literal(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let size_field = extract_bits(opcode, 30, 2);
        let imm19 = extract_bits(opcode, 5, 19);
        let byte_offset = sign_extend(imm19, 19) * 4;
        Ok(Instruction::LoadStore(LoadStore::PcRelativeLiteral {
            is_64bit: size_field != 0,
            data_reg: (opcode & 0x1f) as u8,
            byte_offset,
        }))
    }

    fn is_load_store_pair(&self, op0: u32, op1: u32) -> bool {
        (op0 & 0b0011) == 0b0010 && op1 == 0
    }

    fn decode_load_store_pair(&self, opcode: u32, op2: u32) -> Result<Instruction, CpuError> {
        let size_field = extract_bits(opcode, 30, 2);
        let is_64bit = (size_field & 0b10) != 0;
        let scale = if is_64bit { 8 } else { 4 };

        let writeback = match op2 {
            0b01 => WritebackMode::PostIndex,
            0b11 => WritebackMode::PreIndex,
            0b10 => WritebackMode::None,
            _ => return Ok(Instruction::Undefined { opcode }),
        };

        let imm7 = extract_bits(opcode, 15, 7);
        let byte_offset = sign_extend(imm7, 7) * scale;

        Ok(Instruction::LoadStore(LoadStore::Pair {
            is_64bit,
            is_load: bit_set(opcode, 22),
            first_reg: (opcode & 0x1f) as u8,
            second_reg: extract_bits(opcode, 10, 5) as u8,
            base_reg: extract_bits(opcode, 5, 5) as u8,
            byte_offset,
            writeback,
        }))
    }

    fn decode_load_store_register(
        &self,
        opcode: u32,
        op2: u32,
        op4: u32,
    ) -> Result<Instruction, CpuError> {
        let access_size = AccessSize::from_encoding(extract_bits(opcode, 30, 2) as u8);
        let load_store_opc = extract_bits(opcode, 22, 2);
        let is_load = (load_store_opc & 1) != 0;
        let is_signed = (load_store_opc & 2) != 0;
        let data_reg = (opcode & 0x1f) as u8;
        let base_reg = extract_bits(opcode, 5, 5) as u8;

        if (op2 & 0b10) != 0 && bit_set(opcode, 24) {
            let imm12 = extract_bits(opcode, 10, 12);
            let byte_offset = (imm12 as i64) * (access_size.byte_count() as i64);
            return Ok(Instruction::LoadStore(LoadStore::ImmediateOffset {
                access_size,
                is_load,
                is_signed,
                data_reg,
                base_reg,
                byte_offset,
                writeback: WritebackMode::None,
            }));
        }

        if (op2 & 0b10) == 0 {
            if op4 == 0b10 {
                return self.decode_register_offset(
                    opcode,
                    access_size,
                    is_load,
                    data_reg,
                    base_reg,
                );
            }

            let imm9 = extract_bits(opcode, 12, 9);
            let byte_offset = sign_extend(imm9, 9);
            let writeback = match op4 {
                0b00 => WritebackMode::None,
                0b01 => WritebackMode::PostIndex,
                0b11 => WritebackMode::PreIndex,
                _ => return Ok(Instruction::Undefined { opcode }),
            };

            return Ok(Instruction::LoadStore(LoadStore::ImmediateOffset {
                access_size,
                is_load,
                is_signed,
                data_reg,
                base_reg,
                byte_offset,
                writeback,
            }));
        }

        Ok(Instruction::Undefined { opcode })
    }

    fn decode_register_offset(
        &self,
        opcode: u32,
        access_size: AccessSize,
        is_load: bool,
        data_reg: u8,
        base_reg: u8,
    ) -> Result<Instruction, CpuError> {
        let offset_reg = extract_bits(opcode, 16, 5) as u8;
        let extend_option = extract_bits(opcode, 13, 3) as u8;
        let scale_by_size = bit_set(opcode, 12);

        let extend = match extend_option {
            0b010 => ExtendType::Uxtw,
            0b011 => ExtendType::Uxtx,
            0b110 => ExtendType::Sxtw,
            0b111 => ExtendType::Sxtx,
            _ => return Ok(Instruction::Undefined { opcode }),
        };

        Ok(Instruction::LoadStore(LoadStore::RegisterOffset {
            access_size,
            is_load,
            data_reg,
            base_reg,
            offset_reg,
            extend,
            scale_by_size,
        }))
    }

    fn decode_data_processing_reg(&self, opcode: u32, _pc: u64) -> Result<Instruction, CpuError> {
        let op1 = extract_bits(opcode, 28, 1);
        let op2 = extract_bits(opcode, 21, 4);

        if op1 == 0 && (op2 & 0b1000) == 0 {
            return self.decode_logical_shifted(opcode);
        }

        if op1 == 0 && (op2 & 0b1001) == 0b1000 {
            return self.decode_add_sub_shifted(opcode);
        }

        if op1 == 1 && (op2 & 0b1000) == 0 {
            return self.decode_conditional_select(opcode);
        }

        if op1 == 1 && (op2 & 0b1000) != 0 {
            return self.decode_mul_add(opcode);
        }

        Ok(Instruction::Undefined { opcode })
    }

    fn decode_logical_shifted(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let opc = extract_bits(opcode, 29, 2) as u8;
        let shift_encoding = extract_bits(opcode, 22, 2) as u8;

        let operation = match opc {
            0 => LogicalOp::And,
            1 => LogicalOp::Or,
            2 => LogicalOp::Xor,
            _ => LogicalOp::AndSetFlags,
        };

        match ShiftType::from_encoding(shift_encoding) {
            Some(shift_type) => {
                Ok(Instruction::DataProcessingReg(DataProcessingReg::LogicalShifted {
                    is_64bit: bit_set(opcode, 31),
                    operation,
                    invert_second: bit_set(opcode, 21),
                    dest: (opcode & 0x1f) as u8,
                    first_src: extract_bits(opcode, 5, 5) as u8,
                    second_src: extract_bits(opcode, 16, 5) as u8,
                    shift_type,
                    shift_amount: extract_bits(opcode, 10, 6) as u8,
                }))
            }
            None => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_add_sub_shifted(&self, opcode: u32) -> Result<Instruction, CpuError> {
        let shift_encoding = extract_bits(opcode, 22, 2) as u8;

        match ShiftType::from_encoding(shift_encoding) {
            Some(shift_type) => {
                Ok(Instruction::DataProcessingReg(DataProcessingReg::AddSubShifted {
                    is_64bit: bit_set(opcode, 31),
                    is_subtract: bit_set(opcode, 30),
                    set_flags: bit_set(opcode, 29),
                    dest: (opcode & 0x1f) as u8,
                    first_src: extract_bits(opcode, 5, 5) as u8,
                    second_src: extract_bits(opcode, 16, 5) as u8,
                    shift_type,
                    shift_amount: extract_bits(opcode, 10, 6) as u8,
                }))
            }
            None => Ok(Instruction::Undefined { opcode }),
        }
    }

    fn decode_conditional_select(&self, opcode: u32) -> Result<Instruction, CpuError> {
        Ok(Instruction::DataProcessingReg(DataProcessingReg::ConditionalSelect {
            is_64bit: bit_set(opcode, 31),
            is_negate: bit_set(opcode, 30),
            else_operation: extract_bits(opcode, 10, 2) as u8,
            dest: (opcode & 0x1f) as u8,
            if_src: extract_bits(opcode, 5, 5) as u8,
            else_src: extract_bits(opcode, 16, 5) as u8,
            condition: extract_bits(opcode, 12, 4) as u8,
        }))
    }

    fn decode_mul_add(&self, opcode: u32) -> Result<Instruction, CpuError> {
        Ok(Instruction::DataProcessingReg(DataProcessingReg::MulAdd {
            is_64bit: bit_set(opcode, 31),
            is_subtract: bit_set(opcode, 15),
            dest: (opcode & 0x1f) as u8,
            multiplicand: extract_bits(opcode, 5, 5) as u8,
            multiplier: extract_bits(opcode, 16, 5) as u8,
            addend: extract_bits(opcode, 10, 5) as u8,
        }))
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_bits(value: u32, start: u32, count: u32) -> u32 {
    (value >> start) & ((1 << count) - 1)
}

fn bit_set(value: u32, bit: u32) -> bool {
    (value >> bit) & 1 != 0
}

fn sign_extend(value: u32, bits: u32) -> i64 {
    let shift = 32 - bits;
    ((value as i32) << shift >> shift) as i64
}

fn decode_bitmask(n_bit: bool, width: u8, rotate: u8, is_64bit: bool) -> Option<u64> {
    let len = if n_bit { 6 } else { (width as u32).leading_zeros().saturating_sub(26) as u8 };

    if len == 0 {
        return None;
    }
    if !is_64bit && n_bit {
        return None;
    }

    let element_size = 1u32 << len;
    let element_mask = (1u64 << element_size) - 1;
    let ones_count = (width as u32) & (element_size - 1);
    let rotation = (rotate as u32) & (element_size - 1);

    if ones_count == element_size - 1 {
        return None;
    }

    let unrotated = (1u64 << (ones_count + 1)) - 1;
    let rotated =
        ((unrotated >> rotation) | (unrotated << (element_size - rotation))) & element_mask;

    let mut result = 0u64;
    let mut position = 0;
    while position < 64 {
        result |= rotated << position;
        position += element_size as u64;
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_add_immediate() {
        let decoder = Decoder::new();
        let result = decoder.decode(0x91000a20, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn decodes_unconditional_branch() {
        let decoder = Decoder::new();
        let result = decoder.decode(0x14000000, 0);
        assert!(matches!(
            result,
            Ok(Instruction::Branch(Branch::Unconditional { with_link: false, .. }))
        ));
    }
}
