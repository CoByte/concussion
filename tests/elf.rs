use concussion::backend::compiler::CompilerError;
use concussion::test_helpers::create_and_run_bin;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::result::Result;

use concussion::backend::elf::{
    compile_to_elf, LabelMap, PhdrFlags, Segment, SegmentBuilder,
};
use concussion::{backend, segment};
use iced_x86::{
    code_asm::{self, CodeAssembler},
    IcedError,
};

#[test]
fn hello_world() {
    const HELLO: &[u8] = b"Hello world!\n";

    struct DataSegment;

    impl SegmentBuilder for DataSegment {
        fn code(&self, _labels: &LabelMap) -> Result<Segment, CompilerError> {
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
        fn code(&self, labels: &LabelMap) -> Result<Segment, CompilerError> {
            use code_asm as asm;

            let hello = labels.get("hello")?;

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

    let binary = compile_to_elf(&[&DataSegment, &TextSegment]).unwrap();
    let output = create_and_run_bin(&binary[..]);

    assert_eq!(output.stdout, HELLO);
}
