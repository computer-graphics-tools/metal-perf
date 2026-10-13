use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{
    Error, counter_series::CounterSeries, dispatch::Dispatch, encoder::Encoder, shader_timeline::ShaderTimeline,
};

pub struct ProfileDirectory {
    path: PathBuf,
    encoders: Vec<Encoder>,
    segments: Vec<Vec<Dispatch>>,
    compiler_statistics: HashMap<String, Value>,
}

impl ProfileDirectory {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let encoders_path = path.join("timeline_encoders.json");
        let listing: Value = read_json(&encoders_path)?;
        let encoders = listing["children"]
            .as_array()
            .into_iter()
            .flatten()
            .map(Encoder::from_row)
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::Layout {
                path: encoders_path,
                reason: "an encoder row lacks a name, begin or end",
            })?;

        let mut dispatches = Vec::new();
        for entry in list_directory(path)? {
            let name = entry.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with("shader__") && name.ends_with(".json") {
                let timeline: ShaderTimeline = read_json(&entry)?;
                let shader: Rc<str> = timeline.name.as_str().into();
                dispatches.extend(timeline.merged_intervals().into_iter().map(|interval| Dispatch {
                    begin_ns: interval.begin_ns,
                    end_ns: interval.end_ns,
                    shader: shader.clone(),
                }));
            }
        }
        if dispatches.is_empty() {
            return Err(Error::NoShaders(path.to_owned()));
        }
        dispatches.sort_by_key(|dispatch| dispatch.begin_ns);

        let statistics_path = path.join("compiler_statistics.json");
        let mut compiler_statistics = HashMap::new();
        if statistics_path.exists() {
            let rows: Value = read_json(&statistics_path)?;
            for row in rows.as_array().into_iter().flatten() {
                if let Some(shader) = row["shader"].as_str() {
                    compiler_statistics.entry(shader.to_owned()).or_insert_with(|| row["statistics"].clone());
                }
            }
        }

        Ok(Self {
            path: path.to_owned(),
            segments: split_at_largest_gaps(dispatches, encoders.len()),
            encoders,
            compiler_statistics,
        })
    }

    pub fn segments(&self) -> &[Vec<Dispatch>] {
        &self.segments
    }

    pub fn encoder(
        &self,
        index: usize,
    ) -> Option<&Encoder> {
        self.encoders.get(index)
    }

    pub fn encoder_count(&self) -> usize {
        self.encoders.len()
    }

    pub fn compiler_statistics(
        &self,
        shader: &str,
    ) -> Option<&Value> {
        self.compiler_statistics.get(shader)
    }

    pub fn counters(&self) -> Result<Vec<(String, CounterSeries)>, Error> {
        let directory = self.path.join("counters");
        let mut counters = Vec::new();
        for path in list_directory(&directory)? {
            let Some(stem) = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()) else {
                continue;
            };
            if path.extension().is_some_and(|extension| extension == "json") {
                let mut series: CounterSeries = read_json(&path)?;
                series.samples.sort_by(|left, right| left.t.total_cmp(&right.t));
                counters.push((stem.replacen("__", "/", 1), series));
            }
        }
        counters.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(counters)
    }
}

fn split_at_largest_gaps(
    dispatches: Vec<Dispatch>,
    count: usize,
) -> Vec<Vec<Dispatch>> {
    if count <= 1 || dispatches.len() < 2 {
        return vec![dispatches];
    }
    let mut gaps: Vec<(i64, usize)> =
        dispatches.windows(2).enumerate().map(|(index, pair)| (pair[1].begin_ns - pair[0].end_ns, index)).collect();
    gaps.sort_by(|left, right| right.cmp(left));
    let mut cuts: Vec<usize> = gaps.into_iter().take(count - 1).map(|(_, index)| index + 1).collect();
    cuts.sort_unstable();
    let mut segments = Vec::with_capacity(count);
    let mut rest = dispatches;
    for cut in cuts.into_iter().rev() {
        segments.push(rest.split_off(cut));
    }
    segments.push(rest);
    segments.reverse();
    segments
}

fn list_directory(path: &Path) -> Result<Vec<PathBuf>, Error> {
    let read = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    fs::read_dir(path).map_err(read)?.map(|entry| entry.map(|entry| entry.path()).map_err(read)).collect()
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Parse {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{Dispatch, split_at_largest_gaps};

    #[test]
    fn splits_the_timeline_at_the_largest_gaps() {
        let shader: Rc<str> = "kernel".into();
        let dispatches = [(0, 10), (12, 20), (500, 510), (511, 520), (9000, 9010)]
            .map(|(begin_ns, end_ns)| Dispatch {
                begin_ns,
                end_ns,
                shader: shader.clone(),
            })
            .to_vec();
        let segments = split_at_largest_gaps(dispatches, 3);
        let lengths: Vec<usize> = segments.iter().map(Vec::len).collect();
        assert_eq!(lengths, [2, 2, 1]);
        assert_eq!(segments[1][0].begin_ns, 500);
    }
}
