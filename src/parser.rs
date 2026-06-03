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
    pub name: String,
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
        .args(["ps", "-a", "--format", "{{.Names}}\t{{.Status}}"])
        .output()
        .context("Failed to execture docker command, is docker running?")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Docker command failed {}", error_msg));
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let mut containers: Vec<DockerContainer> = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.split('\t');

        if let Some(raw_name) = parts.next() {
            // return everything before the first ':' or the whole string
            let name = raw_name.split(":").next().unwrap_or(raw_name);
            containers.push(DockerContainer {
                name: name.to_string(),
                status: false,
            });
        }

        if let Some(status) = parts.next() {
            let Some(container) = containers.last_mut() else {
                continue;
            };
            container.status = status.contains("Up");
        }
    }

    Ok(containers)
}
