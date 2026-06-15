use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;

const DECAY_CONSTANT: f64 = 0.0693; // ln(2)/10 for a 10-day half-life.
const SECONDS_PER_DAY: f64 = 86_400.0;
const MAX_HISTORY_DAYS: f64 = 30.0;
const MAX_TIMESTAMPS_PER_ENTRY: usize = 128;

#[derive(Debug, Default)]
pub struct History {
    entries: HashMap<String, VecDeque<u64>>,
}

impl History {
    pub fn load() -> Self {
        let Ok(content) = fs::read_to_string(history_path()) else {
            return Self::default();
        };

        let entries = content
            .lines()
            .filter_map(parse_history_line)
            .collect::<HashMap<_, _>>();

        Self { entries }
    }

    pub fn record_access(&mut self, key: &str) -> Result<()> {
        let now = unix_timestamp();
        let cutoff_time = now.saturating_sub((MAX_HISTORY_DAYS * SECONDS_PER_DAY) as u64);
        let accesses = self.entries.entry(key.to_string()).or_default();

        while let Some(&front_time) = accesses.front() {
            if front_time < cutoff_time || accesses.len() >= MAX_TIMESTAMPS_PER_ENTRY {
                accesses.pop_front();
            } else {
                break;
            }
        }

        accesses.push_back(now);
        self.save()
    }

    pub fn score(&self, key: &str) -> i64 {
        let Some(accesses) = self.entries.get(key) else {
            return 0;
        };

        access_score(accesses)
    }

    fn save(&self) -> Result<()> {
        let path = history_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create cache directory {}", parent.display())
            })?;
        }

        let mut lines = self
            .entries
            .iter()
            .map(|(key, accesses)| {
                let timestamps = accesses
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{key}\t{timestamps}")
            })
            .collect::<Vec<_>>();

        lines.sort();
        fs::write(&path, lines.join("\n"))
            .with_context(|| format!("failed to write history cache {}", path.display()))
    }
}

fn history_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("allmux")
        .join("history.tsv")
}

fn parse_history_line(line: &str) -> Option<(String, VecDeque<u64>)> {
    let mut parts = line.split('\t');
    let key = parts.next()?.to_string();
    let timestamps = parts
        .next()?
        .split(',')
        .filter_map(|timestamp| timestamp.parse::<u64>().ok())
        .collect::<VecDeque<_>>();

    if key.is_empty() || timestamps.is_empty() {
        return None;
    }

    Some((key, timestamps))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn access_score(accesses: &VecDeque<u64>) -> i64 {
    let now = unix_timestamp();
    let cutoff_time = now.saturating_sub((MAX_HISTORY_DAYS * SECONDS_PER_DAY) as u64);
    let mut total_frecency = 0.0;

    for &access_time in accesses.iter().rev() {
        if access_time < cutoff_time {
            break;
        }

        let days_ago = now.saturating_sub(access_time) as f64 / SECONDS_PER_DAY;
        let decay_factor = (-DECAY_CONSTANT * days_ago).exp();
        total_frecency += decay_factor;
    }

    let normalized_frecency = if total_frecency <= 10.0 {
        total_frecency
    } else {
        10.0 + (total_frecency - 10.0).sqrt()
    };

    normalized_frecency.round() as i64
}
