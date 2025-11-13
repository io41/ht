use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use serde_json::Value;
use nix::libc;
use std::fs;

/// Helper struct to manage ht process and parse events
struct HtSession {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl HtSession {
    /// Spawn ht with the given arguments
    fn new(args: &[&str]) -> Self {
        let ht_path = env!("CARGO_BIN_EXE_ht");
        let mut cmd = Command::new(ht_path);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()); // Suppress stderr for cleaner test output

        let mut child = cmd.spawn().expect("Failed to spawn ht");
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        Self { child, reader }
    }

    /// Read the next event from ht's stdout
    fn read_event(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None, // EOF
            Ok(_) => {
                serde_json::from_str(&line).ok()
            }
            Err(_) => None,
        }
    }

    /// Wait for a specific event type
    fn wait_for_event(&mut self, event_type: &str) -> Option<Value> {
        for _ in 0..100 {
            if let Some(event) = self.read_event() {
                if event.get("type").and_then(|t| t.as_str()) == Some(event_type) {
                    return Some(event);
                }
            }
        }
        None
    }

    /// Send an input command to ht
    fn send_input(&mut self, text: &str) {
        if let Some(stdin) = &mut self.child.stdin {
            let cmd = serde_json::json!({
                "type": "input",
                "payload": text
            });
            writeln!(stdin, "{}", cmd).expect("Failed to write to stdin");
            stdin.flush().expect("Failed to flush stdin");
        }
    }

    /// Wait for the process to exit with timeout
    fn wait_with_timeout(&mut self, timeout: Duration) -> std::io::Result<std::process::ExitStatus> {
        let start = std::time::Instant::now();
        loop {
            match self.child.try_wait()? {
                Some(status) => return Ok(status),
                None => {
                    if start.elapsed() > timeout {
                        self.child.kill()?;
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Process did not exit in time"
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}

impl Drop for HtSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Helper to create a temporary test script
fn create_test_script(code: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let script_path = format!("/tmp/ht_test_{}_{}.sh", std::process::id(), id);
    fs::write(&script_path, code).expect("Failed to write test script");
    fs::set_permissions(&script_path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("Failed to set permissions");
    script_path
}

#[test]
fn test_normal_exit_code_zero() {
    let script = create_test_script("#!/bin/sh\nexit 0");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // Verify exit code is 0
    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(0));
    assert!(data.get("signal").is_none() || data.get("signal").and_then(|s| s.as_i64()).is_none());

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_normal_exit_code_nonzero() {
    let script = create_test_script("#!/bin/sh\nexit 42");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // Verify exit code is 42
    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(42));
    assert!(data.get("signal").is_none() || data.get("signal").and_then(|s| s.as_i64()).is_none());

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_exit_code_1() {
    let script = create_test_script("#!/bin/sh\nexit 1");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(1));
    assert!(data.get("signal").is_none() || data.get("signal").and_then(|s| s.as_i64()).is_none());

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_exit_code_255() {
    let script = create_test_script("#!/bin/sh\nexit 255");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(255));
    assert!(data.get("signal").is_none() || data.get("signal").and_then(|s| s.as_i64()).is_none());

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_signal_termination_sigterm() {
    let script = create_test_script("#!/bin/sh\ntrap '' TERM\nsleep 10");
    let mut session = HtSession::new(&[
        "--subscribe", "init,exit",
        &script
    ]);

    // Wait for init event to ensure process is running
    let init_event = session.wait_for_event("init")
        .expect("Should receive init event");

    let pid = init_event.get("data")
        .and_then(|d| d.get("pid"))
        .and_then(|p| p.as_i64())
        .expect("Init event should have pid");

    // Give the process a moment to set up the trap
    std::thread::sleep(Duration::from_millis(200));

    // Send SIGTERM to the child process
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // Verify signal termination (code 128 + 15 = 143)
    let data = exit_event.get("data").expect("Exit event should have data");
    let code = data.get("code").and_then(|c| c.as_i64());
    let signal = data.get("signal").and_then(|s| s.as_i64());

    assert_eq!(code, Some(143));
    assert_eq!(signal, Some(15));

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_signal_termination_sigkill() {
    let script = create_test_script("#!/bin/sh\nsleep 10");
    let mut session = HtSession::new(&[
        "--subscribe", "init,exit",
        &script
    ]);

    // Wait for init event to ensure process is running
    let init_event = session.wait_for_event("init")
        .expect("Should receive init event");

    let pid = init_event.get("data")
        .and_then(|d| d.get("pid"))
        .and_then(|p| p.as_i64())
        .expect("Init event should have pid");

    // Give the process a moment to start
    std::thread::sleep(Duration::from_millis(100));

    // Send SIGKILL to the child process
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // Verify signal termination (code 128 + 9 = 137)
    let data = exit_event.get("data").expect("Exit event should have data");
    let code = data.get("code").and_then(|c| c.as_i64());
    let signal = data.get("signal").and_then(|s| s.as_i64());

    assert_eq!(code, Some(137));
    assert_eq!(signal, Some(9));

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_subscription_filtering_exit_only() {
    let script = create_test_script("#!/bin/sh\necho hello\nexit 5");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    // Should only receive exit event, not init or output
    let first_event = session.read_event()
        .expect("Should receive at least one event");

    assert_eq!(
        first_event.get("type").and_then(|t| t.as_str()),
        Some("exit"),
        "First event should be exit when subscribed to exit only"
    );

    let data = first_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(5));

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_subscription_filtering_no_exit() {
    let script = create_test_script("#!/bin/sh\necho hello\nexit 7");
    let mut session = HtSession::new(&[
        "--subscribe", "init,output",
        &script
    ]);

    // Should receive init event
    let init_event = session.wait_for_event("init")
        .expect("Should receive init event");
    assert_eq!(
        init_event.get("type").and_then(|t| t.as_str()),
        Some("init")
    );

    // May receive output events
    // But should NOT receive exit event

    // Read remaining events until EOF
    let mut received_exit = false;
    for _ in 0..10 {
        if let Some(event) = session.read_event() {
            if event.get("type").and_then(|t| t.as_str()) == Some("exit") {
                received_exit = true;
                break;
            }
        } else {
            break; // EOF
        }
    }

    assert!(!received_exit, "Should not receive exit event when not subscribed");

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_subprocess_signal_exit_code() {
    // Test the case where a subprocess is killed but the shell exits normally
    let script = create_test_script("#!/bin/sh\nsh -c 'sleep 10' & pid=$!\nsleep 0.1\nkill -TERM $pid\nwait $pid");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // The shell should exit with code 128 + 15 = 143
    // But signal field should be None because the shell itself wasn't signaled
    let data = exit_event.get("data").expect("Exit event should have data");
    let code = data.get("code").and_then(|c| c.as_i64());
    let signal = data.get("signal");

    assert_eq!(code, Some(143));
    assert!(
        signal.is_none() || signal.and_then(|s| s.as_i64()).is_none(),
        "Signal should be null when subprocess is killed but shell exits normally"
    );

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_interactive_exit_command() {
    let mut session = HtSession::new(&[
        "--subscribe", "init,exit",
        "bash"
    ]);

    // Wait for init event
    session.wait_for_event("init")
        .expect("Should receive init event");

    // Send exit command
    session.send_input("exit 99\n");

    // Wait for exit event
    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(99));
    assert!(data.get("signal").is_none() || data.get("signal").and_then(|s| s.as_i64()).is_none());

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");
}

#[test]
fn test_exit_event_structure() {
    let script = create_test_script("#!/bin/sh\nexit 0");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    // Verify event structure
    assert_eq!(exit_event.get("type").and_then(|t| t.as_str()), Some("exit"));
    assert!(exit_event.get("data").is_some(), "Exit event should have data field");

    let data = exit_event.get("data").unwrap();
    assert!(data.get("code").is_some(), "Exit data should have code field");
    assert!(data.get("signal").is_some(), "Exit data should have signal field (even if null)");

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}

#[test]
fn test_multiple_commands_exit_code() {
    // Test that only the last command's exit code is reported
    let script = create_test_script("#!/bin/sh\ntrue\nfalse\nexit 13");
    let mut session = HtSession::new(&[
        "--subscribe", "exit",
        &script
    ]);

    let exit_event = session.wait_for_event("exit")
        .expect("Should receive exit event");

    let data = exit_event.get("data").expect("Exit event should have data");
    assert_eq!(data.get("code").and_then(|c| c.as_i64()), Some(13));

    session.wait_with_timeout(Duration::from_secs(2))
        .expect("Process should exit");

    let _ = fs::remove_file(script);
}
