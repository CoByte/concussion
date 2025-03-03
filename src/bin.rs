use std::{fs::File, io::Write};

use concussion::{
    backend::compiler::compile,
    frontend::parser::{Program, IR},
};

fn main() {
    let source = r#"
    ++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.
    "#;

    let p = source.into();
    let p = IR::parse(&p).unwrap();
    let asm = compile(p).unwrap();

    let mut file = File::create("foo").unwrap();
    file.write_all(&asm).unwrap();
}
