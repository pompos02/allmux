use crate::history::History;
use crate::model::{DockerContainer, Entry, SshHost, TmuxSession};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub enum UiAction {
    LaunchSsh(String),
    LaunchDocker(String),
    LaunchTmux(String, Option<String>),
}

enum KeyAction {
    Continue,
    Quit,
    Select(UiAction),
}

enum StatusKind {
    Success,
    Warning,
    Error,
}

struct StatusMessage {
    text: String,
    kind: StatusKind,
}

impl Entry {
    fn is_active_tmux(&self) -> bool {
        match self {
            Entry::Tmux(session) => session.is_active,
            Entry::Ssh(host) => host.is_active_tmux,
            Entry::Docker(container) => container.is_active_tmux,
        }
    }

    fn type_rank(&self) -> u8 {
        match self {
            Entry::Tmux(_) => 3,
            Entry::Ssh(_) => 2,
            Entry::Docker(_) => 1,
        }
    }

    fn marker_color(&self) -> Color {
        match self {
            Entry::Ssh(_) => Color::Cyan,
            Entry::Docker(_) => Color::Blue,
            Entry::Tmux(_) => Color::Green,
        }
    }

    fn list_line(&self, matched_indices: &[usize], selected: bool) -> Line<'static> {
        match self {
            Entry::Tmux(session) => {
                let search_fields = tmux_search_fields(session);
                let display_text = search_fields[0];
                let display_offset = search_field_offset(&search_fields, 0);
                let mut spans = vec![
                    Span::styled(
                        "",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap(" ", selected),
                ];

                if session.is_active {
                    spans.extend(highlighted_text(
                        display_text,
                        matched_indices,
                        display_offset,
                        selected,
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled(
                        "*",
                        selected_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            selected,
                        ),
                    ));
                } else {
                    spans.extend(highlighted_text(
                        display_text,
                        matched_indices,
                        display_offset,
                        selected,
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(styled_gap("  ", selected));
                Line::from(spans)
            }

            Entry::Ssh(host) => {
                let search_fields = ssh_search_fields(host);
                let mut spans = vec![
                    Span::styled(
                        "",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap(" ", selected),
                ];
                spans.extend(highlighted_text(
                    &host.alias,
                    matched_indices,
                    search_field_offset(&search_fields, 0),
                    selected,
                    Style::default().fg(Color::White),
                ));
                if host.is_active_tmux {
                    spans.push(Span::styled(
                        "*",
                        selected_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            selected,
                        ),
                    ));
                }
                spans.push(styled_gap("  ", selected));
                spans.extend(highlighted_text(
                    &host.hostname,
                    matched_indices,
                    search_field_offset(&search_fields, 1),
                    selected,
                    Style::default().fg(Color::DarkGray),
                ));
                Line::from(spans)
            }
            Entry::Docker(container) => {
                let search_fields = docker_search_fields(container);
                let status_style = if container.status {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };

                let mut spans = vec![
                    Span::styled(
                        "",
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap(" ", selected),
                ];
                spans.extend(highlighted_text(
                    &container.name,
                    matched_indices,
                    search_field_offset(&search_fields, 0),
                    selected,
                    Style::default().fg(Color::White),
                ));
                if container.is_active_tmux {
                    spans.push(Span::styled(
                        "*",
                        selected_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            selected,
                        ),
                    ));
                }
                spans.push(styled_gap("  ", selected));
                spans.extend(highlighted_text(
                    docker_status_label(container),
                    matched_indices,
                    search_field_offset(&search_fields, 1),
                    selected,
                    status_style,
                ));
                Line::from(spans)
            }
        }
    }

    fn search_fields(&self) -> Vec<&str> {
        match self {
            Entry::Ssh(host) => {
                let mut fields = ssh_search_fields(host);
                fields.push("ssh");
                fields
            }
            Entry::Docker(container) => {
                let mut fields = docker_search_fields(container);
                fields.push("docker");
                fields.push("doc");
                fields
            }
            Entry::Tmux(session) => {
                let mut fields = tmux_search_fields(session);
                fields.push("tmux");
                fields.push("mux");
                fields
            }
        }
    }

    fn display_search_fields(&self) -> Vec<&str> {
        match self {
            Entry::Ssh(host) => {
                let fields = ssh_search_fields(host);
                vec![fields[0], fields[1]]
            }
            Entry::Docker(container) => {
                let fields = docker_search_fields(container);
                vec![fields[0], fields[1]]
            }
            Entry::Tmux(session) => {
                let fields = tmux_search_fields(session);
                vec![fields[0]]
            }
        }
    }

    fn display_match_indices(&self, matcher: &SkimMatcherV2, query: &str) -> Option<Vec<usize>> {
        let fields = self.display_search_fields();
        let text = join_search_fields(&fields);
        matcher
            .fuzzy_indices(&text, query)
            .map(|(_, indices)| indices)
    }

    fn search_text(&self) -> String {
        join_search_fields(&self.search_fields())
    }

    fn history_key(&self) -> String {
        match self {
            Entry::Ssh(host) => format!("ssh:{}", host.alias),
            Entry::Docker(container) => format!("docker:{}", container.name),
            Entry::Tmux(session) => match &session.full_path {
                Some(path) => format!("tmux:{path}"),
                None => format!("tmux-session:{}", session.session_name),
            },
        }
    }

    fn preview(&self) -> Vec<Line<'static>> {
        match self {
            Entry::Tmux(session) => {
                let mut lines = vec![
                    Line::from(Span::styled(
                        "Tmux Session",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::default(),
                    field_line("Name:", &session.session_name, Color::Green),
                    field_line(
                        "Path:",
                        session.full_path.as_deref().unwrap_or("-"),
                        Color::default(),
                    ),
                    Line::default(),
                    Line::from(Span::styled(
                        "Files",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )),
                ];

                match &session.preview {
                    Some(preview) if !preview.trim().is_empty() => {
                        lines.extend(preview.lines().take(30).map(tmux_file_preview_line));
                    }
                    _ => {
                        lines.push(Line::from(Span::styled(
                            "No preview available.",
                            Style::default().fg(Color::default()),
                        )));
                    }
                }

                lines
            }
            Entry::Ssh(host) => {
                let mut lines = vec![
                    Line::from(Span::styled(
                        "SSH Host",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::default(),
                    field_line("Host:", value_or_dash(&host.alias), Color::default()),
                    field_line("User:", value_or_dash(&host.user), Color::default()),
                    field_line("Hostname:", value_or_dash(&host.hostname), Color::default()),
                    Line::default(),
                    Line::from(Span::styled(
                        "Description:",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )),
                ];

                let description = host.description.as_deref().unwrap_or("-").trim_end();
                lines.extend(description.lines().map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Green),
                    ))
                }));
                lines
            }
            Entry::Docker(container) => vec![
                Line::from(Span::styled(
                    "Docker Container",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::default(),
                field_line("Name", &container.name, Color::Yellow),
                field_line("ID", &container.id, Color::Cyan),
                field_line("Image", &container.image, Color::Green),
                field_line(
                    "Command",
                    value_or_dash(&container.command),
                    Color::default(),
                ),
                Line::default(),
                field_line("Created", value_or_dash(&container.created_at), Color::Blue),
                field_line("Ports", value_or_dash(&container.ports), Color::Magenta),
                Line::default(),
                Line::from(vec![
                    Span::styled(
                        "Status",
                        Style::default()
                            .fg(Color::default())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        if container.status {
                            "running"
                        } else {
                            "stopped"
                        },
                        Style::default()
                            .fg(if container.status {
                                Color::Green
                            } else {
                                Color::Red
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                field_line(
                    "Details",
                    value_or_dash(&container.status_text),
                    Color::default(),
                ),
            ],
        }
    }
}

struct App {
    entries: Vec<Entry>,
    query: String,
    selected: usize,
    status_message: Option<StatusMessage>,
    preview_expanded: bool,
    history: History,
}

struct Match {
    index: usize,
    score: i64,
    indices: Vec<usize>,
}

const MATCH_HIGHLIGHT_BG: Color = Color::Rgb(94, 241, 255);
const SELECTED_BG: Color = Color::Gray;
const SUBTLE_BORDER: Color = Color::Rgb(52, 52, 52);

impl App {
    fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            query: String::new(),
            selected: 0,
            status_message: None,
            preview_expanded: false,
            history: History::load(),
        }
    }

    fn filtered_matches(&self) -> Vec<Match> {
        let matcher = SkimMatcherV2::default();

        let mut matches: Vec<Match> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                if self.query.is_empty() {
                    return Some(Match {
                        index,
                        score: self.history.score(&entry.history_key()),
                        indices: Vec::new(),
                    });
                }

                matcher
                    .fuzzy_indices(&entry.search_text(), &self.query)
                    .map(|(score, indices)| Match {
                        index,
                        score: score
                            + score.saturating_mul(self.history.score(&entry.history_key())) / 100,
                        indices: entry
                            .display_match_indices(&matcher, &self.query)
                            .unwrap_or(indices),
                    })
            })
            .collect();

        matches.sort_by(|left, right| {
            let left_entry = &self.entries[left.index];
            let right_entry = &self.entries[right.index];

            right
                .score
                .cmp(&left.score)
                .then_with(|| {
                    right_entry
                        .is_active_tmux()
                        .cmp(&left_entry.is_active_tmux())
                })
                .then_with(|| right_entry.type_rank().cmp(&left_entry.type_rank()))
                .then_with(|| left.index.cmp(&right.index))
        });
        matches
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_matches().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_up_by(&mut self, count: usize) {
        self.selected = self.selected.saturating_sub(count);
    }

    fn move_down(&mut self) {
        let len = self.filtered_matches().len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }

    fn move_down_by(&mut self, count: usize) {
        let len = self.filtered_matches().len();
        if len > 0 {
            self.selected = (self.selected + count).min(len - 1);
        }
    }

    fn selected_action(&mut self) -> Option<UiAction> {
        let filtered = self.filtered_matches();
        let matched = filtered.get(self.selected)?;
        let entry = &self.entries[matched.index];
        let history_key = entry.history_key();

        let _ = self.history.record_access(&history_key);

        match entry {
            Entry::Tmux(session) => Some(UiAction::LaunchTmux(
                session.session_name.clone(),
                session.full_path.clone(),
            )),
            Entry::Ssh(host) => Some(UiAction::LaunchSsh(host.alias.clone())),
            Entry::Docker(container) => Some(UiAction::LaunchDocker(container.name.clone())),
        }
    }

    fn selected_ssh_hostname(&self) -> Option<String> {
        let filtered = self.filtered_matches();
        let matched = filtered.get(self.selected)?;

        match &self.entries[matched.index] {
            Entry::Ssh(host) if !host.hostname.is_empty() => Some(host.hostname.clone()),
            _ => None,
        }
    }

    fn toggle_preview(&mut self) {
        self.preview_expanded = !self.preview_expanded;
        self.status_message = None;
    }
}

pub fn run(entries: Vec<Entry>) -> Result<Option<UiAction>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, App::new(entries));

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<Option<UiAction>> {
    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match handle_key(key, &mut app) {
                KeyAction::Continue => {}
                KeyAction::Quit => return Ok(None),
                KeyAction::Select(action) => return Ok(Some(action)),
            }
        }
    }
}

fn handle_key(key: KeyEvent, app: &mut App) -> KeyAction {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return KeyAction::Quit;
        }
        KeyCode::Char('q') if app.query.is_empty() => return KeyAction::Quit,
        KeyCode::Esc => return KeyAction::Quit,
        KeyCode::Enter => {
            if let Some(action) = app.selected_action() {
                return KeyAction::Select(action);
            }
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.query.clear();
            app.clamp_selection();
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            delete_previous_word(&mut app.query);
            app.clamp_selection();
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.status_message = match app.selected_ssh_hostname() {
                Some(hostname) => match copy_to_tmux_clipboard(&hostname) {
                    Ok(()) => Some(StatusMessage {
                        text: format!("Copied hostname: {hostname}"),
                        kind: StatusKind::Success,
                    }),
                    Err(error) => Some(StatusMessage {
                        text: format!("Failed to copy hostname: {error}"),
                        kind: StatusKind::Error,
                    }),
                },
                None => Some(StatusMessage {
                    text: "Ctrl-Y only copies SSH entries with a hostname".to_string(),
                    kind: StatusKind::Warning,
                }),
            };
        }
        KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_preview();
        }
        KeyCode::Up => app.move_down(),
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => app.move_down(),
        KeyCode::PageUp => app.move_down_by(5),
        KeyCode::Down => app.move_up(),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => app.move_up(),
        KeyCode::PageDown => app.move_up_by(5),
        KeyCode::Backspace => {
            app.query.pop();
            app.clamp_selection();
            app.status_message = None;
        }
        KeyCode::Char(c) => {
            app.query.push(c);
            app.selected = 0;
            app.status_message = None;
        }
        _ => {}
    }

    KeyAction::Continue
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    let panes = app.preview_expanded.then(|| {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area)
    });
    let left_area = panes.as_ref().map_or(area, |panes| panes[0]);

    let filtered = app.filtered_matches();

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SUBTLE_BORDER));
    let left_inner = left_block.inner(left_area);
    frame.render_widget(left_block, left_area);

    let left_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(visible_list_height(left_inner, filtered.len())),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(left_inner);

    app.clamp_selection();
    let list_height = left_sections[1].height as usize;
    let selected_visual = logical_to_visual_index(filtered.len(), app.selected);
    let visible_window = visible_visual_window(filtered.len(), selected_visual, list_height);

    let items: Vec<ListItem> = visible_window
        .clone()
        .map(|visual_index| {
            let logical_index = visual_to_logical_index(filtered.len(), visual_index);
            let matched = &filtered[logical_index];
            let entry = &app.entries[matched.index];
            let selected = visual_index == selected_visual;
            let line = selection_marker_line(
                entry.list_line(&matched.indices, selected),
                selected,
                entry.marker_color(),
            );

            ListItem::new(line).style(selected_style(Style::default(), selected))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default())
        .highlight_symbol("");

    let mut state = ListState::default();
    if filtered.is_empty() || list_height == 0 {
        state.select(None);
    } else {
        state.select(Some(selected_visual - visible_window.start));
    }
    frame.render_stateful_widget(list, left_sections[1], &mut state);

    let divider = Paragraph::new("").block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(SUBTLE_BORDER)),
    );
    frame.render_widget(divider, left_sections[2]);

    let (prompt_text, prompt_style) = if let Some(status) = &app.status_message {
        let color = match status.kind {
            StatusKind::Success => Color::Green,
            StatusKind::Warning => Color::Yellow,
            StatusKind::Error => Color::Red,
        };

        (
            status.text.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    } else {
        (format!("> {}", app.query), Style::default())
    };

    let prompt = Paragraph::new(prompt_text).style(prompt_style);
    frame.render_widget(prompt, left_sections[3]);
    if app.status_message.is_none() {
        frame.set_cursor_position(Position::new(
            left_sections[3]
                .x
                .saturating_add(2)
                .saturating_add(app.query.chars().count() as u16),
            left_sections[3].y,
        ));
    }

    if let Some(panes) = panes {
        let preview = filtered
            .get(app.selected)
            .map(|matched| {
                let entry = &app.entries[matched.index];
                preview_with_score(entry, matched.score)
            })
            .unwrap_or_else(|| {
                vec![Line::from(Span::styled(
                    "No entries match the current search.",
                    Style::default().fg(Color::DarkGray),
                ))]
            });

        let preview = Paragraph::new(preview)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(SUBTLE_BORDER)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(preview, panes[1]);
    }
}

