//! Replaces this process with Claude Code, so signals, the TTY and the exit
//! code belong to `claude` with no wrapper in between.

use crate::ports::{Error, Launcher, Plan};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

pub struct ExecLauncher {
    program: String,
}

impl ExecLauncher {
    pub fn new(program: String) -> Self {
        ExecLauncher { program }
    }
}

impl Launcher for ExecLauncher {
    fn available(&self) -> bool {
        if self.program.contains('/') {
            return Path::new(&self.program).is_file();
        }
        std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(&self.program).is_file()))
    }

    fn launch(&self, plan: &Plan) -> Error {
        let mut cmd = Command::new(&self.program);
        cmd.args(&plan.args);
        for k in &plan.unset {
            cmd.env_remove(k);
        }
        cmd.envs(plan.set.iter().map(|(k, v)| (k, v)));
        if let Some(dir) = &plan.dir {
            cmd.current_dir(dir);
        }
        let e = cmd.exec();
        Error::Io(format!("cannot start {}: {e}", self.program))
    }
}
