mod parser;
mod tmux;
mod ui;

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let ssh_config_path = dirs::home_dir()
        .map(|home| home.join(".ssh/config"))
        .unwrap_or_else(|| Path::new(".ssh/config").to_path_buf());

    let hosts = parser::parse_ssh_config(&ssh_config_path)?;
    let containers = parser::parse_docker_containers()?;

    if let Some(action) = ui::run(hosts, containers)? {
        match action {
            ui::UiAction::LaunchSsh(alias) => tmux::launch_ssh_session(&alias)?,
            ui::UiAction::LaunchDocker(container_name) => {
                tmux::launch_docker_session(&container_name)?
            }
        }
    }

    Ok(())
}
