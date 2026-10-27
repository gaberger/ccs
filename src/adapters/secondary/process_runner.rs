//! Runs a wrapped status line command with the same stdin (ADR-2610261237).

use crate::ports::CommandRunner;
use std::io::Write;
use std::process::{Command, Stdio};

pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(&self, argv: &[String], stdin: &[u8]) -> Option<String> {
        let (program, args) = argv.split_first()?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        // Feed stdin from a thread: a child that prints before it reads would
        // otherwise deadlock against a full pipe.
        let mut pipe = child.stdin.take()?;
        let input = stdin.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = pipe.write_all(&input);
        });
        let out = child.wait_with_output().ok()?;
        let _ = writer.join();
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}
