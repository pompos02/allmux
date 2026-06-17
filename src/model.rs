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

impl SshHost {
    pub fn search_fields(&self) -> Vec<&str> {
        vec![
            &self.alias,
            &self.hostname,
            &self.user,
            self.description.as_deref().unwrap_or_default(),
        ]
    }
}

impl DockerContainer {
    pub fn search_fields(&self) -> Vec<&str> {
        vec![
            &self.name,
            self.status_label(),
            &self.status_text,
            &self.id,
            &self.image,
            &self.command,
            &self.created_at,
            &self.ports,
        ]
    }

    pub fn status_label(&self) -> &'static str {
        if self.status { "running" } else { "stopped" }
    }
}

impl TmuxSession {
    pub fn search_fields(&self) -> Vec<&str> {
        let display_text = self.display_text();
        let mut fields = vec![display_text];

        if display_text != self.session_name && !display_text.contains(&self.session_name) {
            fields.push(&self.session_name);
        }

        if let Some(full_path) = self.full_path.as_deref() {
            if full_path != display_text {
                fields.push(full_path);
            }
        }

        fields
    }

    pub fn display_text(&self) -> &str {
        if self.is_active {
            &self.session_name
        } else {
            self.full_path.as_deref().unwrap_or(&self.session_name)
        }
    }
}

impl Entry {
    pub fn is_active_tmux(&self) -> bool {
        match self {
            Entry::Tmux(session) => session.is_active,
            Entry::Ssh(host) => host.is_active_tmux,
            Entry::Docker(container) => container.is_active_tmux,
        }
    }

    pub fn type_rank(&self) -> u8 {
        match self {
            Entry::Tmux(_) => 3,
            Entry::Ssh(_) => 2,
            Entry::Docker(_) => 1,
        }
    }

    pub fn search_fields(&self) -> Vec<&str> {
        match self {
            Entry::Ssh(host) => {
                let mut fields = host.search_fields();
                fields.push("ssh");
                fields
            }
            Entry::Docker(container) => {
                let mut fields = container.search_fields();
                fields.push("docker");
                fields.push("doc");
                fields
            }
            Entry::Tmux(session) => {
                let mut fields = session.search_fields();
                fields.push("tmux");
                fields.push("mux");
                fields
            }
        }
    }

    pub fn display_search_fields(&self) -> Vec<&str> {
        match self {
            Entry::Ssh(host) => {
                let fields = host.search_fields();
                vec![fields[0], fields[1]]
            }
            Entry::Docker(container) => {
                let fields = container.search_fields();
                vec![fields[0], fields[1]]
            }
            Entry::Tmux(session) => {
                let fields = session.search_fields();
                vec![fields[0]]
            }
        }
    }

    pub fn search_text(&self) -> String {
        join_search_fields(&self.search_fields())
    }

    pub fn display_search_text(&self) -> String {
        join_search_fields(&self.display_search_fields())
    }

    pub fn history_key(&self) -> String {
        match self {
            Entry::Ssh(host) => format!("ssh:{}", host.alias),
            Entry::Docker(container) => format!("docker:{}", container.name),
            Entry::Tmux(session) => match &session.full_path {
                Some(path) => format!("tmux:{path}"),
                None => format!("tmux-session:{}", session.session_name),
            },
        }
    }
}

fn join_search_fields(fields: &[&str]) -> String {
    fields.join(" ")
}