fn visible_list_height(area: Rect, item_count: usize) -> u16 {
    let reserved_rows = 2;
    let available_rows = area.height.saturating_sub(reserved_rows);
    available_rows.min(item_count as u16)
}

fn logical_to_visual_index(len: usize, logical_index: usize) -> usize {
    len.saturating_sub(1).saturating_sub(logical_index)
}

fn visual_to_logical_index(len: usize, visual_index: usize) -> usize {
    len.saturating_sub(1).saturating_sub(visual_index)
}

fn visible_visual_window(
    len: usize,
    selected_visual: usize,
    height: usize,
) -> std::ops::Range<usize> {
    if len == 0 || height == 0 {
        return 0..0;
    }

    if len <= height {
        return 0..len;
    }

    let bottom_start = len - height;
    if selected_visual >= bottom_start {
        return bottom_start..len;
    }

    selected_visual..selected_visual + height
}

fn value_or_dash(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn preview_with_score(entry: &Entry, score: i64) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Score: ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(score.to_string(), Style::default().fg(Color::Yellow)),
        ]),
        Line::default(),
    ];

    lines.extend(entry.preview());
    lines
}

fn tmux_file_preview_line(line: &str) -> Line<'static> {
    let Some((permissions, rest)) = line.split_once(' ') else {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    };
    let Some((name, size)) = rest.rsplit_once(' ') else {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    };

    let name_color = if permissions.starts_with('d') {
        Color::Rgb(91, 192, 222)
    } else {
        Color::default()
    };
    let name_width: usize = 24;
    let name_padding = name_width.saturating_sub(name.chars().count()).max(1);

    let mut spans = vec![
        Span::styled(name.to_string(), Style::default().fg(name_color)),
        Span::raw(" ".repeat(name_padding)),
    ];

    spans.extend(permissions.chars().map(|character| {
        Span::styled(
            character.to_string(),
            Style::default().fg(permission_char_color(character)),
        )
    }));

    spans.extend([
        Span::raw("  "),
        Span::styled(size.to_string(), Style::default().fg(Color::LightMagenta)),
    ]);

    Line::from(spans)
}

