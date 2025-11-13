use crate::nbio;
use anyhow::Result;
use nix::libc;
use nix::pty;
use nix::sys::signal::{self, SigHandler, Signal};
use nix::sys::wait;
use nix::unistd::{self, ForkResult, Pid};
use std::env;
use std::ffi::{CString, NulError};
use std::fs::File;
use std::future::Future;
use std::io;
use std::os::fd::FromRawFd;
use std::os::fd::{AsRawFd, OwnedFd};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

pub struct ExitStatus {
    pub code: i32,
    pub signal: Option<i32>,
}

pub fn spawn(
    command: String,
    winsize: &pty::Winsize,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<(i32, impl Future<Output = Result<ExitStatus>>)> {
    let result = unsafe { pty::forkpty(Some(winsize), None) }?;

    match result.fork_result {
        ForkResult::Parent { child } => Ok((
            child.as_raw(),
            drive_child(child, result.master, input_rx, output_tx),
        )),

        ForkResult::Child => {
            exec(command)?;
            unreachable!();
        }
    }
}

async fn drive_child(
    child: Pid,
    master: OwnedFd,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<ExitStatus> {
    let result = do_drive_child(master, input_rx, output_tx).await;

    // Try non-blocking wait first to check if process has already exited
    let wait_status = tokio::task::spawn_blocking(move || {
        match wait::waitpid(child, Some(wait::WaitPidFlag::WNOHANG))? {
            wait::WaitStatus::StillAlive => {
                // Process is still running, send SIGHUP to terminate it
                eprintln!("process still running, sending HUP signal");
                unsafe { libc::kill(child.as_raw(), libc::SIGHUP) };
                // Now wait for it to exit
                wait::waitpid(child, None)
            }
            status => {
                // Process has already exited, return the status
                eprintln!("process has already exited");
                Ok(status)
            }
        }
    })
    .await
    .unwrap()?;

    result?;

    Ok(parse_exit_status(wait_status))
}

fn parse_exit_status(status: wait::WaitStatus) -> ExitStatus {
    use wait::WaitStatus::*;

    match status {
        Exited(_pid, code) => ExitStatus {
            code,
            signal: None,
        },
        Signaled(_pid, signal, _core_dump) => ExitStatus {
            code: 128 + signal as i32,
            signal: Some(signal as i32),
        },
        _ => ExitStatus {
            code: 1,
            signal: None,
        },
    }
}

const READ_BUF_SIZE: usize = 128 * 1024;

async fn do_drive_child(
    master: OwnedFd,
    mut input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<()> {
    let mut buf = [0u8; READ_BUF_SIZE];
    let mut input: Vec<u8> = Vec::with_capacity(READ_BUF_SIZE);
    nbio::set_non_blocking(&master.as_raw_fd())?;
    let master_file = unsafe { File::from_raw_fd(master.as_raw_fd()) };
    let master_fd = AsyncFd::new(master)?;

    // Prevent master_file from closing the fd since AsyncFd owns it
    let mut master_file = std::mem::ManuallyDrop::new(master_file);

    loop {
        tokio::select! {
            result = input_rx.recv() => {
                match result {
                    Some(data) => {
                        input.extend_from_slice(&data);
                    }

                    None => {
                        return Ok(());
                    }
                }
            }

            result = master_fd.readable() => {
                let mut guard = result?;

                loop {
                    match nbio::read(&mut *master_file, &mut buf)? {
                        Some(0) => {
                            return Ok(());
                        }

                        Some(n) => {
                            output_tx.send(buf[0..n].to_vec()).await?;
                        }

                        None => {
                            guard.clear_ready();
                            break;
                        }
                    }
                }
            }

            result = master_fd.writable(), if !input.is_empty() => {
                let mut guard = result?;
                let mut buf: &[u8] = input.as_ref();

                loop {
                    match nbio::write(&mut *master_file, buf)? {
                        Some(0) => {
                            return Ok(());
                        }

                        Some(n) => {
                            buf = &buf[n..];

                            if buf.is_empty() {
                                break;
                            }
                        }

                        None => {
                            guard.clear_ready();
                            break;
                        }
                    }
                }

                let left = buf.len();

                if left == 0 {
                    input.clear();
                } else {
                    input.drain(..input.len() - left);
                }
            }
        }
    }
}

fn exec(command: String) -> io::Result<()> {
    let command = ["/bin/sh".to_owned(), "-c".to_owned(), command]
        .iter()
        .map(|s| CString::new(s.as_bytes()))
        .collect::<Result<Vec<CString>, NulError>>()?;

    env::set_var("TERM", "xterm-256color");
    unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigDfl) }?;
    unistd::execvp(&command[0], &command)?;
    unsafe { libc::_exit(1) }
}
