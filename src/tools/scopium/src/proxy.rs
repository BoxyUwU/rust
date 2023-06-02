use std::process::Command;

fn main() -> std::io::Result<()> {
    let rustup_home_dir =
        std::env::var("RUSTUP_HOME").expect("home dir for rustup could not be found");
    let toolchain =
        std::env::var("RUSTUP_TOOLCHAIN").expect("could not find what rustup toolchain was set");

    let executable_path =
        rustup_home_dir + "/toolchains/" + toolchain.as_str() + "/bin/scopium-wrapper";
    let executable_path = executable_path
        + match cfg!(windows) {
            true => ".exe",
            false => "",
        };

    Command::new(executable_path).args(std::env::args().skip(2)).spawn()?.wait()?;
    Ok(())
}
