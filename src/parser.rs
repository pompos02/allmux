use crate::tmux::{self, tmux_has_session, tmux_sessions};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SshHost {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub description: Option<String>,
    pub is_active_tmux: bool,
}

#[derive(Debug, Clone)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub command: String,
    pub created_at: String,
    pub status_text: String,
    pub ports: String,
    pub status: bool,
    pub is_active_tmux: bool,
}

// Holds the active sessions and paths defined that are no sessions yet
#[derive(Debug, Clone)]
pub struct TmuxSession {
    pub full_path: Option<String>,
    pub session_name: String,
    pub is_active: bool,
}

/// Paths that will be used to in the list with depth = 1
static TMUX_PATHS: &[&str] = &[
    "", // home directory
    "projects",
    "projects/personal",
    "projects/opensource",
    "training",
    "repos",
    "projects/misc",
];

// Get the filename of the fullpath
fn basename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

/// Returns the: paths that the `TMUX_PATHS` **Global** defines,
/// their children, and the basedir name as tuple
fn tmux_dirs(paths: &[&str]) -> Result<Vec<(String, String)>> {
    let home_path = dirs::home_dir().unwrap_or_default();
    let dirs_config_path = dirs::config_dir().unwrap_or_default().push(".allmux-paths");
    let mut tmux_path_tuple: Vec<(String, String)> = Vec::new();

    for path in paths {
        let full_path = home_path.join(path);

        // Just skip if the path is not valid
        let Ok(entries) = fs::read_dir(&full_path) else {
            continue;
        };

        tmux_path_tuple.push((
            full_path.to_string_lossy().to_string(),
            basename(&full_path),
        ));

        for entry in entries.flatten() {
            let child_path = entry.path();
            if child_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }

            if child_path.is_dir() {
                tmux_path_tuple.push((
                    child_path.to_string_lossy().to_string(),
                    basename(&child_path),
                ));
            }
        }
    }

    Ok(tmux_path_tuple)
}

pub fn tmux_paths_and_sessions() -> Result<Vec<TmuxSession>> {
    let dirs_tuple = tmux_dirs(TMUX_PATHS)?;

    let mut tmux_sessions_and_paths: Vec<TmuxSession> = Vec::new();
    let active_sessions = tmux_sessions()?;
    let mut path_session_names: HashSet<String> = HashSet::new();

    // push the dirs into the tmux array
    for (full_path, basename) in dirs_tuple {
        path_session_names.insert(basename.clone());

        let mut entry = TmuxSession {
            full_path: Some(full_path),
            session_name: basename.clone(),
            is_active: false,
        };

        if active_sessions.contains(&basename) {
            entry.is_active = true
        }

        tmux_sessions_and_paths.push(entry);
    }

    for active_session in active_sessions {
        if path_session_names.contains(&active_session) {
            continue;
        }

        tmux_sessions_and_paths.push(TmuxSession {
            full_path: None,
            session_name: active_session,
            is_active: true,
        })
    }

    Ok(tmux_sessions_and_paths)
}

pub fn parse_ssh_config(path: &Path) -> Result<Vec<SshHost>> {
    let content = fs::read_to_string(path).context("failed to read shh config")?;

    let mut hosts: Vec<SshHost> = Vec::new();

    let mut current_description: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();

        if line.is_empty() {
            continue;
        }
        if line.starts_with("#") {
            let desc = current_description.get_or_insert(String::new());
            let trimmed = line.trim_start_matches("#").trim();

            desc.push_str(trimmed);
            desc.push('\n');

            continue;
        }

        let mut parts = line.split_whitespace();

        let Some(key) = parts.next() else {
            continue;
        };

        match key.to_lowercase().as_str() {
            "host" => {
                let Some(alias) = parts.next() else {
                    continue;
                };
                if alias == "*" {
                    continue;
                }

                hosts.push(SshHost {
                    alias: alias.to_string(),
                    hostname: String::new(),
                    user: String::new(),
                    description: current_description.take(),
                    is_active_tmux: false,
                });
            }
            "hostname" => {
                if let Some(ref mut host) = hosts.last_mut() {
                    host.hostname = parts.next().map(String::from).unwrap_or_default();
                }
            }
            "user" => {
                if let Some(ref mut host) = hosts.last_mut() {
                    host.user = parts.next().map(String::from).unwrap_or_default();
                }
            }
            _ => {}
        }
    }

    let tmux_sessions = tmux::tmux_sessions()?;
    for host in &mut hosts {
        if tmux_sessions.contains(&host.alias) {
            host.is_active_tmux = true;
        }
    }

    Ok(hosts)
}

pub fn parse_docker_containers() -> Result<Vec<DockerContainer>> {
    let output = Command::new("docker")
        .args(["ps", "-a"])
        .output()
        .context("Failed to execture docker command, is docker running?")?;

    let mut containers: Vec<DockerContainer> = Vec::new();

    if !output.status.success() {
        // for now we should just pass in the empty vector
        return Ok(containers);
        // let error_msg = String::from_utf8_lossy(&output.stderr);
        // return Err(anyhow::anyhow!("Docker command failed {}", error_msg));
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let mut lines = content.lines();

    let Some(header) = lines.next() else {
        return Ok(containers);
    };

    let columns = [
        "CONTAINER ID",
        "IMAGE",
        "COMMAND",
        "CREATED",
        "STATUS",
        "PORTS",
        "NAMES",
    ];

    // Get the index where each column starts
    let starts: Vec<usize> = columns
        .iter()
        .filter_map(|name| header.find(name))
        .collect();

    fn field(line: &str, starts: &[usize], index: usize) -> String {
        let Some(&start) = starts.get(index) else {
            return String::new();
        };

        let end = starts.get(index + 1).copied().unwrap_or(line.len());

        line.get(start..end).unwrap_or("").trim().to_string()
    }

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let id = field(line, &starts, 0);
        let image = field(line, &starts, 1);
        let command = field(line, &starts, 2);
        let created_at = field(line, &starts, 3);
        let status_text = field(line, &starts, 4);
        let ports = field(line, &starts, 5);
        let name = field(line, &starts, 6);
        let status = field(line, &starts, 4).contains("Up");

        containers.push(DockerContainer {
            id,
            name,
            image,
            command,
            created_at,
            status_text,
            ports,
            status,
            is_active_tmux: false,
        });
    }

    let tmux_sessions = tmux::tmux_sessions()?;
    for container in &mut containers {
        if tmux_sessions.contains(&container.name) {
            container.is_active_tmux = true;
        }
    }

    Ok(containers)
}
