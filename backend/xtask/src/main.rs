use std::env;
use std::path::PathBuf;
use std::process::{exit, Command};

fn main() {
    let mut args = env::args().skip(1);
    let task = match args.next() {
        Some(task) => task,
        None => {
            eprintln!("usage: cargo xtask <fmt|lint|test|ci>");
            exit(1);
        }
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");

    let result = match task.as_str() {
        "fmt" => run_commands(&workspace_root, [("cargo", &["fmt", "--all"])]),
        "lint" => run_commands(
            &workspace_root,
            [
                ("cargo", &["fmt", "--all", "--", "--check"]),
                ("cargo", &["clippy", "--workspace", "--", "-D", "warnings"]),
            ],
        ),
        "test" => run_commands(&workspace_root, [("cargo", &["test", "--workspace"])]),
        "ci" => run_ci(&workspace_root),
        _ => {
            eprintln!("unknown task '{task}'. expected one of: fmt, lint, test, ci");
            false
        }
    };

    if !result {
        exit(1);
    }
}

fn run_ci(workspace_root: &PathBuf) -> bool {
    if !run_commands(
        workspace_root,
        [
            ("cargo", &["fmt", "--all", "--", "--check"]),
            ("cargo", &["clippy", "--workspace", "--", "-D", "warnings"]),
        ],
    ) {
        return false;
    }
    run_commands(workspace_root, [("cargo", &["test", "--workspace"])])
}

fn run_commands<const N: usize>(
    workspace_root: &PathBuf,
    commands: [(&str, &[&str]); N],
) -> bool {
    for (program, args) in commands {
        let status = Command::new(program)
            .args(args)
            .current_dir(workspace_root)
            .status();

        match status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("command '{program} {}' failed with {status}", args.join(" "));
                return false;
            }
            Err(err) => {
                eprintln!("failed to spawn '{program}': {err}");
                return false;
            }
        }
    }
    true
}
