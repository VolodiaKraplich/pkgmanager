//! Process execution utilities
//!
//! Provides safe process execution with proper error handling and logging.

use crate::error::{BuilderError, Result};
use std::process::{Command, Stdio};
use tracing::{debug, info, instrument, warn};

/// Utility for running external processes
#[derive(Debug)]
pub struct ProcessRunner {
    debug: bool,
}

/// Result of a process execution
#[derive(Debug)]
pub struct ProcessResult {
    /// Exit status code
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether the process was successful
    pub success: bool,
}

impl ProcessRunner {
    /// Create a new process runner
    #[must_use]
    pub const fn new(debug: bool) -> Self {
        Self { debug }
    }

    /// Run a command with arguments, inheriting stdout/stderr
    #[instrument(skip(self))]
    pub fn run_command(&self, command: &str, args: &[&str]) -> Result<()> {
        self.run_command_with_env(command, args, &[])
    }

    /// Run a command with arguments and environment variables
    #[instrument(skip(self, env_vars))]
    pub fn run_command_with_env(
        &self,
        command: &str,
        args: &[&str],
        env_vars: &[(String, String)],
    ) -> Result<()> {
        let cmd_str = format!("{} {}", command, args.join(" "));

        if self.debug {
            debug!("Running command: {}", cmd_str);
            if !env_vars.is_empty() {
                debug!("Environment variables: {:?}", env_vars);
            }
        } else {
            info!("+ {}", cmd_str);
        }

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Add environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let status = cmd.status().map_err(|e| {
            BuilderError::process(
                cmd_str.clone(),
                None,
                String::new(),
                format!("Failed to execute command: {e}"),
            )
        })?;

        if !status.success() {
            let exit_code = status.code();
            return Err(BuilderError::process(
                cmd_str,
                exit_code,
                String::new(),
                format!("Command failed with exit code: {exit_code:?}"),
            ));
        }

        debug!("Command completed successfully");
        Ok(())
    }

    /// Run a command and capture its output
    #[instrument(skip(self))]
    pub fn run_command_with_output(&self, command: &str, args: &[&str]) -> Result<ProcessResult> {
        self.run_command_with_output_and_env(command, args, &[])
    }

    /// Run a command with environment variables and capture output
    #[instrument(skip(self, env_vars))]
    pub fn run_command_with_output_and_env(
        &self,
        command: &str,
        args: &[&str],
        env_vars: &[(String, String)],
    ) -> Result<ProcessResult> {
        let cmd_str = format!("{} {}", command, args.join(" "));

        debug!("Running command with output capture: {}", cmd_str);

        let mut cmd = Command::new(command);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        // Add environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let output = cmd.output().map_err(|e| {
            BuilderError::process(
                cmd_str.clone(),
                None,
                String::new(),
                format!("Failed to execute command: {e}"),
            )
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let exit_code = output.status.code();

        debug!(
            "Command finished: success={}, exit_code={:?}, stdout_len={}, stderr_len={}",
            success,
            exit_code,
            stdout.len(),
            stderr.len()
        );

        if !success {
            debug!("Command stderr: {}", stderr);
            return Err(BuilderError::process(cmd_str, exit_code, stdout, stderr));
        }

        Ok(ProcessResult {
            exit_code,
            stdout,
            stderr,
            success,
        })
    }

    /// Check if a command exists in PATH
    #[instrument(skip(self))]
    pub fn command_exists(&self, command: &str) -> bool {
        debug!("Checking if command exists: {}", command);

        let result = Command::new("which")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match result {
            Ok(status) => {
                let exists = status.success();
                debug!("Command '{}' exists: {}", command, exists);
                exists
            }
            Err(e) => {
                debug!("Failed to check if command '{}' exists: {}", command, e);
                false
            }
        }
    }

