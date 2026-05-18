//! Cross-platform command execution utilities
//!
//! Provides helpers for running external commands that work correctly on all platforms.
//! On Windows, this module ensures commands don't create visible terminal windows.
//!
//! Both sync (`CommandBuilder`) and async (`AsyncCommandBuilder`) versions are provided.

use std::process::{Command as StdCommand, Output, Stdio};
use tokio::process::Command as TokioCommand;
use anyhow::{Context, Result};
use tracing::debug;

/// Windows-safe command builder
/// 
/// On Windows, this ensures commands run without creating visible terminal windows.
/// This prevents PowerShell, CMD, and other terminal windows from flashing on screen.
pub struct CommandBuilder {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    pipe_stdout: bool,
    pipe_stderr: bool,
    null_stdout: bool,
    null_stderr: bool,
}

impl CommandBuilder {
    /// Create a new command builder for the given program
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            envs: Vec::new(),
            pipe_stdout: false,
            pipe_stderr: false,
            null_stdout: false,
            null_stderr: false,
        }
    }

    /// Add a single argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for arg in args {
            self.args.push(arg.into());
        }
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.envs.push((key.into(), val.into()));
        self
    }

    /// Set stdout to piped
    #[allow(dead_code)]
    pub fn stdout_piped(mut self) -> Self {
        self.pipe_stdout = true;
        self
    }

    /// Set stderr to piped
    #[allow(dead_code)]
    pub fn stderr_piped(mut self) -> Self {
        self.pipe_stderr = true;
        self
    }

    /// Set stdout to null (discard output)
    pub fn stdout_null(mut self) -> Self {
        self.null_stdout = true;
        self
    }

    /// Set stderr to null (discard errors)
    pub fn stderr_null(mut self) -> Self {
        self.null_stderr = true;
        self
    }

    /// Build the command with platform-specific settings
    fn build(&self) -> StdCommand {
        let mut cmd = StdCommand::new(&self.program);
        cmd.args(&self.args);
        
        for (key, val) in &self.envs {
            cmd.env(key, val);
        }
        
        if self.null_stdout {
            cmd.stdout(Stdio::null());
        } else if self.pipe_stdout {
            cmd.stdout(Stdio::piped());
        }
        
        if self.null_stderr {
            cmd.stderr(Stdio::null());
        } else if self.pipe_stderr {
            cmd.stderr(Stdio::piped());
        }
        
        // On Windows, set CREATE_NO_WINDOW flag to prevent terminal windows
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        cmd
    }

    /// Run the command and wait for output
    pub fn output(&self) -> Result<Output> {
        debug!("Running command: {} {:?}", self.program, self.args);
        let mut cmd = self.build();
        
        // Ensure stdout/stderr are piped for output() calls if not explicitly set
        if !self.pipe_stdout && !self.null_stdout {
            cmd.stdout(Stdio::piped());
        }
        if !self.pipe_stderr && !self.null_stderr {
            cmd.stderr(Stdio::piped());
        }
        
        cmd.output()
            .with_context(|| format!("Failed to execute command: {}", self.program))
    }

    /// Run the command and wait for exit status (ignoring output)
    pub fn status(&self) -> Result<std::process::ExitStatus> {
        debug!("Running command (status): {} {:?}", self.program, self.args);
        self.build()
            .status()
            .with_context(|| format!("Failed to execute command: {}", self.program))
    }

    /// Spawn the command as a child process
    pub fn spawn(&self) -> Result<std::process::Child> {
        debug!("Spawning command: {} {:?}", self.program, self.args);
        self.build()
            .spawn()
            .with_context(|| format!("Failed to spawn command: {}", self.program))
    }
}

/// Helper function to run a command and get output (Windows-safe)
pub fn run_command(program: &str, args: &[&str]) -> Result<Output> {
    CommandBuilder::new(program)
        .args(args.iter().map(|s| s.to_string()))
        .output()
}

/// Helper function to run a command and get exit status (Windows-safe)
pub fn run_command_status(program: &str, args: &[&str]) -> Result<std::process::ExitStatus> {
    CommandBuilder::new(program)
        .args(args.iter().map(|s| s.to_string()))
        .status()
}

