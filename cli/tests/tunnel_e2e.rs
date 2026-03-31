//! End-to-end integration test for rs-rok tunnel.
//!
//! Orchestrates:
//!   1. mock-service on a free port
//!   2. wrangler dev (local Cloudflare Worker)
//!   3. rs-rok CLI connecting to the local Worker
//!   4. HTTP round-trip through the tunnel
//!
//! Run with: cargo test -p rs-rok-cli --test tunnel_e2e -- --ignored
//! (marked ignored so it doesn't run in normal `cargo test`)
//!
//! Prerequisites: `cargo build --workspace` must have been run first.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind to free port");
    listener.local_addr().unwrap().port()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn target_debug_dir() -> PathBuf {
    workspace_root().join("target").join("debug")
}

struct ProcessGuard {
    child: Child,
    name: &'static str,
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        eprintln!("[cleanup] killed {}", self.name);
    }
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn wait_for_http(url: &str, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(url).send() {
            if resp.status().is_success() {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

#[test]
#[ignore] // requires wrangler + built binaries; run with --ignored
fn tunnel_round_trip() {
    let debug_dir = target_debug_dir();
    let mock_bin = if cfg!(windows) {
        debug_dir.join("mock-service.exe")
    } else {
        debug_dir.join("mock-service")
    };
    let cli_bin = if cfg!(windows) {
        debug_dir.join("rs-rok.exe")
    } else {
        debug_dir.join("rs-rok")
    };

    assert!(
        mock_bin.exists(),
        "mock-service binary not found at {:?}. Run `cargo build --workspace` first.",
        mock_bin
    );
    assert!(
        cli_bin.exists(),
        "rs-rok binary not found at {:?}. Run `cargo build --workspace` first.",
        cli_bin
    );

    // 1. Start mock-service
    let mock_port = free_port();
    let mock = Command::new(&mock_bin)
        .args(["--port", &mock_port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start mock-service");
    let _mock_guard = ProcessGuard {
        child: mock,
        name: "mock-service",
    };

    assert!(
        wait_for_port(mock_port, Duration::from_secs(5)),
        "mock-service did not start on port {}",
        mock_port
    );
    eprintln!("[e2e] mock-service ready on port {}", mock_port);

    // 2. Start wrangler dev
    let worker_port = free_port();
    let wrangler = Command::new("bun")
        .args([
            "wrangler",
            "dev",
            "--port",
            &worker_port.to_string(),
            "--local",
        ])
        .current_dir(workspace_root().join("worker"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start wrangler dev");
    let _wrangler_guard = ProcessGuard {
        child: wrangler,
        name: "wrangler",
    };

    let worker_url = format!("http://127.0.0.1:{}", worker_port);
    assert!(
        wait_for_http(
            &format!("{}/health", worker_url),
            Duration::from_secs(30)
        ),
        "wrangler dev did not become ready on port {}",
        worker_port
    );
    eprintln!("[e2e] wrangler dev ready on port {}", worker_port);

    // 3. Start rs-rok CLI
    let cli_endpoint = format!("ws://127.0.0.1:{}", worker_port);
    let mut cli = Command::new(&cli_bin)
        .args(["http", &mock_port.to_string()])
        .env("RS_ROK_ENDPOINT", &cli_endpoint)
        .env("RS_ROK_TOKEN", "test-token")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rs-rok cli");

    // Read CLI stderr to find the public URL
    let stderr = cli.stderr.take().unwrap();
    let reader = BufReader::new(stderr);
    let mut tunnel_url = String::new();

    let timeout = Duration::from_secs(15);
    let start = std::time::Instant::now();
    for line in reader.lines() {
        if start.elapsed() > timeout {
            break;
        }
        let line = line.expect("read line");
        eprintln!("[cli] {}", line);
        if line.contains("/tunnel/") {
            if let Some(url_start) = line.find("http") {
                let url_part = &line[url_start..];
                if let Some(end) = url_part.find(char::is_whitespace) {
                    tunnel_url = url_part[..end].to_string();
                } else {
                    tunnel_url = url_part.to_string();
                }
                break;
            }
        }
    }
    let _cli_guard = ProcessGuard {
        child: cli,
        name: "rs-rok",
    };

    assert!(
        !tunnel_url.is_empty(),
        "failed to extract tunnel URL from CLI output"
    );
    eprintln!("[e2e] tunnel URL: {}", tunnel_url);

    // 4. HTTP round-trip through tunnel
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("{}/echo", tunnel_url))
        .header("X-Test", "rs-rok-e2e")
        .timeout(Duration::from_secs(10))
        .send()
        .expect("tunnel request failed");

    assert_eq!(resp.status(), 200, "expected 200 from tunnel echo");
    let body: serde_json::Value = resp.json().expect("parse json");
    assert_eq!(body["method"], "GET", "method should be echoed");
    eprintln!("[e2e] tunnel round-trip succeeded: {}", body);
}
