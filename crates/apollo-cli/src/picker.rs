use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, terminal,
};
use owo_colors::OwoColorize;

const MAX_VISIBLE: usize = 12;

pub struct Item {
    pub label: String,
    pub hint: String,
}

impl Item {
    pub fn new(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: hint.into(),
        }
    }
}

fn window(len: usize, cursor: usize) -> (usize, usize) {
    if len <= MAX_VISIBLE {
        return (0, len);
    }
    let half = MAX_VISIBLE / 2;
    let start = cursor.saturating_sub(half).min(len - MAX_VISIBLE);
    (start, start + MAX_VISIBLE)
}

fn move_cursor(len: usize, cursor: usize, key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::Up | KeyCode::Char('k') => Some(cursor.saturating_sub(1)),
        KeyCode::Down | KeyCode::Char('j') => Some((cursor + 1).min(len.saturating_sub(1))),
        KeyCode::Home | KeyCode::Char('g') => Some(0),
        KeyCode::End | KeyCode::Char('G') => Some(len.saturating_sub(1)),
        _ => None,
    }
}

fn rows_drawn(items: usize, cursor: usize) -> u16 {
    let (start, end) = window(items, cursor);
    (end - start) as u16 + 2
}

fn filtered_indices(items: &[Item], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let q = query.to_lowercase();
    (0..items.len())
        .filter(|&i| items[i].label.to_lowercase().contains(&q))
        .collect()
}

fn draw(title: &str, items: &[Item], filtered: &[usize], cursor: usize, current: usize, query: &str, searching: bool) {
    let display_title = if searching {
        format!("{}  /{}", title, query)
    } else if !query.is_empty() {
        format!("{}  (filter: {})", title, query)
    } else {
        title.to_string()
    };
    let (start, end) = window(filtered.len(), cursor);
    let mut out = io::stdout();
    let _ = execute!(out, cursor::MoveToColumn(0), terminal::Clear(terminal::ClearType::CurrentLine));
    let _ = write!(out, "{}\r\n", display_title.dimmed());
    let _ = execute!(out, cursor::MoveToColumn(0), terminal::Clear(terminal::ClearType::CurrentLine));
    let _ = write!(out, "\r\n");
    for idx in start..end {
        let i = filtered[idx];
        let item = &items[i];
        let marker = if idx == cursor { "›" } else { " " };
        let suffix = if i == current { " ●" } else { "" };
        let line = format!("{marker} {}{suffix}", item.label);
        let hinted = if item.hint.is_empty() {
            line
        } else {
            format!("{line}  {}", item.hint.dimmed())
        };
        let _ = execute!(out, cursor::MoveToColumn(0), terminal::Clear(terminal::ClearType::CurrentLine));
        if idx == cursor {
            let _ = write!(out, "{}\r\n", hinted.bold().cyan());
        } else if i == current {
            let _ = write!(out, "{hinted}\r\n");
        } else {
            let _ = write!(out, "{}\r\n", hinted.dimmed());
        }
    }
    let _ = out.flush();
}

fn move_up(rows: u16) {
    if rows > 0 {
        let _ = execute!(io::stdout(), cursor::MoveUp(rows));
    }
}

fn erase_down(rows: u16) {
    let mut out = io::stdout();
    for i in 0..rows {
        let _ = execute!(
            out,
            cursor::MoveToColumn(0),
            terminal::Clear(terminal::ClearType::CurrentLine)
        );
        if i + 1 < rows {
            let _ = execute!(out, cursor::MoveDown(1));
        }
    }
    let _ = out.flush();
}

