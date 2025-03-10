use std::{collections::HashMap, u32};

use iced_x86::{
    code_asm::{
        self,
        asm_traits::{
            CodeAsmAdd, CodeAsmInt, CodeAsmMov, CodeAsmSub, CodeAsmZero_bytes,
        },
        CodeAssembler, CodeLabel,
    },
    IcedError,
};
use thiserror::Error;

use crate::{
    frontend::parser::{Instruction, IR},
    segment,
};

use super::elf::{compile_to_elf, LabelMap, PhdrFlags, SegmentBuilder};

use code_asm as asm;

#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("could not generate asm: {0}")]
    Assemble(#[from] IcedError),
    #[error("missing patch")]
    MissingPatch,
    #[error("missing _start label")]
    MissingEntryPoint,
    #[error("missing label: {0}")]
    MissingLabel(&'static str),
}

const CELL_BUFFER_LENGTH: u32 = 30_000;

// dataptr = RCX/ECX

fn emit_shift_left(
    a: &mut CodeAssembler,
    base: u32,
    amount: u32, // precondition: amount <= |CELL_BUFFER_LENGTH|
) -> Result<(), IcedError> {
    let mut l = a.create_label();

    a.lea(asm::rcx, asm::rcx - amount)?;
    a.cmp(asm::ecx, base)?;
    a.jae(l)?;
    a.lea(asm::rcx, asm::rcx + CELL_BUFFER_LENGTH)?;

    a.set_label(&mut l)?;

    Ok(())
}

fn emit_shift_right(
    a: &mut CodeAssembler,
    base: u32,
    amount: u32, // precondition: amount <= |CELL_BUFFER_LENGTH|
) -> Result<(), IcedError> {
    let mut l = a.create_label();

    a.lea(asm::rcx, asm::rcx + amount)?;
    a.cmp(asm::ecx, base + CELL_BUFFER_LENGTH)?;
    a.jb(l)?;
    a.sub(asm::ecx, CELL_BUFFER_LENGTH)?;

    a.set_label(&mut l)?;

    Ok(())
}

fn emit_add(a: &mut CodeAssembler, amount: u8) -> Result<(), IcedError> {
    // iced should use imm8 (https://github.com/icedland/iced/issues/384)
    a.add(asm::byte_ptr(asm::rcx), amount as u32)?;

    Ok(())
}

fn emit_sub(a: &mut CodeAssembler, amount: u8) -> Result<(), IcedError> {
    a.sub(asm::byte_ptr(asm::rcx), amount as u32)?;

    Ok(())
}

fn emit_write(a: &mut CodeAssembler) -> Result<(), IcedError> {
    a.mov(asm::r15, asm::rcx)?;
    a.mov(asm::rax, 1u64)?;
    a.mov(asm::rdi, 1u64)?;
    a.mov(asm::rsi, asm::rcx)?;
    a.mov(asm::rdx, 1u64)?;
    a.syscall()?;
    a.mov(asm::rcx, asm::r15)?;

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

struct DataSegment;

impl SegmentBuilder for DataSegment {
    fn code(
        &self,
        _labels: &LabelMap,
    ) -> Result<super::elf::Segment, CompilerError> {
        let mut a = CodeAssembler::new(64)?;

        let mut cell_buffer = a.create_label();
        a.set_label(&mut cell_buffer)?;
        a.db(&[0u8; CELL_BUFFER_LENGTH as usize])?;

        Ok(segment!(a, cell_buffer))
    }

    fn flags(&self) -> PhdrFlags {
        PhdrFlags::R | PhdrFlags::W
    }
}

struct TextSegment {
    instructions: IR,
}

impl SegmentBuilder for TextSegment {
    fn code(
        &self,
        labels: &LabelMap,
    ) -> Result<super::elf::Segment, CompilerError> {
        let mut a = CodeAssembler::new(64)?;

        let mut _start = a.create_label();
        a.set_label(&mut _start)?;

        // setup
        let buffer_start = labels.get("cell_buffer")?;
        a.mov(asm::rcx, buffer_start)?;

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
                    let v: u32 =
                        (v % CELL_BUFFER_LENGTH as u64).try_into().unwrap();
                    emit_shift_left(&mut a, buffer_start as u32, v)?
                }
                I::ShiftRight(v) => {
                    let v: u32 =
                        (v % CELL_BUFFER_LENGTH as u64).try_into().unwrap();
                    emit_shift_right(&mut a, buffer_start as u32, v)?
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

        // end!
        a.mov(asm::rax, 60u64)?;
        a.mov(asm::rdi, 0u64)?;
        a.syscall()?;

        Ok(segment!(a, _start))
    }

    fn flags(&self) -> super::elf::PhdrFlags {
        PhdrFlags::X | PhdrFlags::R
    }
}

pub fn compile(ir: IR) -> Result<Vec<u8>, CompilerError> {
    let ts = TextSegment { instructions: ir };

    compile_to_elf(&[&DataSegment, &ts])
}
