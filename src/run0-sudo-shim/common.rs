// SPDX-License-Identifier: BSD-3-Clause

use std::{
    ffi::OsStr,
    fmt::Display,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::exit,
};

pub static POLKIT_STDIN_AGENT: &str = match option_env!("POLKIT_STDIN_AGENT") {
    Some(x) => x,
    None => "polkit-stdin-agent",
};

pub static RUN0_CMD: &str = match option_env!("RUN0") {
    Some(x) => x,
    None => "run0",
};

pub static TRUE_CMD: &str = match option_env!("TRUE") {
    Some(x) => x,
    None => "true",
};

pub fn die(msg: &str) -> ! {
    eprintln!("run0-sudo-shim: {msg}");
    exit(1)
}

fn is_executable(p: &Path) -> bool {
    fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn resolve_command(cmd: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    if cmd.contains('/') {
        let p = PathBuf::from(cmd);
        return is_executable(&p).then_some(p);
    }
    std::env::split_paths(path?)
        .map(|dir| dir.join(cmd))
        .find(|p| is_executable(p))
}

#[derive(PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum Error {
    Unsupported(String),
    InvalidUsage(String),
    UnknownUser(String),
    UnknownGroup(String),
    PrintHelp,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(feat) => {
                f.write_fmt(format_args!("{} is currently unsupported", feat))
            }
            Self::InvalidUsage(msg) => f.write_str(msg),
            Self::PrintHelp => f.write_str(""), // FIXME
            Error::UnknownUser(u) => f.write_fmt(format_args!("unknown user: {u}")),
            Error::UnknownGroup(g) => f.write_fmt(format_args!("unknown group: {g}")),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Eq, PartialEq)]
pub struct ShimResult {
    pub cli: Vec<String>,
    pub list_command: Option<Vec<String>>,
    stderr: String,
    stdout: String,
}

impl ShimResult {
    pub fn new() -> Self {
        Self {
            cli: Vec::new(),
            list_command: None,
            stderr: String::new(),
            stdout: String::new(),
        }
    }

    #[cfg(test)]
    pub fn ok_from(cli: Vec<String>) -> Result<Self, Error> {
        Ok(Self {
            cli,
            list_command: None,
            stderr: String::new(),
            stdout: String::new(),
        })
    }
    #[cfg(test)]
    pub fn get_stderr(&self) -> &str {
        &self.stderr
    }

    #[cfg(test)]
    pub fn get_stdout(&self) -> &str {
        &self.stdout
    }

    pub fn push_stderr(&mut self, line: impl std::fmt::Display) {
        use std::fmt::Write;
        write!(self.stderr, "{line}").unwrap();
    }
    pub fn push_stdout(&mut self, line: impl std::fmt::Display) {
        use std::fmt::Write;
        write!(self.stderr, "{line}").unwrap();
    }
}

pub struct Run0Cli {
    pub res: Result<ShimResult, Error>,
    cmd: clap::Command,
}

impl Run0Cli {
    pub fn new(res: Result<ShimResult, Error>, cmd: clap::Command) -> Self {
        Self { res, cmd }
    }

    // CAN EXIT(1)
    pub fn finalize(mut self) -> (Vec<String>, Option<Vec<String>>) {
        let res = match self.res {
            Ok(res) => res,
            Err(e) => match e {
                Error::PrintHelp => {
                    self.cmd.print_help().ok();
                    exit(1);
                }
                _ => die(&format!("{}", e)),
            },
        };
        if !res.stderr.is_empty() {
            eprintln!("{}", res.stderr);
        }
        if !res.stdout.is_empty() {
            println!("{}", res.stdout);
        }
        (res.cli, res.list_command)
    }
}
