use std::{
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use clap::Args;

use super::Error;

const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Path of the .gputrace to write
    #[arg(long)]
    out: PathBuf,
    /// Capture the process whose `gpucapture list` row contains this text instead of the launched
    /// command, for commands that are wrapper scripts
    #[arg(long)]
    process: Option<String>,
    /// Seconds to wait for the target to become capturable
    #[arg(long, default_value_t = 120)]
    timeout: u64,
    /// The command to launch, after `--`
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

impl CaptureArgs {
    pub fn run(&self) -> Result<(), Error> {
        if self.out.exists() {
            return Err(Error::OutputExists(self.out.clone()));
        }
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .env("MTL_CAPTURE_ENABLED", "1")
            .env("MTLCAPTURE_WAIT_FOR_SIGNAL", "1")
            .spawn()
            .map_err(|source| Error::Launch {
                program: self.command[0].clone(),
                source,
            })?;
        let captured = self.capture(&mut child);
        if captured.is_err() {
            child.kill().ok();
        }
        let status = child.wait().map_err(Error::Wait)?;
        captured?;
        if !status.success() {
            return Err(Error::TargetFailed {
                path: self.out.clone(),
                status,
            });
        }
        eprintln!("wrote {}", self.out.display());
        Ok(())
    }

    fn capture(
        &self,
        child: &mut Child,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(self.timeout);
        let launched = child.id().to_string();
        let pid = loop {
            if let Some(status) = child.try_wait().map_err(Error::Wait)? {
                return Err(Error::ExitedEarly(status));
            }
            let listing = Command::new("gpucapture").arg("list").output().map_err(Error::Gpucapture)?;
            let found = String::from_utf8_lossy(&listing.stdout).lines().find_map(|line| {
                let pid = line.split_whitespace().next()?;
                let matches = match &self.process {
                    Some(process) => line.contains(process.as_str()),
                    None => pid == launched,
                };
                (matches && pid.bytes().all(|byte| byte.is_ascii_digit())).then(|| pid.to_owned())
            });
            if let Some(pid) = found {
                break pid;
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout(self.timeout));
            }
            thread::sleep(POLL_INTERVAL);
        };
        eprintln!("capturing process {pid} until it exits");
        let status = Command::new("gpucapture")
            .args(["start", "--pid", &pid, "--until-exit", "--output"])
            .arg(&self.out)
            .status()
            .map_err(Error::Gpucapture)?;
        if !status.success() {
            return Err(Error::CaptureFailed(status));
        }
        Ok(())
    }
}
