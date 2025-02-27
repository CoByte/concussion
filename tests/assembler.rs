use pretty_assertions::assert_eq;
use tempdir::TempDir;

use std::fs::Permissions;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use std::{collections::HashMap, fs::File, io::Write};

use concussion::assembler::{
    compile_to_elf, PhdrFlags, Segment, SegmentBuilder,
};
use concussion::segment;
use iced_x86::{
    code_asm::{self, CodeAssembler},
    IcedError,
};

#[test]
fn hello_world() {
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

    let binary = compile_to_elf(&[&DataSegment, &TextSegment]).unwrap();

    let dir = TempDir::new("hello_world").unwrap();

    let elf_path = dir.path().join("elf");
    let mut elf = File::create(elf_path.clone()).unwrap();
    elf.write_all(&binary[..]).unwrap();

    elf.set_permissions(Permissions::from_mode(0o755)).unwrap();

    // I don't understand any of this, but it's needed to allow an executable
    // to be generated and ran
    // See: https://github.com/rust-lang/rust/issues/114554#issue-1838269767
    sleep(Duration::from_micros(2));
    unsafe { libc::flock(elf.as_raw_fd(), libc::LOCK_EX) };

    drop(elf);

    let file = File::open(elf_path.clone()).unwrap();
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH) };
    drop(file);

    let output = Command::new(elf_path).output().unwrap();

    assert_eq!(output.stdout, HELLO);
}
