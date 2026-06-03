mod parser;

use parser::{DockerContainer, SshHost};
use std::path::Path;

fn main() {
    let ssh_config_path = Path::new("/home/karavellas/.ssh/config");
    let hosts: Vec<SshHost> = parser::parse_ssh_config(ssh_config_path).unwrap();
    let containers: Vec<DockerContainer> = parser::parse_docker_containers().unwrap();
    dbg!(hosts);
    dbg!(containers);
}
