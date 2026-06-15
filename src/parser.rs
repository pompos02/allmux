use crate::tmux::{self, tmux_sessions};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
    pub preview: Option<String>,
}

// Get the filename of the fullpath
fn basename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

fn push_tmux_dir(
    dirs: &mut Vec<(String, String)>,
    seen_paths: &mut HashSet<PathBuf>,
    path: &Path,
) -> Result<()> {
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve tmux path {}", path.display()))?;

    if !seen_paths.insert(canonical_path.clone()) {
        return Ok(());
    }

    dirs.push((
        canonical_path.to_string_lossy().to_string(),
        basename(&canonical_path),
    ));

    Ok(())
}

/// Returns the: paths that the `TMUX_PATHS` **Global** defines,
/// their children, and the basedir name as tuple
fn tmux_dirs() -> Result<Vec<(String, String)>> {
    let home_path = dirs::home_dir().unwrap_or_default();

    let config_file_path = dirs::config_dir()
        .context("cannot find $HOME")?
        .join(".allmux");

    let content = fs::read_to_string(&config_file_path).with_context(|| format!("Could not read the file at {:?}", config_file_path))?;

    let mut tmux_path_tuple: Vec<(String, String)> = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

    for path in content.lines() {
        let full_path = home_path.join(path);

        // Just skip if the path is not valid
        let Ok(entries) = fs::read_dir(&full_path) else {
            continue;
        };

        push_tmux_dir(&mut tmux_path_tuple, &mut seen_paths, &full_path)?;

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
                push_tmux_dir(&mut tmux_path_tuple, &mut seen_paths, &child_path)?;
            }
        }
    }

    Ok(tmux_path_tuple)
}

fn tmux_ls_preview(path: &str) -> Option<String> {
    let mut entries = fs::read_dir(path)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            let permissions = format_permissions(metadata.permissions().mode());

            Some((name, permissions, human_size(metadata.len())))
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|(name, _, _)| name.to_lowercase());

    Some(
        entries
            .into_iter()
            .map(|(name, permissions, size)| format!("{permissions} {name} {size}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn format_permissions(mode: u32) -> String {
    let file_type = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        _ => '-',
    };

    let mut permissions = String::with_capacity(10);
    permissions.push(file_type);

    for bit in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ] {
        permissions.push(match (mode & bit != 0, bit) {
            (true, 0o400 | 0o040 | 0o004) => 'r',
            (true, 0o200 | 0o020 | 0o002) => 'w',
            (true, 0o100 | 0o010 | 0o001) => 'x',
            _ => '-',
        });
    }

    permissions
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{}{}", bytes, UNITS[unit])
    } else if size < 10.0 {
        format!("{:.1}{}", size, UNITS[unit])
    } else {
        format!("{:.0}{}", size, UNITS[unit])
    }
}

pub fn tmux_paths_and_sessions() -> Result<Vec<TmuxSession>> {
    let dirs_tuple = tmux_dirs()?;

    let mut tmux_sessions_and_paths: Vec<TmuxSession> = Vec::new();
    let active_sessions = tmux_sessions()?;
    let mut path_session_names: HashSet<String> = HashSet::new();

    // push the dirs into the tmux array
    for (full_path, basename) in dirs_tuple {
        path_session_names.insert(basename.clone());

        let mut entry = TmuxSession {
            full_path: Some(full_path.clone()),
            session_name: basename.clone(),
            is_active: false,
            preview: tmux_ls_preview(&full_path),
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
            preview: None,
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
