use derive_more::TryFrom;

#[derive(Clone, Copy, Debug, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
enum Instruction {
    Movr = b'>',
    Movl = b'<',
    Incr = b'+',
    Decr = b'-',
    Writ = b'.',
    Read = b',',
    JmpF = b'[',
    JmpB = b']',
}

struct Program {
    instrs: Vec<Instruction>,
}

impl From<&str> for Program {
    fn from(value: &str) -> Self {
        let instrs = value.bytes().flat_map(|c| c.try_into()).collect();

        Program { instrs }
    }
}
