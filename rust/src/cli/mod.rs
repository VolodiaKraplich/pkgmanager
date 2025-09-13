//! Command-line interface module
//!
//! Provides argument parsing and command execution.

pub mod args;
pub mod commands;

pub use args::{Args, Command, parse_args};
pub use commands::execute_command;
