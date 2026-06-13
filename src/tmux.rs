use anyhow::{Context, Result};
use std::process::Command;

pub fn launch_ssh_session(alias: &str, active_sessions: &[String]) -> Result<()> {
    dbg!(&active_sessions);
    if !active_sessions.contains(&alias.to_owned()) {
        let pane_target = new_session(alias)?;
        send_ssh_command(&pane_target, alias)?;
    }

    goto_session(alias)
}

pub fn launch_docker_session(container_name: &str, active_sessions: &[String]) -> Result<()> {
    if !active_sessions.contains(&container_name.to_owned()) {
        let pane_target = new_session(container_name)?;
        send_ssh_command(&pane_target, container_name)?;
    }

    goto_session(container_name)
}

pub fn launch_tmux_session(session_name: &str,  active_sessions: &[String]) -> Result<()> {
    if !active_sessions.contains(&session_name.to_owned()) {
        let _ = new_session(session_name)?;
    }

    goto_session(session_name)
}

pub fn tmux_sessions() -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .context("failed to list tmux sessions")?;

    if !output.status.success() {
        anyhow::bail!("failed to list tmux sessions")
    }

    let sessions: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(sessions)
}

fn goto_session(name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["switch-client", "-t", name])
        .status()
        .context("failed to go to tmux session")?;

    if !status.success() {
        anyhow::bail!("failed to go to tmux session");
    }

    Ok(())
}

fn build_ssh_command(alias: &str) -> String {
    format!("ssh {}", alias)
}

fn send_docker_command(pane_target: &str, container_name: &str) -> Result<()> {
    let cmd = format!("docker exec -it {} bash", container_name);

    let status = Command::new("tmux")
        .args(["send-keys", "-t", &pane_target, &cmd, "C-m"])
        .status()
        .context("failed to send docker command to pane")?;

    if !status.success() {
        anyhow::bail!("failed to send docker command to tmux pane");
    }

    Ok(())
}

fn send_ssh_command(pane_target: &str, alias: &str) -> Result<()> {
    let ssh_command = build_ssh_command(alias);

    let status = Command::new("tmux")
        .args(["send-keys", "-t", pane_target, &ssh_command, "C-m"])
        .status()
        .context("failed to send ssh command to tmux pane")?;

    if !status.success() {
        anyhow::bail!("tmux send-keys failed");
    }

    Ok(())
}

/// Creates a new tmux session with the `name`
fn new_session(name: &str) -> Result<String> {
    let home = dirs::home_dir().context("failed to find $HOME")?;

    let output = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-P",
            "-F",
            "#{session_name}:#{window_index}.#{pane_index}",
            "-s",
            name,
            "-c",
        ])
        .arg(home)
        .output()
        .context("failed to create tmux session")?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux new-session failed!: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn tmux_has_session(session_name: &str) -> Result<bool> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .context("failed to check tmux session")?;

    println!("Here we are man dont be shy {}", status.success());
    Ok(status.success())
}
