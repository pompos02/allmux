use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SshHost {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub description: Option<String>,
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

    Ok(hosts)
}

pub fn parse_docker_containers() -> Result<Vec<DockerContainer>> {
    let output = Command::new("docker")
        .args(["ps", "-a"])
        .output()
        .context("Failed to execture docker command, is docker running?")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Docker command failed {}", error_msg));
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let mut containers: Vec<DockerContainer> = Vec::new();
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
        });
    }

    Ok(containers)
}
