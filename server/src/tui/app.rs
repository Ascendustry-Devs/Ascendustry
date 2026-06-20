use crate::tui::bridge::{TuiCommand, TuiState};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

fn health_color(score: u8) -> Color {
    if score >= 15 {
        Color::Green
    } else if score >= 10 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn health_bar(score: u8) -> String {
    let filled = (score as f64 / 20.0 * 10.0) as usize;
    let empty = 10usize.saturating_sub(filled);
    format!("{}{} {}/20", "█".repeat(filled), "░".repeat(empty), score)
}

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
            " Ascendustry  |  up {}  |  {}  |  {} connectés  |  seed: {}  |  chunks: {}  modifs: {}",
            uptime,
            health_bar(state.health_score),
            state.connected_player_count,
            state.seed,
            state.chunk_count,
            state.modified_count,
        );
        let top_color = health_color(state.health_score);
        let top = Paragraph::new(top_text).style(Style::default().fg(top_color).add_modifier(Modifier::BOLD));
        frame.render_widget(top, chunks[0]);

        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(18),
                Constraint::Percentage(57),
            ])
            .split(chunks[1]);

        // --- Player list ---
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

        // --- Health panel ---
        let health_lines = vec![
            Line::from(Span::styled(
                health_bar(state.health_score),
                Style::default()
                    .fg(health_color(state.health_score))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Paquets:  {:.1}/s", state.packets_per_second)),
            Line::from(format!("Charge:   {:.1}%", state.guard_cycle_load_pct)),
            Line::from(format!("CG avg:   {:.2}ms", state.guard_cycle_avg_ms)),
            Line::from(format!("Rejetés:  {}", state.packets_rejected)),
            Line::from(format!(
                "Conn.:    {} ({} total)",
                state.connected_player_count, state.total_connections
            )),
        ];
        let health_widget = Paragraph::new(health_lines).block(Block::default().borders(Borders::ALL).title("Santé"));
        frame.render_widget(health_widget, body_chunks[1]);

        // --- Logs ---
        let log_lines: Vec<Line> = state.logs.iter().take(50).map(|l| Line::from(Span::raw(l))).collect();
        let log_widget = Paragraph::new(log_lines)
            .block(Block::default().borders(Borders::ALL).title("Logs"))
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0));
        frame.render_widget(log_widget, body_chunks[2]);

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
