use crate::tui::bridge::{TuiCommand, TuiState};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub struct TuiApp {
    pub scroll: u16,
    pub selected_player_idx: usize,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            selected_player_idx: 0,
        }
    }

    pub fn draw(frame: &mut Frame, state: &TuiState, app: &Self) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)])
            .split(frame.area());

        let up_secs = state.start_time.elapsed().as_secs();
        let uptime = format!("{:02}:{:02}:{:02}", up_secs / 3600, up_secs / 60 % 60, up_secs % 60);
        let top_text = format!(
            " Ascendustry  |  up {}  |  {} connectés  |  seed: {}  |  chunks: {}  modifs: {}",
            uptime, state.connected_player_count, state.seed, state.chunk_count, state.modified_count,
        );
        let top = Paragraph::new(top_text).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        frame.render_widget(top, chunks[0]);

        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[1]);

        let player_items: Vec<ListItem> = state
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let prefix = if i == app.selected_player_idx { "► " } else { "  " };
                ListItem::new(format!("{}{}", prefix, p.username))
            })
            .collect();
        let player_list = List::new(player_items).block(Block::default().borders(Borders::ALL).title("Joueurs"));
        frame.render_widget(player_list, body_chunks[0]);

        let log_lines: Vec<Line> = state.logs.iter().take(50).map(|l| Line::from(Span::raw(l))).collect();
        let log_widget = Paragraph::new(log_lines)
            .block(Block::default().borders(Borders::ALL).title("Logs"))
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0));
        frame.render_widget(log_widget, body_chunks[1]);

        let help = Paragraph::new(" Ctrl+S: Save  Q: Quit  ↑↓: Select player  k: Kick selected ")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[2]);
    }

    pub fn handle_input(key: KeyEvent, app: &mut Self, player_ids: &[u64]) -> Option<TuiCommand> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(TuiCommand::Shutdown),
            KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => Some(TuiCommand::Save),
            KeyCode::Char('k') | KeyCode::Char('K') => {
                let idx = app.selected_player_idx;
                if idx < player_ids.len() {
                    Some(TuiCommand::Kick(player_ids[idx]))
                } else {
                    None
                }
            }
            KeyCode::Up => {
                app.selected_player_idx = app.selected_player_idx.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                app.selected_player_idx = app.selected_player_idx.saturating_add(1);
                None
            }
            _ => None,
        }
    }
}
