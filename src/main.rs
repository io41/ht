mod api;
mod cli;
mod command;
mod locale;
mod nbio;
mod pty;
mod session;
use anyhow::{Context, Result};
use command::Command;
use session::Session;
use std::net::{SocketAddr, TcpListener};
use tokio::{sync::mpsc, task::JoinHandle};

#[tokio::main]
async fn main() -> Result<()> {
    locale::check_utf8_locale()?;
    let cli = cli::Cli::new();

    let (input_tx, input_rx) = mpsc::channel(1024);
    let (output_tx, output_rx) = mpsc::channel(1024);
    let (command_tx, command_rx) = mpsc::channel(1024);
    let (clients_tx, clients_rx) = mpsc::channel(1);

    start_http_api(cli.listen, clients_tx.clone()).await?;
    let api = start_stdio_api(command_tx, clients_tx, cli.subscribe.unwrap_or_default());
    let (pid, pty) = start_pty(cli.command, &cli.size, input_rx, output_tx)?;
    let session = build_session(&cli.size, pid);
    run_event_loop(output_rx, input_tx, command_rx, clients_rx, session, api, pty).await
}

fn build_session(size: &cli::Size, pid: i32) -> Session {
    Session::new(size.cols(), size.rows(), pid)
}

fn start_stdio_api(
    command_tx: mpsc::Sender<Command>,
    clients_tx: mpsc::Sender<session::Client>,
    sub: api::Subscription,
) -> JoinHandle<Result<()>> {
    tokio::spawn(api::stdio::start(command_tx, clients_tx, sub))
}

fn start_pty(
    command: Vec<String>,
    size: &cli::Size,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
) -> Result<(i32, JoinHandle<Result<pty::ExitStatus>>)> {
    let command = command.join(" ");
    eprintln!("launching \"{}\" in terminal of size {}", command, size);
    let (pid, fut) = pty::spawn(command, size, input_rx, output_tx)?;

    Ok((pid, tokio::spawn(fut)))
}

async fn start_http_api(
    listen_addr: Option<SocketAddr>,
    clients_tx: mpsc::Sender<session::Client>,
) -> Result<()> {
    if let Some(addr) = listen_addr {
        let listener = TcpListener::bind(addr).context("cannot start HTTP listener")?;
        tokio::spawn(api::http::start(listener, clients_tx).await?);
    }

    Ok(())
}

fn validate_mouse_coordinates(mouse_event: &command::MouseEvent, session: &Session) {
    let (cols, rows) = session.size();
    if mouse_event.row > rows || mouse_event.col > cols {
        eprintln!(
            "warning: mouse coordinates ({},{}) exceed terminal size ({}x{})",
            mouse_event.col, mouse_event.row, cols, rows
        );
    }
}

async fn run_event_loop(
    mut output_rx: mpsc::Receiver<Vec<u8>>,
    input_tx: mpsc::Sender<Vec<u8>>,
    mut command_rx: mpsc::Receiver<Command>,
    mut clients_rx: mpsc::Receiver<session::Client>,
    mut session: Session,
    mut api_handle: JoinHandle<Result<()>>,
    mut pty_handle: JoinHandle<Result<pty::ExitStatus>>,
) -> Result<()> {
    let mut serving = true;
    let mut stdin_open = true;
    let mut api_running = true;
    let mut output_open = true;

    loop {
        tokio::select! {
            result = &mut pty_handle => {
                match result {
                    Ok(Ok(exit_status)) => {
                        eprintln!("process exited with code {}, shutting down...", exit_status.code);
                        session.exit(exit_status.code, exit_status.signal);
                    },
                    Ok(Err(e)) => {
                        eprintln!("pty error: {e}, shutting down...");
                        session.exit(1, None);
                    },
                    Err(e) => {
                        eprintln!("pty task error: {e}, shutting down...");
                        session.exit(1, None);
                    }
                }
                break;
            }

            result = output_rx.recv(), if output_open => {
                match result {
                    Some(data) => {
                        session.output(String::from_utf8_lossy(&data).to_string());
                    },

                    None => {
                        eprintln!("output channel closed, waiting for process to exit...");
                        output_open = false;
                    }
                }
            }

            command = command_rx.recv(), if stdin_open => {
                match command {
                    Some(Command::Input(seqs)) => {
                        let data = command::seqs_to_bytes(&seqs, session.cursor_key_app_mode());
                        input_tx.send(data).await?;
                    }

                    Some(Command::Mouse(mouse_event)) => {
                        validate_mouse_coordinates(&mouse_event, &session);
                        let data = command::mouse_to_bytes(&mouse_event);
                        input_tx.send(data).await?;
                    }

                    Some(Command::MouseClick(mouse_event)) => {
                        validate_mouse_coordinates(&mouse_event, &session);

                        // Send press event
                        let mut press_event = mouse_event.clone();
                        press_event.event_type = command::MouseEventType::Press;
                        let press_data = command::mouse_to_bytes(&press_event);
                        input_tx.send(press_data).await?;

                        // Send release event
                        let mut release_event = mouse_event;
                        release_event.event_type = command::MouseEventType::Release;
                        let release_data = command::mouse_to_bytes(&release_event);
                        input_tx.send(release_data).await?;
                    }

                    Some(Command::Snapshot) => {
                        session.snapshot();
                    }

                    Some(Command::Resize(cols, rows)) => {
                        session.resize(cols, rows);
                    }

                    None => {
                        eprintln!("stdin closed, continuing to wait for process to exit...");
                        stdin_open = false;
                    }
                }
            }

            client = clients_rx.recv(), if serving => {
                match client {
                    Some(client) => {
                        client.accept(session.subscribe());
                    }

                    None => {
                        serving = false;
                    }
                }
            }

            _ = &mut api_handle, if api_running => {
                eprintln!("api task exited, continuing to wait for process to exit...");
                api_running = false;
            }
        }
    }

    Ok(())
}