fn permission_char_color(character: char) -> Color {
    match character {
        'd' | 'l' => Color::Rgb(68, 180, 220),
        'r' | 'w' => Color::Rgb(214, 169, 102),
        'x' => Color::Rgb(156, 188, 112),
        '-' => Color::default(),
        _ => Color::Gray,
    }
}

fn ssh_search_fields(host: &SshHost) -> Vec<&str> {
    vec![
        &host.alias,
        &host.hostname,
        &host.user,
        host.description.as_deref().unwrap_or_default(),
    ]
}

fn docker_search_fields(container: &DockerContainer) -> Vec<&str> {
    vec![
        &container.name,
        docker_status_label(container),
        &container.status_text,
        &container.id,
        &container.image,
        &container.command,
        &container.created_at,
        &container.ports,
    ]
}

fn tmux_search_fields(session: &TmuxSession) -> Vec<&str> {
    let display_text = tmux_display_text(session);
    let mut fields = vec![display_text];

    if display_text != session.session_name && !display_text.contains(&session.session_name) {
        fields.push(&session.session_name);
    }

    if let Some(full_path) = session.full_path.as_deref() {
        if full_path != display_text {
            fields.push(full_path);
        }
    }

    fields
}

fn tmux_display_text(session: &TmuxSession) -> &str {
    if session.is_active {
        &session.session_name
    } else {
        session
            .full_path
            .as_deref()
            .unwrap_or(&session.session_name)
    }
}