/// Helper function to check if a command exists
pub fn command_exists(program: &str) -> bool {
    CommandBuilder::new(program)
        .args(["--version"])
        .stdout_null()
        .stderr_null()
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Resolve the absolute path for Homebrew's `brew` binary.
///
/// GUI apps on macOS don't inherit the shell PATH, so bare `"brew"` often
/// fails. This checks the well-known install locations first.
#[cfg(target_os = "macos")]
pub fn resolve_brew_path() -> Option<String> {
    let candidates = [
        "/opt/homebrew/bin/brew",  // Apple Silicon
        "/usr/local/bin/brew",     // Intel
        "brew",                    // fallback: on PATH
    ];
    for candidate in candidates {
        if CommandBuilder::new(candidate)
            .args(["--version"])
            .stdout_null()
            .stderr_null()
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    None
}

// =============================================================================
// Async Command Builder for tokio::process::Command
// =============================================================================

/// Windows-safe async command builder
/// 
/// On Windows, this ensures async commands run without creating visible terminal windows.
/// This is the async equivalent of `CommandBuilder` for use with tokio.
pub struct AsyncCommandBuilder {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    pipe_stdout: bool,
    pipe_stderr: bool,
    pipe_stdin: bool,
    null_stdout: bool,
    null_stderr: bool,
}

impl AsyncCommandBuilder {
    /// Create a new async command builder for the given program
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            envs: Vec::new(),
            pipe_stdout: false,
            pipe_stderr: false,
            pipe_stdin: false,
            null_stdout: false,
            null_stderr: false,
        }
    }

    /// Add a single argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for arg in args {
            self.args.push(arg.into());
        }
        self
    }

    /// Add an environment variable
    #[allow(dead_code)]
    pub fn env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.envs.push((key.into(), val.into()));
        self
    }

    /// Set stdout to piped
    pub fn stdout_piped(mut self) -> Self {
        self.pipe_stdout = true;
        self
    }

    /// Set stderr to piped
    pub fn stderr_piped(mut self) -> Self {
        self.pipe_stderr = true;
        self
    }
    
    /// Set stdin to piped
    #[allow(dead_code)]
    pub fn stdin_piped(mut self) -> Self {
        self.pipe_stdin = true;
        self
    }

    /// Set stdout to null (discard output)
    pub fn stdout_null(mut self) -> Self {
        self.null_stdout = true;
        self
    }

    /// Set stderr to null (discard errors)
    pub fn stderr_null(mut self) -> Self {
        self.null_stderr = true;
        self
    }

    /// Build the async command with platform-specific settings
    fn build(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new(&self.program);
        cmd.args(&self.args);
        
        for (key, val) in &self.envs {
            cmd.env(key, val);
        }
        
        if self.null_stdout {
            cmd.stdout(Stdio::null());
        } else if self.pipe_stdout {
            cmd.stdout(Stdio::piped());
        }
        
        if self.null_stderr {
            cmd.stderr(Stdio::null());
        } else if self.pipe_stderr {
            cmd.stderr(Stdio::piped());
        }
        
        if self.pipe_stdin {
            cmd.stdin(Stdio::piped());
        }
        
        // On Windows, set CREATE_NO_WINDOW flag to prevent terminal windows
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        cmd
    }

    /// Run the command and wait for output
    pub async fn output(&self) -> Result<Output> {
        debug!("Running async command: {} {:?}", self.program, self.args);
        let mut cmd = self.build();
        
        // Ensure stdout/stderr are piped for output() calls if not explicitly set
        if !self.pipe_stdout && !self.null_stdout {
            cmd.stdout(Stdio::piped());
        }
        if !self.pipe_stderr && !self.null_stderr {
            cmd.stderr(Stdio::piped());
        }
        
        cmd.output()
            .await
            .with_context(|| format!("Failed to execute async command: {}", self.program))
    }

    /// Run the command and wait for exit status (ignoring output)
    pub async fn status(&self) -> Result<std::process::ExitStatus> {
        debug!("Running async command (status): {} {:?}", self.program, self.args);
        self.build()
            .status()
            .await
            .with_context(|| format!("Failed to execute async command: {}", self.program))
    }

    /// Spawn the command as a child process
    pub fn spawn(&mut self) -> Result<tokio::process::Child> {
        debug!("Spawning async command: {} {:?}", self.program, self.args);
        self.build()
            .spawn()
            .with_context(|| format!("Failed to spawn async command: {}", self.program))
    }
    
    /// Kill a process by PID (Windows-safe)
    #[cfg(target_os = "windows")]
    pub fn kill_process(pid: u32) -> Result<Output> {
        CommandBuilder::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
    }
    
    /// Kill a process by PID (Unix)
    #[cfg(unix)]
    pub fn kill_process(pid: u32) -> Result<Output> {
        CommandBuilder::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_builder() {
        let output = CommandBuilder::new("echo")
            .arg("hello")
            .output();
        
        // This test may fail on Windows where echo is different
        #[cfg(unix)]
        {
            let output = output.unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("hello"));
        }
    }

    #[test]
    fn test_run_command() {
        let result = run_command("echo", &["test"]);
        #[cfg(unix)]
        assert!(result.is_ok());
    }
}
