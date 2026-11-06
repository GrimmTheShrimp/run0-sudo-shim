// SPDX-License-Identifier: BSD-3-Clause

use std::{
    ffi::OsStr,
    fmt::Display,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
    process::exit,
};

use rustix::{
    fs::{Access, access},
    io::Errno,
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

// systemd default_PATH()
const SYSTEMD_DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin";

fn check_executable(p: &Path) -> Result<(), Errno> {
    let meta = fs::metadata(p).map_err(|e| Errno::from_io_error(&e).unwrap_or(Errno::NOENT))?;
    if !meta.is_file() {
        return Err(Errno::ACCESS);
    }
    access(p, Access::EXEC_OK)
}

fn make_absolute(p: &Path, cwd: Option<&Path>) -> PathBuf {
    let joined = match cwd {
        Some(cwd) if p.is_relative() => cwd.join(p),
        _ => p.to_path_buf(),
    };
    joined
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect()
}

// same lookup as run0 does client side, see find_executable() in systemd
pub fn resolve_command(cmd: &str, path: Option<&OsStr>, cwd: Option<&Path>) -> Option<PathBuf> {
    if cmd.contains('/') {
        let p = Path::new(cmd);
        return match check_executable(p) {
            Ok(()) => Some(make_absolute(p, cwd)),
            Err(Errno::ACCESS | Errno::PERM) => Some(p.to_path_buf()),
            Err(_) => None,
        };
    }
    let path = path.unwrap_or(OsStr::new(SYSTEMD_DEFAULT_PATH));
    path.as_bytes()
        .split(|&b| b == b':')
        .map(|dir| Path::new(OsStr::from_bytes(dir)))
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(cmd))
        .find(|p| check_executable(p).is_ok())
        .map(|p| make_absolute(&p, None))
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
