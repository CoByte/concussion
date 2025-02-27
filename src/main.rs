use std::{collections::HashMap, fs::File, io::Write};

use assembler::{compile_to_elf, PhdrFlags, Segment, SegmentBuilder};
use iced_x86::{
    code_asm::{self, CodeAssembler},
    IcedError,
};

mod assembler;

const HELLO: &[u8] = b"Hello world!\n";

struct DataSegment;

impl SegmentBuilder for DataSegment {
    fn code(
        &self,
        _labels: &HashMap<&'static str, u64>,
    ) -> Result<Segment, IcedError> {
        let mut a = CodeAssembler::new(64)?;

        let mut hello = a.create_label();
        a.set_label(&mut hello)?;
        a.db(HELLO)?;

        Ok(segment![a, hello])
    }

    fn flags(&self) -> PhdrFlags {
        PhdrFlags::R | PhdrFlags::W
    }
}

struct TextSegment;

impl SegmentBuilder for TextSegment {
    fn code(
        &self,
        labels: &HashMap<&'static str, u64>,
    ) -> Result<Segment, IcedError> {
        use code_asm as asm;

        let hello = labels["hello"];

        let mut a = CodeAssembler::new(64)?;

        let mut _start = a.create_label();
        a.set_label(&mut _start)?;

        a.mov(asm::rax, 1u64)?;
        a.mov(asm::rdi, 1u64)?;
        a.mov(asm::rsi, hello)?;
        a.mov(asm::rdx, HELLO.len() as u64)?;
        a.syscall()?;

        a.mov(asm::rax, 60u64)?;
        a.mov(asm::rdi, 0u64)?;
        a.syscall()?;

        Ok(segment![a, _start])
    }

    fn flags(&self) -> PhdrFlags {
        PhdrFlags::R | PhdrFlags::X
    }
}

fn main() {
    let binary = compile_to_elf(&[&DataSegment, &TextSegment])
        .expect("You probably forgot a patch");

    let mut file = File::create("output").unwrap();
    file.write_all(&binary[..]).unwrap();
}
