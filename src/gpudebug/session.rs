use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

use regex::Regex;
use serde_json::Value;

use super::Error;

const POLL_INTERVAL: Duration = Duration::from_secs(3);

pub struct Session {
    id: String,
}

impl Session {
    pub fn open(trace: &Path) -> Result<Self, Error> {
        let output = gpudebug([
            OsStr::new("-q"),
            "--timeout".as_ref(),
            "0".as_ref(),
            "-t".as_ref(),
            trace.as_os_str(),
            "-c".as_ref(),
            "list".as_ref(),
        ])?;
        let pattern = Regex::new(r"Session (\d+)").expect("the session pattern is valid");
        let id = pattern.captures(&output).ok_or_else(|| Error::NoSession(output.clone()))?[1].to_owned();
        Ok(Self {
            id,
        })
    }

    pub fn attach(id: String) -> Self {
        Self {
            id,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn wait_until_ready(
        &self,
        deadline: Instant,
    ) -> Result<(), Error> {
        loop {
            let listing = gpudebug(["-l"])?;
            let row = listing.lines().find_map(|line| {
                let mut words = line.split_whitespace();
                (words.next() == Some(self.id.as_str())).then(|| words.collect::<Vec<_>>())
            });
            match row {
                None => return Err(Error::SessionMissing(self.id.clone())),
                Some(words) if words.contains(&"ready") => return Ok(()),
                Some(words) if words.contains(&"error") => return Err(Error::SessionFailed(self.id.clone())),
                Some(_) => pause(deadline, "the trace to load")?,
            }
        }
    }

    pub fn profile(
        &self,
        deadline: Instant,
    ) -> Result<(), Error> {
        let output = self.run(&[], "profile run --gpu-state high --exec serial")?;
        if !output.contains("Profile data collected") {
            return Err(Error::ProfileFailed(output));
        }
        loop {
            let ready = self.list(&["performance", "commands"]).is_ok_and(|listing| {
                listing["children"].as_array().is_some_and(|rows| rows.iter().any(|row| row["name"] == "disp0"))
            });
            if ready {
                return Ok(());
            }
            pause(deadline, "the performance tree")?;
        }
    }

    pub fn list(
        &self,
        path: &[&str],
    ) -> Result<Value, Error> {
        self.json(path, "list --all")
    }

    pub fn json(
        &self,
        path: &[&str],
        command: &str,
    ) -> Result<Value, Error> {
        let output = self.execute(path, command, true)?;
        let lines: Vec<&str> = output.lines().collect();
        let starts: Vec<usize> = (0..lines.len()).filter(|&index| lines[index].starts_with('{')).collect();
        let expected = path.len() + 2;
        match starts.last() {
            Some(&start) if starts.len() == expected => Ok(serde_json::from_str(&lines[start..].join("\n"))?),
            _ => Err(Error::UnexpectedJson {
                command: command.to_owned(),
                expected,
                found: starts.len(),
                output,
            }),
        }
    }

    pub fn fetch(
        &self,
        path: &[&str],
        name: &str,
        destination: &Path,
    ) -> Result<(), Error> {
        let staging = std::env::temp_dir().join(format!("metal-perf-{}-{name}.json", process::id()));
        if staging.to_string_lossy().contains(char::is_whitespace) {
            return Err(Error::PathWithSpace(staging));
        }
        self.run(path, &format!("fetch {name} --out {}", staging.display()))?;
        let moved = fs::copy(&staging, destination).and_then(|_| fs::remove_file(&staging));
        moved.map_err(|source| Error::Write {
            path: destination.to_owned(),
            source,
        })
    }

    pub fn run(
        &self,
        path: &[&str],
        command: &str,
    ) -> Result<String, Error> {
        self.execute(path, command, false)
    }

    pub fn terminate(&self) -> Result<(), Error> {
        gpudebug(["--terminate", self.id.as_str()]).map(drop)
    }

    fn execute(
        &self,
        path: &[&str],
        command: &str,
        json: bool,
    ) -> Result<String, Error> {
        let mut commands = vec!["go /".to_owned()];
        commands.extend(path.iter().map(|node| format!("go {node}")));
        commands.push(command.to_owned());
        let mut arguments = vec!["-s".to_owned(), self.id.clone(), "-q".to_owned()];
        if json {
            arguments.push("--json".to_owned());
        }
        for command in commands {
            arguments.push("-c".to_owned());
            arguments.push(command);
        }
        gpudebug(&arguments)
    }
}

fn gpudebug(arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<String, Error> {
    let arguments: Vec<_> = arguments.into_iter().map(|argument| argument.as_ref().to_owned()).collect();
    let output = Command::new("gpudebug").args(&arguments).output().map_err(Error::Spawn)?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let errors: Vec<&str> = text.lines().filter(|line| line.starts_with("error:")).collect();
    if !output.status.success() || !errors.is_empty() {
        let joined = arguments.iter().map(|argument| argument.to_string_lossy()).collect::<Vec<_>>().join(" ");
        return Err(Error::Failed {
            arguments: joined,
            output: if errors.is_empty() {
                text.clone()
            } else {
                errors.join("; ")
            },
        });
    }
    Ok(text)
}

fn pause(
    deadline: Instant,
    what: &'static str,
) -> Result<(), Error> {
    if Instant::now() >= deadline {
        return Err(Error::Timeout(what));
    }
    thread::sleep(POLL_INTERVAL);
    Ok(())
}
