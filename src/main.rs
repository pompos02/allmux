mod history;
mod model;
mod parser;
mod tmux;
mod ui;

use crate::tmux::tmux_sessions;

fn main() -> anyhow::Result<()> {

    let active_tmux_sessions = tmux_sessions()?;

    let entries = parser::build_entries(&active_tmux_sessions)?;

    if let Some(action) = ui::run(entries)? {
        match action {
            ui::UiAction::LaunchSsh(alias) => {
                tmux::launch_ssh_session(&alias, &active_tmux_sessions)?
            }
            ui::UiAction::LaunchDocker(container_name) => {
                tmux::launch_docker_session(&container_name, &active_tmux_sessions)?;
            }
            ui::UiAction::LaunchTmux(session_name, full_path) => tmux::launch_tmux_session(
                &session_name,
                full_path.as_deref(),
                &active_tmux_sessions,
            )?,
        }
    }
    Ok(())
}
