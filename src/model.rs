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

#[derive(Debug, Clone)]
pub enum Entry {
    Ssh(SshHost),
    Docker(DockerContainer),
    Tmux(TmuxSession),
}

pub enum ListEntry {
    SshHost,
    DockerContainer,
    TmuxSession,
}
