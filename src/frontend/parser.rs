use derive_more::TryFrom;
use itertools::Itertools;
use thiserror::Error;

#[derive(Clone, Copy, Debug, TryFrom)]
#[try_from(repr)]
#[repr(u8)]
enum Command {
    Movr = b'>',
    Movl = b'<',
    Incr = b'+',
    Decr = b'-',
    Writ = b'.',
    Read = b',',
    JmpF = b'[',
    JmpB = b']',
}

pub struct Program {
    instrs: Vec<Command>,
}

impl From<&str> for Program {
    fn from(value: &str) -> Self {
        let instrs = value.bytes().flat_map(|c| c.try_into()).collect();

        Program { instrs }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Instruction {
    ShiftLeft(u64),
    ShiftRight(u64),
    Add(u8),
    Sub(u8),
    Read,
    Write,
    JumpForward(u64),
    JumpBackward(u64),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Missing matching brace for {0} at position: {1}")]
    NestingError(char, usize),
}

pub struct IR(pub Vec<Instruction>);

fn compute_jumps(instrs: &mut [Instruction]) -> Result<(), ParseError> {
    use Instruction as I;
    fn find_bracket_offset(
        mut subprogram: impl Iterator<Item = Instruction>,
    ) -> Option<usize> {
        let mut bracket_nesting: i32 = 0;
        subprogram.position(|elem| {
            match elem {
                I::JumpForward(_) => bracket_nesting += 1,
                I::JumpBackward(_) => bracket_nesting -= 1,
                _ => (),
            };

            bracket_nesting == 0
        })
    }

    for pc in 0..instrs.len() {
        let po = pc as u64;
        instrs[pc] = match instrs[pc] {
            I::JumpForward(_) => I::JumpForward(
                po + find_bracket_offset(instrs[pc..].iter().copied())
                    .ok_or(ParseError::NestingError('[', pc))?
                    as u64,
            ),
            I::JumpBackward(_) => I::JumpBackward(
                po - find_bracket_offset(instrs[..=pc].iter().rev().copied())
                    .ok_or(ParseError::NestingError(']', pc))?
                    as u64,
            ),
            v => v,
        };
    }

    Ok(())
}

impl IR {
    pub fn parse(program: &Program) -> Result<Self, ParseError> {
        use Command as C;
        use Instruction as I;
        let mut parsed: Vec<_> = program
            .instrs
            .iter()
            .dedup_by_with_count(|l, r| {
                matches!(
                    (l, r),
                    (C::Movr, C::Movr)
                        | (C::Movl, C::Movl)
                        | (C::Incr, C::Incr)
                        | (C::Decr, C::Decr)
                )
            })
            .map(|(count, code)| match code {
                C::Movr => I::ShiftRight(count as u64),
                C::Movl => I::ShiftLeft(count as u64),
                C::Incr => I::Add((count % 255) as u8),
                C::Decr => I::Sub((count % 255) as u8),
                C::Writ => I::Write,
                C::Read => I::Read,
                C::JmpF => I::JumpForward(0),
                C::JmpB => I::JumpBackward(0),
            })
            .collect();

        compute_jumps(&mut parsed[..])?;

        Ok(IR(parsed))
    }
}
