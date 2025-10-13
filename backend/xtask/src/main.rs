use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(author, version, about = "Developer tasks for the OpenGuild backend")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    #[command(about = "Run `cargo fmt --all`")]
    Fmt,
    #[command(about = "Run format + clippy lint checks")]
    Lint,
    #[command(about = "Execute `cargo test --workspace`")]
    Test,
    #[command(about = "Run fmt + clippy + test sequence")]
    Ci,
    #[command(about = "Compile server with metrics and verify `/metrics` endpoint")]
    CiMetricsSmoke,
}

fn main() {
    let cli = Cli::parse();

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .expect("workspace root");

    let result = match cli.command {
        Task::Fmt => run_commands(&workspace_root, [("cargo", &["fmt", "--all"])]),
        Task::Lint => run_commands(
            &workspace_root,
            [
                ("cargo", &["fmt", "--all", "--", "--check"]),
                ("cargo", &["clippy", "--workspace", "--", "-D", "warnings"]),
            ],
        ),
        Task::Test => run_commands(&workspace_root, [("cargo", &["test", "--workspace"])]),
        Task::Ci => run_ci(&workspace_root),
        Task::CiMetricsSmoke => run_ci_metrics_smoke(&workspace_root),
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

fn run_ci_metrics_smoke(workspace_root: &PathBuf) -> bool {
    const READY_PATH: &str = "ready";
    const METRICS_PATH: &str = "metrics";

    if !run_commands(
        workspace_root,
        [(
            "cargo",
            &["build", "--features", "metrics", "-p", "openguild-server"],
        )],
    ) {
        return false;
    }

    let port = match reserve_port() {
        Some(port) => port,
        None => {
            eprintln!("failed to reserve a free TCP port");
            return false;
        }
    };
    let bind_addr = format!("127.0.0.1:{port}");

    let mut child = match Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--features",
            "metrics",
            "-p",
            "openguild-server",
            "--",
            "--bind-addr",
            &bind_addr,
            "--metrics-enabled",
            "true",
        ])
        .env("RUST_LOG", "warn")
        .current_dir(workspace_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            eprintln!("failed to launch openguild-server: {err}");
            return false;
        }
    };

    let result = match Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => {
            let base_url = format!("http://{bind_addr}");
            let ready_url = format!("{base_url}/{READY_PATH}");
            let metrics_url = format!("{base_url}/{METRICS_PATH}");

            if !wait_for_ready(&client, &ready_url, Duration::from_secs(30)) {
                eprintln!("server failed to report ready state within timeout");
                false
            } else {
                verify_metrics(&client, &metrics_url)
            }
        }
        Err(err) => {
            eprintln!("failed to build HTTP client: {err}");
            false
        }
    };

    if let Err(err) = child.kill() {
        if err.kind() != std::io::ErrorKind::InvalidInput {
            eprintln!("failed to terminate server process: {err}");
        }
    }
    let _ = child.wait();

    result
}

fn reserve_port() -> Option<u16> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .ok()
}

fn wait_for_ready(client: &Client, url: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match client.get(url).send() {
            Ok(response) if response.status().is_success() => return true,
            Ok(_) | Err(_) => sleep(Duration::from_millis(500)),
        }
    }
    false
}

fn verify_metrics(client: &Client, url: &str) -> bool {
    match client.get(url).send() {
        Ok(response) if response.status() == StatusCode::OK => match response.text() {
            Ok(body) => {
                if body.contains("openguild_http_requests_total") {
                    true
                } else {
                    eprintln!("metrics endpoint responded without expected counters");
                    false
                }
            }
            Err(err) => {
                eprintln!("failed to read metrics body: {err}");
                false
            }
        },
        Ok(response) => {
            eprintln!("unexpected metrics response status: {}", response.status());
            false
        }
        Err(err) => {
            eprintln!("failed to call metrics endpoint: {err}");
            false
        }
    }
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
