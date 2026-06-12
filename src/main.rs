mod parser;
mod tmux;
mod ui;


use std::path::Path;
use crate::tmux::tmux_sessions;

fn main() -> anyhow::Result<()> {
    let ssh_config_path = dirs::home_dir()
        .map(|home| home.join(".ssh/config"))
        .unwrap_or_else(|| Path::new(".ssh/config").to_path_buf());

    let hosts = parser::parse_ssh_config(&ssh_config_path)?;
    let containers = parser::parse_docker_containers()?;
    let tmux_paths_and_sessions = parser::tmux_paths_and_sessions()?;
    let active_tmux_sessions = tmux_sessions()?;
    dbg!(&tmux_paths_and_sessions);

    if let Some(action) = ui::run(hosts, containers, tmux_paths_and_sessions)? {
        match action {
            ui::UiAction::LaunchSsh(alias) => tmux::launch_ssh_session(&alias, &active_tmux_sessions)?,
            ui::UiAction::LaunchDocker(container_name) => {
                tmux::launch_docker_session(&container_name, &active_tmux_sessions)?;
            }
            ui::UiAction::LaunchTmux(session_name) => tmux::launch_tmux_session(&session_name, &active_tmux_sessions)?

            }
        }
    Ok(())
}