    /// Run multiple commands in sequence
    #[instrument(skip(self, commands))]
    pub fn run_commands_sequence(
        &self,
        commands: &[(&str, &[&str])],
    ) -> Result<Vec<ProcessResult>> {
        let mut results = Vec::new();

        for (i, (command, args)) in commands.iter().enumerate() {
            debug!(
                "Running command {} of {}: {}",
                i + 1,
                commands.len(),
                command
            );

            match self.run_command_with_output(command, args) {
                Ok(result) => {
                    debug!("Command {} completed successfully", i + 1);
                    results.push(result);
                }
                Err(e) => {
                    warn!("Command {} failed: {}", i + 1, e);
                    return Err(e);
                }
            }
        }

        info!("All {} commands completed successfully", commands.len());
        Ok(results)
    }

    /// Kill a process by PID (Unix only)
    #[cfg(unix)]
    #[instrument(skip(self))]
    pub fn kill_process(&self, pid: u32, signal: i32) -> Result<()> {
        debug!("Killing process {} with signal {}", pid, signal);

        let result = self.run_command("kill", &[&format!("-{signal}"), &pid.to_string()]);

        match result {
            Ok(()) => {
                debug!("Process {} killed successfully", pid);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to kill process {}: {}", pid, e);
                Err(e)
            }
        }
    }

    /// Get process information by name (Unix only)
    #[cfg(unix)]
    #[instrument(skip(self))]
    pub fn get_processes_by_name(&self, name: &str) -> Result<Vec<u32>> {
        debug!("Getting processes by name: {}", name);

        let result = self.run_command_with_output("pgrep", &[name])?;

        let pids: Vec<u32> = result
            .stdout
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();

        debug!(
            "Found {} processes named '{}': {:?}",
            pids.len(),
            name,
            pids
        );
        Ok(pids)
    }
}

impl Default for ProcessRunner {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_runner_creation() {
        let runner = ProcessRunner::new(true);
        assert!(runner.debug);

        let runner = ProcessRunner::default();
        assert!(!runner.debug);
    }

    #[test]
    fn test_run_simple_command() {
        let runner = ProcessRunner::new(false);
        let result = runner.run_command("echo", &["hello"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_command_with_output() {
        let runner = ProcessRunner::new(false);
        let result = runner
            .run_command_with_output("echo", &["hello", "world"])
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout.trim(), "hello world");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_command_exists() {
        let runner = ProcessRunner::new(false);

        // These commands should exist on most Unix systems
        assert!(runner.command_exists("echo"));
        assert!(runner.command_exists("ls"));

        // This command should not exist
        assert!(!runner.command_exists("nonexistent_command_12345"));
    }

    #[test]
    fn test_run_failing_command() {
        let runner = ProcessRunner::new(false);
        let result = runner.run_command("false", &[]);
        assert!(result.is_err());

        if let Err(BuilderError::Process {
            command, exit_code, ..
        }) = result
        {
            assert_eq!(command, "false ");
            assert_eq!(exit_code, Some(1));
        } else {
            panic!("Expected ProcessError");
        }
    }

    #[test]
    fn test_run_command_with_env() {
        let runner = ProcessRunner::new(false);
        let env_vars = vec![("TEST_VAR".to_string(), "test_value".to_string())];

        let result = runner
            .run_command_with_output_and_env("sh", &["-c", "echo $TEST_VAR"], &env_vars)
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout.trim(), "test_value");
    }

    #[test]
    fn test_run_commands_sequence() {
        let runner = ProcessRunner::new(false);
        let commands = vec![("echo", &["first"][..]), ("echo", &["second"][..])];

        let results = runner.run_commands_sequence(&commands).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].stdout.trim(), "first");
        assert_eq!(results[1].stdout.trim(), "second");
    }

    #[test]
    fn test_run_commands_sequence_failure() {
        let runner = ProcessRunner::new(false);
        let commands = vec![
            ("echo", &["first"][..]),
            ("false", &[][..]), // This will fail
            ("echo", &["third"][..]),
        ];

        let result = runner.run_commands_sequence(&commands);
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_get_processes_by_name() {
        let runner = ProcessRunner::new(false);

        // Try to get processes for a common process (this might be empty, that's ok)
        let result = runner.get_processes_by_name("init");
        assert!(result.is_ok());

        // Try with a process name that definitely doesn't exist
        let result = runner.get_processes_by_name("nonexistent_process_12345");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
