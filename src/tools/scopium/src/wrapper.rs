use std::{env, path::PathBuf, process::Command};

struct ScopiumCmd {
    cargo_subcommand: &'static str,
    args: Vec<String>,
    scopium_args: Vec<String>,
}

impl ScopiumCmd {
    fn new(mut old_args: impl Iterator<Item = String>) -> Self {
        let mut args = vec![];
        let mut scopium_args = vec![];

        for arg in old_args.by_ref() {
            if arg.as_str() == "--" {
                break;
            }
            args.push(arg);
        }

        scopium_args.append(&mut old_args.collect());

        Self { cargo_subcommand: "check", args, scopium_args }
    }

    fn path() -> PathBuf {
        let mut path = env::current_exe()
            .expect("current executable path invalid")
            .with_file_name("scopium-driver");

        if cfg!(windows) {
            path.set_extension("exe");
        }

        path
    }

    fn into_std_cmd(self) -> Command {
        let mut cmd = Command::new("cargo");

        let scopium_args: String =
            self.scopium_args.iter().map(|arg| format!("{arg}__SCOPIUM_ARG__")).collect();

        cmd.env("RUSTC_WORKSPACE_WRAPPER", Self::path())
            .env("SCOPIUM_ARGS", scopium_args)
            .arg(self.cargo_subcommand)
            .args(&self.args);

        cmd
    }
}

fn process(old_args: impl Iterator<Item = String>) -> Result<(), i32> {
    let cmd = ScopiumCmd::new(old_args);
    let mut cmd = cmd.into_std_cmd();

    let exit_status =
        cmd.spawn().expect("could not run cargo").wait().expect("failed to wait for cargo?");

    if exit_status.success() { Ok(()) } else { Err(exit_status.code().unwrap_or(-1)) }
}

fn main() {
    if let Err(code) = process(env::args().skip(2)) {
        std::process::exit(code)
    }
}
