use core::panic;
use std::{collections::HashMap, iter, marker::PhantomData};

use bitflags::bitflags;
use bytemuck::{bytes_of, Pod};
use iced_x86::{
    code_asm::{CodeAssembler, CodeLabel},
    BlockEncoderOptions, IcedError,
};

struct BinaryBuilder {
    binary: Vec<u8>,
    outstanding_patches: usize,
}

struct Patch<T> {
    index: usize,
    phantom: PhantomData<T>,
}

impl BinaryBuilder {
    fn new() -> Self {
        BinaryBuilder {
            binary: vec![],
            outstanding_patches: 0,
        }
    }

    fn current_addr(&self) -> usize {
        self.binary.len()
    }

    fn emit(&mut self, bytes: impl Pod) {
        self.binary.extend_from_slice(bytes_of(&bytes));
    }

    fn emit_slice(&mut self, bytes: &[u8]) {
        self.binary.extend_from_slice(bytes);
    }

    fn pad(&mut self, count: usize) {
        self.binary.extend(iter::repeat(0).take(count));
    }

    fn pad_to_width(&mut self, width: usize) {
        let over = self.current_addr() % width;
        self.pad(width - over);
    }

    fn mark<T>(&mut self) -> Patch<T> {
        let patch = Patch {
            index: self.current_addr(),
            phantom: PhantomData,
        };

        self.outstanding_patches += 1;
        self.pad(size_of::<T>());

        patch
    }

    fn patch<T>(&mut self, patch: Patch<T>, bytes: T)
    where
        T: Pod,
    {
        self.outstanding_patches -= 1;

        self.binary[patch.index..patch.index + size_of::<T>()]
            .copy_from_slice(bytes_of(&bytes));
    }

    fn build(self) -> Result<Vec<u8>, ()> {
        if self.outstanding_patches == 0 {
            Ok(self.binary)
        } else {
            Err(())
        }
    }
}

bitflags! {
    pub struct PhdrFlags: u32 {
        const X = 1 << 0;
        const W = 1 << 1;
        const R = 1 << 2;
    }
}

pub struct Segment {
    code: CodeAssembler,
    labels: Vec<(&'static str, CodeLabel)>,
}

impl Segment {
    pub fn new(
        code: CodeAssembler,
        labels: Vec<(&'static str, CodeLabel)>,
    ) -> Self {
        Self { code, labels }
    }
}

#[macro_export]
macro_rules! segment {
    ($code:expr, $($label:ident),*) => {
        $crate::backend::elf::Segment::new(
            $code,
            vec![$((stringify!($label), $label)),+]
        )
    };
}

pub trait SegmentBuilder {
    fn code(
        &self,
        labels: &HashMap<&'static str, u64>,
    ) -> Result<Segment, IcedError>;

    fn flags(&self) -> PhdrFlags;

    fn build(
        &self,
        ip: u64,
        labels: &mut HashMap<&'static str, u64>,
    ) -> Result<Vec<u8>, IcedError> {
        let Segment {
            mut code,
            labels: new_labels,
        } = self.code(labels)?;

        let result = code.assemble_options(
            ip,
            BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS,
        )?;

        for (name, label) in new_labels {
            labels.insert(name, result.label_ip(&label)?);
        }

        Ok(result.inner.code_buffer)
    }
}

pub fn compile_to_elf(segments: &[&dyn SegmentBuilder]) -> Result<Vec<u8>, ()> {
    if cfg!(target_endian = "big") {
        panic!("Program is not valid on big-endian architecture!");
    }

    const LOAD_POS: u64 = 0x08048000;
    const PAGE_SIZE: u64 = 0x1000;
    const EHDR_SIZE: u16 = 0x40; // known statically for elf64
    const PHDR_SIZE: u16 = 0x38;

    let mut b = BinaryBuilder::new();

    // === ELF HEADER ===
    b.emit(*b"\x7FELF"); // magic
    b.emit([2u8, 1, 1, 0]); // class, endian, version, abi
    b.pad(8);

    b.emit(2u16); // type
    b.emit(0x3Eu16); // machine
    b.emit(1u32); // version

    let entry_point = b.mark(); // entry point
    let prog_header_offset = b.mark(); // program header table offset
    b.emit(0u64); // section header (none)

    b.emit(0u32); // flags (none)
    b.emit(EHDR_SIZE); // elf header size
    b.emit(PHDR_SIZE); // program header size
    b.emit((segments.len()) as u16); // number of program headers

    b.emit([0u16, 0, 0]); // no section header

    b.patch(prog_header_offset, b.current_addr() as u64);

    // === PHDR HEADERS ===
    let mut seg_patches: Vec<[Patch<u64>; 4]> = Vec::new();
    for seg in segments {
        b.emit(1u32); // segment type: loadable
        b.emit(seg.flags().bits()); // flags: r+x
        let offset = b.mark();
        let vaddr = b.mark();
        b.emit(0u64); // physical memory size is ignored
        let file_size = b.mark();
        let mem_size = b.mark();
        b.emit(PAGE_SIZE);

        seg_patches.push([offset, vaddr, file_size, mem_size]);
    }

    b.pad_to_width(PAGE_SIZE as usize); // get the thing right

    // === SEGMENTS ===
    let mut labels: HashMap<&'static str, u64> = HashMap::new();
    for (seg, patches) in segments.iter().zip(seg_patches) {
        let [offset, vaddr, file_size, mem_size] = patches;

        let file_offset = b.current_addr() as u64;
        let vmem_offset = file_offset + LOAD_POS;
        let source = seg.build(vmem_offset, &mut labels).or(Err(()))?;

        b.patch(offset, file_offset);
        b.patch(vaddr, vmem_offset);
        b.patch(file_size, source.len() as u64);
        b.patch(mem_size, source.len() as u64);

        b.emit_slice(&source[..]);
        b.pad_to_width(PAGE_SIZE as usize);
    }

    b.patch(entry_point, *labels.get("_start").ok_or(())?);

    b.build()
}
