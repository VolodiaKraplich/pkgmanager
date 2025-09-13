//! Command-line interface module
//!
//! Provides argument parsing and command execution.

pub mod args;
pub mod commands;

pub use args::{parse_args, Args, Command};
pub use commands::execute_command;