fn docker_status_label(container: &DockerContainer) -> &'static str {
    if container.status {
        "running"
    } else {
        "stopped"
    }
}

fn join_search_fields(fields: &[&str]) -> String {
    fields.join(" ")
}

fn search_field_offset(fields: &[&str], field_index: usize) -> usize {
    fields
        .iter()
        .take(field_index)
        .map(|field| field.chars().count() + 1)
        .sum()
}

fn delete_previous_word(query: &mut String) {
    // skip all the leading whitespaces
    while query
        .chars()
        .last()
        .is_some_and(|character| character.is_whitespace())
    {
        query.pop();
    }

    while query
        .chars()
        .last()
        .is_some_and(|character| !character.is_whitespace())
    {
        query.pop();
    }
}

fn copy_to_tmux_clipboard(value: &str) -> Result<()> {
    let mut child = Command::new("tmux")
        .args(["load-buffer", "-w", "-"])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(value.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("tmux load-buffer failed");
    }

    Ok(())
}

fn field_line(label: &'static str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            label,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(value.to_string(), Style::default().fg(value_color)),
    ])
}

fn highlighted_text(
    text: &str,
    matched_indices: &[usize],
    offset: usize,
    selected: bool,
    base_style: Style,
) -> Vec<Span<'static>> {
    text.chars()
        .enumerate()
        .map(|(index, character)| {
            let style = if matched_indices.contains(&(index + offset)) {
                base_style
                    .fg(Color::Black)
                    .bg(MATCH_HIGHLIGHT_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                selected_style(base_style, selected)
            };

            Span::styled(character.to_string(), style)
        })
        .collect()
}

fn styled_gap(text: &'static str, selected: bool) -> Span<'static> {
    Span::styled(text, selected_style(Style::default(), selected))
}

fn selection_marker_line(
    mut line: Line<'static>,
    selected: bool,
    marker_color: Color,
) -> Line<'static> {
    let marker = if selected { "▌ " } else { "  " };
    let style = if selected {
        Style::default().fg(marker_color).bg(SELECTED_BG)
    } else {
        Style::default()
    };

    line.spans.insert(0, Span::styled(marker, style));
    line
}

fn selected_style(style: Style, selected: bool) -> Style {
    if selected {
        style.bg(SELECTED_BG).add_modifier(Modifier::BOLD)
    } else {
        style
    }
}
