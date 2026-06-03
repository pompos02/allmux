use crate::parser::{DockerContainer, SshHost};
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
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::io;

#[derive(Debug, Clone)]
enum Entry {
    Ssh(SshHost),
    Docker(DockerContainer),
}

impl Entry {
    fn list_line(&self, matched_indices: &[usize], selected: bool) -> Line<'static> {
        match self {
            Entry::Ssh(host) => {
                let mut spans = vec![
                    Span::styled(
                        " SSH ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap("  ", selected),
                ];
                spans.extend(highlighted_text(&host.alias, matched_indices, 0, selected));
                spans.push(styled_gap("  ", selected));
                spans.push(Span::styled(
                    host.hostname.clone(),
                    row_style(Color::DarkGray, selected),
                ));
                Line::from(spans)
            }
            Entry::Docker(container) => {
                let status_style = if container.status {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };

                let mut spans = vec![
                    Span::styled(
                        " DOC ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    styled_gap("  ", selected),
                ];
                spans.extend(highlighted_text(
                    &container.name,
                    matched_indices,
                    0,
                    selected,
                ));
                spans.push(styled_gap("  ", selected));
                spans.push(Span::styled(
                    if container.status {
                        "running"
                    } else {
                        "stopped"
                    },
                    selected_style(status_style, selected),
                ));
                Line::from(spans)
            }
        }
    }

    fn list_text(&self) -> String {
        match self {
            Entry::Ssh(host) => host.alias.clone(),
            Entry::Docker(container) => container.name.clone(),
        }
    }

    fn haystack(&self) -> String {
        match self {
            Entry::Ssh(host) => format!(
                "{} {} {} {}",
                self.list_text(),
                host.hostname,
                host.user,
                host.description.as_deref().unwrap_or_default()
            ),
            Entry::Docker(container) => format!(
                "{} {} {} {} {} {} {}",
                self.list_text(),
                container.id,
                container.image,
                container.command,
                container.created_at,
                container.status_text,
                container.ports
            ),
        }
    }

    fn preview(&self) -> Vec<Line<'static>> {
        match self {
            Entry::Ssh(host) => {
                let mut lines = vec![
                    Line::from(Span::styled(
                        "SSH Host",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::default(),
                    field_line("Alias", &host.alias, Color::Yellow),
                    field_line("Hostname", value_or_dash(&host.hostname), Color::Green),
                    field_line("User", value_or_dash(&host.user), Color::Blue),
                    Line::default(),
                    Line::from(Span::styled(
                        "Description",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::default(),
                ];

                let description = host.description.as_deref().unwrap_or("-").trim_end();
                lines.extend(description.lines().map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Gray),
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
                field_line("Command", value_or_dash(&container.command), Color::Gray),
                Line::default(),
                field_line("Created", value_or_dash(&container.created_at), Color::Blue),
                field_line("Ports", value_or_dash(&container.ports), Color::Magenta),
                Line::default(),
                Line::from(vec![
                    Span::styled(
                        "Status",
                        Style::default()
                            .fg(Color::DarkGray)
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
                    Color::Gray,
                ),
            ],
        }
    }
}

struct App {
    entries: Vec<Entry>,
    query: String,
    selected: usize,
}

struct Match {
    index: usize,
    score: i64,
    indices: Vec<usize>,
}

impl App {
    fn new(hosts: Vec<SshHost>, containers: Vec<DockerContainer>) -> Self {
        let mut entries = Vec::with_capacity(hosts.len() + containers.len());
        entries.extend(hosts.into_iter().map(Entry::Ssh));
        entries.extend(containers.into_iter().map(Entry::Docker));

        Self {
            entries,
            query: String::new(),
            selected: 0,
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
                        score: 0,
                        indices: Vec::new(),
                    });
                }

                matcher
                    .fuzzy_indices(&entry.haystack(), &self.query)
                    .map(|(score, indices)| Match {
                        index,
                        score,
                        indices,
                    })
            })
            .collect();

        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
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

    fn move_down(&mut self) {
        let len = self.filtered_matches().len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }
}

pub fn run(hosts: Vec<SshHost>, containers: Vec<DockerContainer>) -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, App::new(hosts, containers));

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if handle_key(key, &mut app) {
                break;
            }
        }
    }

    Ok(())
}

fn handle_key(key: KeyEvent, app: &mut App) -> bool {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
        KeyCode::Char('q') if app.query.is_empty() => return true,
        KeyCode::Esc => return true,
        KeyCode::Up => app.move_up(),
        KeyCode::Down => app.move_down(),
        KeyCode::Backspace => {
            app.query.pop();
            app.clamp_selection();
        }
        KeyCode::Char(c) => {
            app.query.push(c);
            app.selected = 0;
        }
        _ => {}
    }

    false
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(vertical[1]);

    let filtered = app.filtered_matches();
    let search = Paragraph::new(app.query.as_str()).block(
        Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(search, vertical[0]);

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(row, matched)| {
            let entry = &app.entries[matched.index];
            ListItem::new(entry.list_line(&matched.indices, row == app.selected))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Entries ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(Style::default())
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if filtered.is_empty() {
        state.select(None);
    } else {
        app.clamp_selection();
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, body[0], &mut state);

    let preview = filtered
        .get(app.selected)
        .map(|matched| app.entries[matched.index].preview())
        .unwrap_or_else(|| {
            vec![Line::from(Span::styled(
                "No entries match the current search.",
                Style::default().fg(Color::DarkGray),
            ))]
        });

    let preview = Paragraph::new(preview)
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, body[1]);

    let help = Paragraph::new(
        "Type to search | Up/Down to move | Esc/Ctrl-C to quit | q quits when search is empty",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, vertical[2]);
}

fn value_or_dash(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn field_line(label: &'static str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            label,
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(value.to_string(), Style::default().fg(value_color)),
    ])
}

fn highlighted_text(
    text: &str,
    matched_indices: &[usize],
    offset: usize,
    selected: bool,
) -> Vec<Span<'static>> {
    text.char_indices()
        .map(|(index, character)| {
            let style = if matched_indices.contains(&(index + offset)) {
                selected_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    selected,
                )
            } else {
                row_style(Color::White, selected)
            };

            Span::styled(character.to_string(), style)
        })
        .collect()
}

fn styled_gap(text: &'static str, selected: bool) -> Span<'static> {
    Span::styled(text, selected_style(Style::default(), selected))
}

fn row_style(color: Color, selected: bool) -> Style {
    selected_style(Style::default().fg(color), selected)
}

fn selected_style(style: Style, selected: bool) -> Style {
    if selected {
        style.bg(Color::Rgb(38, 38, 38))
    } else {
        style
    }
}
