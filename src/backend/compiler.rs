use std::{collections::HashMap, u32};

use iced_x86::{
    code_asm::{
        self,
        asm_traits::{CodeAsmAdd, CodeAsmMov, CodeAsmSub, CodeAsmZero_bytes},
        CodeAssembler, CodeLabel,
    },
    IcedError,
};

use crate::{
    frontend::parser::{Instruction, IR},
    segment,
};

use super::elf::{PhdrFlags, SegmentBuilder};

use code_asm as asm;

const CELL_BUFFER_LENGTH: u32 = 30_000;

// dataptr = RCX/ECX

fn emit_shift_left(
    a: &mut CodeAssembler,
    amount: u32, // precondition: amount <= |CELL_BUFFER_LENGTH|
) -> Result<(), IcedError> {
    let mut l = a.create_label();

    a.sub(asm::ecx, amount)?;
    a.jno(l)?;
    a.sub(asm::ecx, u32::MAX - CELL_BUFFER_LENGTH)?;

    a.set_label(&mut l)?;

    Ok(())
}

fn emit_shift_right(
    a: &mut CodeAssembler,
    amount: u32, // precondition: amount <= |CELL_BUFFER_LENGTH|
) -> Result<(), IcedError> {
    let mut l = a.create_label();

    a.add(asm::ecx, amount)?;
    a.cmp(asm::ecx, CELL_BUFFER_LENGTH)?;
    a.jl(l)?;
    a.add(asm::ecx, u32::MAX - CELL_BUFFER_LENGTH)?;

    a.set_label(&mut l)?;

    Ok(())
}

fn emit_add(a: &mut CodeAssembler, amount: u8) -> Result<(), IcedError> {
    // iced should use imm8 (https://github.com/icedland/iced/issues/384)
    a.add(asm::byte_ptr(asm::rcx), amount as u32)?;

    Ok(())
}

fn emit_sub(a: &mut CodeAssembler, amount: u8) -> Result<(), IcedError> {
    a.add(asm::byte_ptr(asm::rcx), amount as u32)?;

    Ok(())
}

fn emit_write(a: &mut CodeAssembler) -> Result<(), IcedError> {
    a.mov(asm::rax, 1u64)?;
    a.mov(asm::rdi, 1u64)?;
    a.mov(asm::rsi, asm::rcx)?;
    a.mov(asm::rdx, 1u64)?;

    Ok(())
}

fn emit_jump_forward(
    a: &mut CodeAssembler,
    target: CodeLabel,
    position: &mut CodeLabel,
) -> Result<(), IcedError> {
    a.cmp(asm::byte_ptr(asm::rcx), 0)?;
    a.je(target)?;

    a.set_label(position)?;

    Ok(())
}

fn emit_jump_backward(
    a: &mut CodeAssembler,
    target: CodeLabel,
    position: &mut CodeLabel,
) -> Result<(), IcedError> {
    a.cmp(asm::byte_ptr(asm::rcx), 0)?;
    a.jne(target)?;

    a.set_label(position)?;

    Ok(())
}

struct Compiler {
    instructions: IR,
}

impl SegmentBuilder for Compiler {
    fn code(
        &self,
        labels: &HashMap<&'static str, u64>,
    ) -> Result<super::elf::Segment, iced_x86::IcedError> {
        let mut a = CodeAssembler::new(64)?;

        let mut _start = a.create_label();
        a.set_label(&mut _start)?;

        // setup
        a.mov(asm::rcx, *labels.get("cell_buffer").unwrap())?;

        let mut jump_labels: HashMap<u64, CodeLabel> = self
            .instructions
            .0
            .iter()
            .enumerate()
            .filter_map(|(c, i)| match i {
                Instruction::JumpForward(_) | Instruction::JumpBackward(_) => {
                    Some((c as u64, a.create_label()))
                }
                _ => None,
            })
            .collect();

        for (i, instr) in self.instructions.0.iter().enumerate() {
            use Instruction as I;
            match instr {
                I::ShiftLeft(v) => {
                    let v: u32 = (*v).try_into().unwrap();
                    emit_shift_left(&mut a, v % CELL_BUFFER_LENGTH)?
                } // bad!!!!
                I::ShiftRight(v) => {
                    let v: u32 = (*v).try_into().unwrap();
                    emit_shift_right(&mut a, v % CELL_BUFFER_LENGTH)?
                }
                I::Add(v) => emit_add(&mut a, *v)?,
                I::Sub(v) => emit_sub(&mut a, *v)?,
                I::Read => todo!(),
                I::Write => emit_write(&mut a)?,
                I::JumpForward(v) => {
                    let target = *jump_labels.get(v).unwrap();
                    let position = jump_labels.get_mut(&(i as u64)).unwrap();
                    emit_jump_forward(&mut a, target, position)?;
                }
                I::JumpBackward(v) => {
                    let target = *jump_labels.get(v).unwrap();
                    let position = jump_labels.get_mut(&(i as u64)).unwrap();
                    emit_jump_backward(&mut a, target, position)?;
                }
            }
        }

        // instructions may emit tailing label
        a.zero_bytes()?;

        Ok(segment!(a, _start))
    }

    fn flags(&self) -> super::elf::PhdrFlags {
        PhdrFlags::X | PhdrFlags::R
    }
}
