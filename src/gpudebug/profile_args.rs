use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clap::Args;
use serde_json::{Value, json};

use super::{Error, session::Session};

#[derive(Debug, Args)]
pub struct ProfileArgs {
    /// The .gputrace to replay
    #[arg(required_unless_present = "session", conflicts_with = "session")]
    trace: Option<PathBuf>,
    /// Output directory
    #[arg(long)]
    out: PathBuf,
    /// Use an open gpudebug session instead of creating one; it is left open
    #[arg(long)]
    session: Option<String>,
    /// Skip `profile run` because the session is profiled already
    #[arg(long)]
    skip_profile: bool,
    /// Fetch compiler statistics for this many of the costliest shaders
    #[arg(long, default_value_t = 12)]
    top: usize,
    /// Counter groups to fetch [default: every group in the trace]
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,
    /// Leave the session this command creates open
    #[arg(long)]
    keep_open: bool,
    /// Seconds to wait for the replayer and the profiler
    #[arg(long, default_value_t = 1800)]
    timeout: u64,
}

impl ProfileArgs {
    pub fn run(&self) -> Result<(), Error> {
        let (session, owned) = match (&self.session, &self.trace) {
            (Some(id), _) => (Session::attach(id.clone()), false),
            (None, Some(trace)) => (Session::open(trace)?, true),
            (None, None) => return Err(Error::NoTarget),
        };
        eprintln!("session {}", session.id());
        let result = self.collect(&session);
        if owned && !self.keep_open {
            return result.and(session.terminate());
        }
        eprintln!("session {0} left open; close it with `gpudebug --terminate {0}`", session.id());
        result
    }

    fn collect(
        &self,
        session: &Session,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(self.timeout);
        session.wait_until_ready(deadline)?;
        if !self.skip_profile {
            eprintln!("profiling (serial execution, high GPU performance state)");
            session.profile(deadline)?;
        }
        let counters_directory = self.out.join("counters");
        fs::create_dir_all(&counters_directory).map_err(|source| Error::Write {
            path: counters_directory.clone(),
            source,
        })?;

        let encoders = session.list(&["performance", "timeline", "encoders"])?;
        write_json(&self.out.join("timeline_encoders.json"), &encoders)?;
        let timeline_shaders = session.list(&["performance", "timeline", "shaders"])?;
        write_json(&self.out.join("timeline_shaders.json"), &timeline_shaders)?;
        let shaders = session.list(&["performance", "shaders"])?;
        write_json(&self.out.join("shaders.json"), &shaders)?;

        let timeline_names = child_names(&timeline_shaders);
        eprintln!("fetching {} shader timelines", timeline_names.len());
        for name in &timeline_names {
            let destination = self.out.join(format!("shader__{name}.json"));
            if !destination.exists() {
                session.fetch(&["performance", "timeline", "shaders"], name, &destination)?;
            }
        }

        let mut statistics = Vec::new();
        for row in shaders["children"].as_array().into_iter().flatten().take(self.top) {
            let Some(name) = row["name"].as_str() else {
                continue;
            };
            let function = row["values"][0]["value"].clone();
            let info = session.json(&["performance", "shaders"], &format!("info {name} --all"))?;
            statistics.push(json!({ "shader": function, "statistics": info }));
        }
        write_json(&self.out.join("compiler_statistics.json"), &Value::Array(statistics))?;

        let groups = if self.groups.is_empty() {
            child_names(&session.list(&["performance", "timeline", "counters"])?)
        } else {
            self.groups.clone()
        };
        for group in &groups {
            let path = ["performance", "timeline", "counters", group.as_str()];
            let counters = child_names(&session.list(&path)?);
            eprintln!("fetching counter group {group} ({} counters)", counters.len());
            for counter in &counters {
                let destination = counters_directory.join(format!("{group}__{counter}.json"));
                if !destination.exists() {
                    session.fetch(&path, counter, &destination)?;
                }
            }
        }
        eprintln!("wrote {}", self.out.display());
        Ok(())
    }
}

fn child_names(listing: &Value) -> Vec<String> {
    listing["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["name"].as_str().map(str::to_owned))
        .collect()
}

fn write_json(
    path: &Path,
    value: &Value,
) -> Result<(), Error> {
    fs::write(path, serde_json::to_vec(value)?).map_err(|source| Error::Write {
        path: path.to_owned(),
        source,
    })
}
