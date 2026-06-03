mod parser;
mod ui;

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let ssh_config_path = dirs::home_dir()
        .map(|home| home.join(".ssh/config"))
        .unwrap_or_else(|| Path::new(".ssh/config").to_path_buf());

    let hosts = parser::parse_ssh_config(&ssh_config_path)?;
    let containers = parser::parse_docker_containers()?;

    dbg!(&containers);

    // todo!();
    ui::run(hosts, containers)
}
