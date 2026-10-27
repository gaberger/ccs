//! AWS CLI profiles, read through the `aws` binary (ADR-2610261238, rule 1).
//! It asks only for names and regions; it never reads a credential.

use crate::ports::{AwsProfiles, Error};
use std::process::{Command, Stdio};

pub struct AwsCli {
    program: String,
}

impl AwsCli {
    pub fn new(program: String) -> Self {
        AwsCli { program }
    }

    fn query(&self, args: &[&str]) -> Result<Option<String>, Error> {
        let out = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| Error::Io(format!("cannot run the AWS CLI ({}): {e}", self.program)))?;
        Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
    }
}

impl AwsProfiles for AwsCli {
    fn names(&self) -> Result<Vec<String>, Error> {
        let text = self
            .query(&["configure", "list-profiles"])?
            .ok_or_else(|| Error::Io("`aws configure list-profiles` failed".into()))?;
        Ok(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
    }

    fn region(&self, profile: &str) -> Option<String> {
        // `aws configure get` exits 1 when the value is unset.
        let text = self.query(&["configure", "get", "region", "--profile", profile]).ok()??;
        let r = text.trim();
        (!r.is_empty()).then(|| r.to_string())
    }
}