pub fn pick(title: &str, items: &[Item], current: usize) -> io::Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }
    let mut cursor = current.min(items.len() - 1);
    let mut query = String::new();
    let mut searching = false;
    let mut filtered = filtered_indices(items, &query);

    // ensure cursor within filtered
    if filtered.is_empty() {
        cursor = 0;
    } else {
        cursor = cursor.min(filtered.len() - 1);
    }

    terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    let _ = execute!(out, cursor::Hide);
    draw(title, items, &filtered, cursor, current, &query, searching);
    let mut drawn = rows_drawn(filtered.len(), cursor);

    let mut result = None;
    loop {
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
                {
                    break;
                }
                if searching {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            searching = false;
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            query.pop();
                            filtered = filtered_indices(items, &query);
                            cursor = 0;
                        }
                        KeyCode::Char(c) => {
                            query.push(c);
                            filtered = filtered_indices(items, &query);
                            cursor = 0;
                        }
                        other => {
                            if let Some(next) = move_cursor(filtered.len(), cursor, other) {
                                if next != cursor {
                                    cursor = next;
                                }
                            }
                        }
                    }
                } else {
                    match key.code {
                        KeyCode::Char('/') => {
                            searching = true;
                            query.clear();
                            filtered = filtered_indices(items, &query);
                            cursor = 0;
                        }
                        KeyCode::Enter => {
                            if !filtered.is_empty() {
                                result = Some(filtered[cursor]);
                            }
                            break;
                        }
                        KeyCode::Esc | KeyCode::Char('q') => break,
                        other => {
                            if let Some(next) = move_cursor(filtered.len(), cursor, other) {
                                if next != cursor {
                                    cursor = next;
                                }
                            }
                        }
                    }
                }
                // redraw
                let old_drawn = drawn;
                move_up(old_drawn);
                erase_down(old_drawn);
                if old_drawn > 0 {
                    let _ = execute!(io::stdout(), cursor::MoveUp(old_drawn.saturating_sub(1)));
                }
                let _ = execute!(io::stdout(), cursor::MoveToColumn(0));
                draw(title, items, &filtered, cursor, current, &query, searching);
                drawn = rows_drawn(filtered.len(), cursor);
            }
            Event::Resize(..) => {
                let old_drawn = drawn;
                move_up(old_drawn);
                erase_down(old_drawn);
                if old_drawn > 0 {
                    let _ = execute!(io::stdout(), cursor::MoveUp(old_drawn.saturating_sub(1)));
                }
                let _ = execute!(io::stdout(), cursor::MoveToColumn(0));
                draw(title, items, &filtered, cursor, current, &query, searching);
                drawn = rows_drawn(filtered.len(), cursor);
            }
            _ => {}
        }
    }

    move_up(drawn);
    erase_down(drawn);
    let _ = execute!(out, cursor::Show);
    terminal::disable_raw_mode()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_shows_everything_when_short() {
        assert_eq!(window(3, 0), (0, 3));
        assert_eq!(window(12, 5), (0, 12));
    }

    #[test]
    fn window_follows_cursor() {
        let (start, end) = window(100, 50);
        assert_eq!(end - start, MAX_VISIBLE);
        assert!(start <= 50 && 50 < end);
        assert_eq!(window(100, 0), (0, MAX_VISIBLE));
        let (s, e) = window(100, 99);
        assert_eq!((s, e), (100 - MAX_VISIBLE, 100));
    }

    #[test]
    fn cursor_movement_clamps() {
        assert_eq!(move_cursor(5, 0, KeyCode::Up), Some(0));
        assert_eq!(move_cursor(5, 4, KeyCode::Down), Some(4));
        assert_eq!(move_cursor(5, 2, KeyCode::Down), Some(3));
        assert_eq!(move_cursor(5, 2, KeyCode::Char('k')), Some(1));
        assert_eq!(move_cursor(5, 2, KeyCode::End), Some(4));
        assert_eq!(move_cursor(5, 2, KeyCode::Home), Some(0));
        assert_eq!(move_cursor(5, 2, KeyCode::Char('x')), None);
    }

    #[test]
    fn rows_cover_title_blank_and_items() {
        assert_eq!(rows_drawn(3, 0), 5);
        assert_eq!(rows_drawn(100, 50), MAX_VISIBLE as u16 + 2);
    }

    #[test]
    fn filtered_indices_filters_by_label() {
        let items = vec![Item::new("apple", ""), Item::new("banana", ""), Item::new("apricot", "")];
        let f = filtered_indices(&items, "ap");
        assert_eq!(f, vec![0, 2]);
    }
}