use crate::history::History;
use crate::model::Entry;
use crate::search::{SearchMatch, filtered_matches};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::fs;
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

struct Theme {
    selected_bg: Color,
    match_highlight_bg: Color,
    matched_char_fg: Color,
}

impl Theme {
    fn selected_style(&self, style: Style, selected: bool) -> Style {
        if selected {
            style.bg(self.selected_bg).add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }
}

impl Entry {
    fn marker_color(&self) -> Color {
        match self {
            Entry::Ssh(_) => Color::Cyan,
            Entry::Docker(_) => Color::Blue,
            Entry::Tmux(_) => Color::Green,
        }
    }

    fn list_line(&self, matched_indices: &[usize], selected: bool, theme: &Theme) -> Line<'static> {
        match self {
            Entry::Tmux(session) => {
                let search_fields = session.search_fields();
                let display_text = search_fields[0];
                let display_offset = search_field_offset(&search_fields, 0);
                let mut spans = vec![
                    Span::styled(
                        "",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap(" ", selected, theme),
                ];

                if session.is_active {
                    spans.extend(highlighted_text(
                        display_text,
                        matched_indices,
                        display_offset,
                        selected,
                        theme,
                        Style::default().fg(Color::White),
                    ));
                    spans.push(Span::styled(
                        "*",
                        theme.selected_style(
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
                        theme,
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(styled_gap("  ", selected, theme));
                Line::from(spans)
            }

            Entry::Ssh(host) => {
                let search_fields = host.search_fields();
                let mut spans = vec![
                    Span::styled(
                        "",
                        Style::default()
                            .fg(Color::LightMagenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap(" ", selected, theme),
                ];
                spans.extend(highlighted_text(
                    &host.alias,
                    matched_indices,
                    search_field_offset(&search_fields, 0),
                    selected,
                    theme,
                    Style::default().fg(Color::White),
                ));
                if host.is_active_tmux {
                    spans.push(Span::styled(
                        "*",
                        theme.selected_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            selected,
                        ),
                    ));
                }
                spans.push(styled_gap("  ", selected, theme));
                spans.extend(highlighted_text(
                    &host.hostname,
                    matched_indices,
                    search_field_offset(&search_fields, 1),
                    selected,
                    theme,
                    Style::default().fg(Color::DarkGray),
                ));
                Line::from(spans)
            }
            Entry::Docker(container) => {
                let search_fields = container.search_fields();
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
                    styled_gap(" ", selected, theme),
                ];
                spans.extend(highlighted_text(
                    &container.name,
                    matched_indices,
                    search_field_offset(&search_fields, 0),
                    selected,
                    theme,
                    Style::default().fg(Color::White),
                ));
                if container.is_active_tmux {
                    spans.push(Span::styled(
                        "*",
                        theme.selected_style(
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            selected,
                        ),
                    ));
                }
                spans.push(styled_gap("  ", selected, theme));
                spans.extend(highlighted_text(
                    container.status_label(),
                    matched_indices,
                    search_field_offset(&search_fields, 1),
                    selected,
                    theme,
                    status_style,
                ));
                Line::from(spans)
            }
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
    color_variant: String,
}

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
            color_variant: "dark".to_string(),
        }
    }

    fn color(mut self, color: &str) -> Self {
        self.color_variant = match color.trim() {
            "light" => "light",
            _ => "dark",
        }
        .to_string();

        self
    }

    fn filtered_matches(&self) -> Vec<SearchMatch> {
        filtered_matches(&self.entries, &self.query, &self.history)
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

    fn selected_entry(&self) -> Option<String> {
        let filtered = self.filtered_matches();
        let matched = filtered.get(self.selected)?;

        match &self.entries[matched.index] {
            Entry::Ssh(host) if !host.hostname.is_empty() => Some(host.hostname.clone()),
            Entry::Tmux(session) if session.full_path.is_some() => session.full_path.clone(),
            _ => None,
        }
    }

    fn toggle_preview(&mut self) {
        self.preview_expanded = !self.preview_expanded;
        self.status_message = None;
    }

    fn theme(&self) -> Theme {
        match self.color_variant.as_str() {
            "light" => Theme {
                selected_bg: Color::Rgb(228, 231, 234),
                match_highlight_bg: Color::Rgb(209, 0, 191),
                matched_char_fg: Color::Black,
            },
            _ => Theme {
                selected_bg: Color::Rgb(60, 64, 72),
                match_highlight_bg: Color::Rgb(94, 241, 255),
                matched_char_fg: Color::Black,
            },
        }
    }

    fn switch_theme(&mut self) -> Result<()> {
        let file = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("allmux")
            .join("color_variant");

        match self.color_variant.as_str() {
            "dark" => {
                fs::write(&file, "light").context("Error writing to color_variant config file")?;
                self.color_variant = "light".to_string();
            }
            "light" => {
                fs::write(&file, "dark").context("Error writing to color_variant config file")?;
                self.color_variant = "dark".to_string();
            }
            _ => {}
        }
        Ok(())
    }
}

fn get_color_variant() -> Result<String> {
    let file = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("allmux")
        .join("color_variant");

    if !file.exists() {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).context("Error creating the allmux cache directory")?;
        }
        fs::write(&file, "dark").context("Error creating the color_variant config file")?;
    }

    fs::read_to_string(file)
        .map(|color| color.trim().to_string())
        .context("Error opening the color_variant config file")
}

pub fn run(entries: Vec<Entry>) -> Result<Option<UiAction>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = get_color_variant()?;
    let app = App::new(entries).color(&theme);

    let result = run_app(&mut terminal, app);

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
            match handle_key(key, &mut app)? {
                KeyAction::Continue => {}
                KeyAction::Quit => return Ok(None),
                KeyAction::Select(action) => return Ok(Some(action)),
            }
        }
    }
}

