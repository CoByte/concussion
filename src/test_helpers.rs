use std::{
    fs::{File, Permissions},
    io::Write,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    process::{Command, Output},
    thread::sleep,
    time::Duration,
};

use tempdir::TempDir;

pub fn create_and_run_bin(binary: &[u8]) -> Output {
    let dir = TempDir::new("hello_world").unwrap();

    let elf_path = dir.path().join("elf");
    let mut elf = File::create(elf_path.clone()).unwrap();
    elf.write_all(binary).unwrap();

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

    Command::new(elf_path).output().unwrap()
}
