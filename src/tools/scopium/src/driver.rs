#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;

use std::{env, ops::Deref, path::Path, process::exit};

use rustc_interface::interface;
use rustc_session::parse::ParseSess;
use rustc_span::Symbol;

fn arg_value<'a, T: Deref<Target = str>>(
    args: &'a [T],
    find_arg: &str,
    pred: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    let mut args = args.iter().map(Deref::deref);
    while let Some(arg) = args.next() {
        let mut arg = arg.splitn(2, '=');
        if arg.next() != Some(find_arg) {
            continue;
        }

        match arg.next().or_else(|| args.next()) {
            Some(v) if pred(v) => return Some(v),
            _ => {}
        }
    }
    None
}

fn track_scopium_args(parse_sess: &mut ParseSess, args_env_var: &Option<String>) {
    parse_sess
        .env_depinfo
        .get_mut()
        .insert((Symbol::intern("SCOPIUM_ARGS"), args_env_var.as_deref().map(Symbol::intern)));
}

fn track_files(parse_sess: &mut ParseSess) {
    let file_depinfo = parse_sess.file_depinfo.get_mut();

    if let Ok(current_exe) = env::current_exe() {
        if let Some(current_exe) = current_exe.to_str() {
            file_depinfo.insert(Symbol::intern(current_exe));
        }
    }
}

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

struct ScopiumCallbacks {
    scopium_args_var: Option<String>,
}
impl rustc_driver::Callbacks for ScopiumCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let scopium_args_var = self.scopium_args_var.take();
        config.parse_sess_created = Some(Box::new(move |parse_sess| {
            track_scopium_args(parse_sess, &scopium_args_var);
            track_files(parse_sess);
        }));
        config.override_queries = Some(|_sess, providers, _extern_providers| {
            providers.emit_solver_tree = |_, tree| println!("{tree:#?}");
        });
    }
}

fn main() {
    rustc_driver::init_rustc_env_logger();

    exit(rustc_driver::catch_with_exit_code(move || {
        let mut orig_args: Vec<String> = env::args().collect();
        let has_sysroot_arg = arg_value(&orig_args, "--sysroot", |_| true).is_some();

        let sys_root_env = std::env::var("SYSROOT").ok();
        let pass_sysroot_env_if_given = |args: &mut Vec<String>, sys_root_env| {
            if let Some(sys_root) = sys_root_env {
                if !has_sysroot_arg {
                    args.extend(vec!["--sysroot".into(), sys_root]);
                }
            }
        };

        if let Some(pos) = orig_args.iter().position(|arg| arg == "--rustc") {
            orig_args.remove(pos);
            orig_args[0] = "rustc".to_string();

            let mut args: Vec<String> = orig_args.clone();
            pass_sysroot_env_if_given(&mut args, sys_root_env);

            return rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run();
        }

        let wrapper_mode =
            orig_args.get(1).map(Path::new).and_then(Path::file_stem) == Some("rustc".as_ref());

        if wrapper_mode {
            orig_args.remove(1);
        }

        let mut args: Vec<String> = orig_args.clone();
        pass_sysroot_env_if_given(&mut args, sys_root_env);

        let scopium_args_var = env::var("SCOPIUM_ARGS").ok();
        let scopium_args = scopium_args_var
            .as_deref()
            .unwrap_or_default()
            .split("__SCOPIUM_ARG__")
            .filter_map(|s| match s {
                "" => None,
                _ => Some(s.to_string()),
            })
            .chain(vec!["--cfg".into(), r#"feature="cargo-scopium""#.into()])
            .collect::<Vec<String>>();

        args.extend(scopium_args);
        rustc_driver::RunCompiler::new(&args, &mut ScopiumCallbacks { scopium_args_var }).run()
    }))
}