/// Keymaps definitions
fn handle_key(key: KeyEvent, app: &mut App) -> Result<KeyAction> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(KeyAction::Quit);
        }
        KeyCode::Char('q') if app.query.is_empty() => return Ok(KeyAction::Quit),
        KeyCode::Esc => return Ok(KeyAction::Quit),
        KeyCode::Enter => {
            if let Some(action) = app.selected_action() {
                return Ok(KeyAction::Select(action));
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
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.switch_theme()?;
        }

        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.status_message = match app.selected_entry() {
                Some(entry_info) => match copy_to_tmux_clipboard(&entry_info) {
                    Ok(()) => Some(StatusMessage {
                        text: format!("Copied: {entry_info}"),
                        kind: StatusKind::Success,
                    }),
                    Err(error) => Some(StatusMessage {
                        text: format!("Failed to copy: {error}"),
                        kind: StatusKind::Error,
                    }),
                },
                None => Some(StatusMessage {
                    text: "Ctrl-Y only copies SSH entries with a hostname, and session full paths"
                        .to_string(),
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

    Ok(KeyAction::Continue)
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
    let theme = app.theme();

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
                entry.list_line(&matched.indices, selected, &theme),
                selected,
                entry.marker_color(),
            );

            ListItem::new(line).style(theme.selected_style(Style::default(), selected))
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
    theme: &Theme,
    base_style: Style,
) -> Vec<Span<'static>> {
    text.chars()
        .enumerate()
        .map(|(index, character)| {
            let style = if matched_indices.contains(&(index + offset)) {
                base_style
                    .fg(theme.matched_char_fg)
                    .bg(theme.match_highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.selected_style(base_style, selected)
            };

            Span::styled(character.to_string(), style)
        })
        .collect()
}

fn styled_gap(text: &'static str, selected: bool, theme: &Theme) -> Span<'static> {
    Span::styled(text, theme.selected_style(Style::default(), selected))
}

fn selection_marker_line(
    mut line: Line<'static>,
    selected: bool,
    marker_color: Color,
) -> Line<'static> {
    let marker = if selected { "▌ " } else { "  " };
    let style = if selected {
        Style::default().fg(marker_color)
    } else {
        Style::default()
    };

    line.spans.insert(0, Span::styled(marker, style));
    line
}
