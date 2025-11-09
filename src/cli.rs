use crate::api::Subscription;
use anyhow::bail;
use clap::Parser;
use nix::pty;
use std::{fmt::Display, net::SocketAddr, ops::Deref, str::FromStr};

#[derive(Debug, Parser)]
#[clap(version, about)]
#[command(
    name = "ht",
    after_help = "EXAMPLES:
    # Start bash in a PTY
    ht

    # Start fish shell
    ht fish

    # Run vim with a file
    ht vim README.md

    # Run a command with arguments (use -- to separate)
    ht -- bash --norc

    # Subscribe to exit events only
    ht --subscribe exit

    # Run with custom terminal size
    ht --size 80x24 -- vim

    # Enable HTTP server for live preview
    ht -l

For more information, see: https://github.com/andyk/ht"
)]
pub struct Cli {
    /// Terminal size (columns x rows)
    #[arg(long, value_name = "COLSxROWS", default_value = Some("120x40"))]
    pub size: Size,

    /// Command to run inside the terminal. Use -- to separate ht options from command arguments.
    #[arg(default_value = "bash", trailing_var_arg = true)]
    pub command: Vec<String>,

    /// Enable HTTP server for WebSocket API and live terminal preview
    #[arg(short, long, value_name = "LISTEN_ADDR", default_missing_value = "127.0.0.1:0", num_args = 0..=1)]
    pub listen: Option<SocketAddr>,

    /// Subscribe to specific events (comma-separated: init,output,resize,snapshot,exit)
    #[arg(long, value_name = "EVENTS")]
    pub subscribe: Option<Subscription>,
}

impl Cli {
    pub fn new() -> Self {
        Cli::parse()
    }
}

#[derive(Debug, Clone)]
pub struct Size(pty::Winsize);

impl Size {
    pub fn cols(&self) -> usize {
        self.0.ws_col as usize
    }

    pub fn rows(&self) -> usize {
        self.0.ws_row as usize
    }
}

impl FromStr for Size {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        match s.split_once('x') {
            Some((cols, rows)) => {
                let cols: u16 = cols.parse()
                    .map_err(|_| anyhow::anyhow!(
                        "invalid columns value '{}' in size '{}'\n  \
                         tip: use format COLSxROWS, e.g., --size 80x24",
                        cols, s
                    ))?;
                let rows: u16 = rows.parse()
                    .map_err(|_| anyhow::anyhow!(
                        "invalid rows value '{}' in size '{}'\n  \
                         tip: use format COLSxROWS, e.g., --size 80x24",
                        rows, s
                    ))?;

                let winsize = pty::Winsize {
                    ws_col: cols,
                    ws_row: rows,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                Ok(Size(winsize))
            }

            None => {
                bail!(
                    "invalid size format: '{}'\n  \
                     tip: use format COLSxROWS, e.g., --size 80x24",
                    s
                );
            }
        }
    }
}

impl Deref for Size {
    type Target = pty::Winsize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.0.ws_col, self.0.ws_row)
    }
}
