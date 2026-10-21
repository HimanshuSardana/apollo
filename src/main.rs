mod client;
mod completion;
mod highlight;
mod markdown;
mod models;
mod picker;
mod tools;

use std::io::Write;

use owo_colors::OwoColorize;
use rustyline::{Editor, error::ReadlineError, history::DefaultHistory};
use unicode_width::UnicodeWidthStr;

use client::{ChatMessage, Client};
use completion::PathCompleter;
use highlight::Lang;
use markdown::StreamRenderer;
use tools::Display;

const MAX_TOOL_STEPS: usize = 24;
const BASH_PREVIEW_LINES: usize = 40;
const DIFF_DEL_BG: u8 = 52;
const DIFF_ADD_BG: u8 = 22;

fn paint_bg(text: &str, code: u8) -> String {
    format!("\x1b[48;5;{code}m{text}\x1b[49m")
}

fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(100)
        .max(40)
}

fn fit(text: &str, width: usize) -> String {
    let used = text.width();
    if used <= width {
        return format!("{text}{}", " ".repeat(width - used));
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in text.chars() {
        let cw = ch.to_string().width();
        if w + cw > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    format!("{out}{}", " ".repeat(width.saturating_sub(out.width())))
}

fn render_code(path: &str, content: &str, note: &str) {
    if !note.is_empty() {
        println!("  {}", note.dimmed());
    }
    let total = content.lines().count().max(1);
    let num_w = total.to_string().len().max(2);
    let mut hl = highlight::State::new(Lang::from_path(path));
    for (i, line) in content.lines().enumerate() {
        let gutter = format!("{:>num_w$} │", i + 1, num_w = num_w);
        println!("{} {}", gutter.dimmed(), hl.highlight(line));
    }
}

fn diff_rows<'a>(
    old: &[&'a str],
    new: &[&'a str],
) -> Vec<(char, Option<&'a str>, Option<&'a str>)> {
    let n = old.len();
    let m = new.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old[i] == new[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0usize, 0usize);
    let mut rows = Vec::new();
    while i < n && j < m {
        if old[i] == new[j] {
            rows.push((' ', Some(old[i]), Some(new[j])));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            rows.push(('-', Some(old[i]), None));
            i += 1;
        } else {
            rows.push(('+', None, Some(new[j])));
            j += 1;
        }
    }
    while i < n {
        rows.push(('-', Some(old[i]), None));
        i += 1;
    }
    while j < m {
        rows.push(('+', None, Some(new[j])));
        j += 1;
    }

    let mut paired = Vec::new();
    let mut k = 0;
    while k < rows.len() {
        if rows[k].0 == '-'
            && k + 1 < rows.len()
            && rows[k + 1].0 == '+'
        {
            paired.push(('~', rows[k].1, rows[k + 1].2));
            k += 2;
        } else {
            paired.push(rows[k]);
            k += 1;
        }
    }
    paired
}

#[derive(Clone, Copy)]
enum DiffKind {
    Del,
    Add,
    Ctx,
}

fn diff_row(
    old: Option<usize>,
    new: Option<usize>,
    sign: char,
    content: &str,
    width: usize,
    num_w: usize,
    kind: DiffKind,
) {
    let blank = " ".repeat(num_w);
    let onum = old.map(|n| format!("{n:>num_w$}")).unwrap_or_else(|| blank.clone());
    let nnum = new.map(|n| format!("{n:>num_w$}")).unwrap_or_else(|| blank.clone());
    let prefix = format!("{onum} {nnum} {sign} ");
    let avail = width.saturating_sub(prefix.width()).max(8);
    let row = format!("{prefix}{}", fit(content, avail));
    match kind {
        DiffKind::Del => println!("{}", paint_bg(&row, DIFF_DEL_BG)),
        DiffKind::Add => println!("{}", paint_bg(&row, DIFF_ADD_BG)),
        DiffKind::Ctx => println!("{}", row.dimmed()),
    }
}

fn render_diff(old: &str, new: &str, start_line: usize) {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let rows = diff_rows(&a, &b);
    let removed = rows.iter().filter(|r| matches!(r.0, '-' | '~')).count();
    let added = rows.iter().filter(|r| matches!(r.0, '+' | '~')).count();
    println!("  {}", format!("+{added} −{removed}").dimmed());

    let width = term_width();
    let num_w = (start_line + a.len().max(b.len()))
        .to_string()
        .len()
        .max(2);
    let mut old_no = start_line;
    let mut new_no = start_line;

    for (op, left, right) in rows {
        match op {
            '-' => {
                diff_row(
                    Some(old_no),
                    None,
                    '-',
                    left.unwrap_or(""),
                    width,
                    num_w,
                    DiffKind::Del,
                );
                old_no += 1;
            }
            '+' => {
                diff_row(
                    None,
                    Some(new_no),
                    '+',
                    right.unwrap_or(""),
                    width,
                    num_w,
                    DiffKind::Add,
                );
                new_no += 1;
            }
            '~' => {
                diff_row(
                    Some(old_no),
                    None,
                    '-',
                    left.unwrap_or(""),
                    width,
                    num_w,
                    DiffKind::Del,
                );
                diff_row(
                    None,
                    Some(new_no),
                    '+',
                    right.unwrap_or(""),
                    width,
                    num_w,
                    DiffKind::Add,
                );
                old_no += 1;
                new_no += 1;
            }
            _ => {
                diff_row(
                    Some(old_no),
                    Some(new_no),
                    ' ',
                    left.unwrap_or(""),
                    width,
                    num_w,
                    DiffKind::Ctx,
                );
                old_no += 1;
                new_no += 1;
            }
        }
    }
}

fn render_text(body: &str) {
    let is_error = body.starts_with("error:");
    let mut lines = body.lines();
    let mut shown = 0usize;
    for line in lines.by_ref() {
        if shown >= BASH_PREVIEW_LINES {
            break;
        }
        let rendered = format!("  {line}");
        if is_error {
            println!("{}", rendered.red().dimmed());
        } else {
            println!("{}", rendered.dimmed());
        }
        shown += 1;
    }
    let rest = lines.count();
    if rest > 0 {
        println!("  {}", format!("… {rest} more lines").dimmed());
    }
}

fn render_tool_result(display: &Display) {
    match display {
        Display::Code {
            path,
            content,
            note,
        } => render_code(path, content, note),
        Display::Diff { old, new, start_line } => render_diff(old, new, *start_line),
        Display::Text { body } => render_text(body),
    }
}

fn print_banner(client: &Client) {
    let effort = client.effort().unwrap_or("auto");
    println!(
        "{} {}",
        "apollo".bold().cyan(),
        format!(
            "— {} [{effort}] via opencode-go",
            models::display_name(client.model())
        )
        .dimmed()
    );
    println!(
        "  {}",
        "type a message · /models /effort /clear /exit".dimmed()
    );
    println!();
}

fn pick_model(client: &mut Client) {
    let items: Vec<picker::Item> = models::CATALOG
        .iter()
        .map(|m| picker::Item::new(m.name, m.id))
        .collect();
    let current = models::index_of(client.model()).unwrap_or(0);
    match picker::pick("model", &items, current) {
        Ok(Some(i)) => {
            let picked = &models::CATALOG[i];
            client.set_model(picked.id);
            let reset = match client.effort() {
                Some(e) if !picked.efforts.contains(&e) => {
                    client.set_effort(None);
                    true
                }
                _ => false,
            };
            if reset {
                println!(
                    "{}",
                    format!("model → {} · effort reset to auto", picked.name).dimmed()
                );
            } else {
                println!(
                    "{}",
                    format!("model → {} ({})", picked.name, picked.id).dimmed()
                );
            }
        }
        Ok(None) => {}
        Err(e) => println!("{}", format!("picker: {e}").dimmed()),
    }
}

fn effort_hint(level: &str) -> &'static str {
    match level {
        "auto" => "model default",
        "off" => "no reasoning",
        "low" => "fast",
        "medium" => "balanced",
        "high" => "deep",
        "max" => "deepest",
        _ => "",
    }
}

fn pick_effort(client: &mut Client) {
    let levels = models::efforts_for(client.model());
    let mut items = vec![picker::Item::new("auto", effort_hint("auto"))];
    for level in levels {
        items.push(picker::Item::new(*level, effort_hint(level)));
    }
    let current = match client.effort() {
        None => 0,
        Some(e) => levels
            .iter()
            .position(|l| l == &e)
            .map(|p| p + 1)
            .unwrap_or(0),
    };
    match picker::pick("effort", &items, current) {
        Ok(Some(0)) => {
            client.set_effort(None);
            println!("{}", "effort → auto".dimmed());
        }
        Ok(Some(i)) => {
            let level = items[i].label.clone();
            client.set_effort(Some(&level));
            println!("{}", format!("effort → {level}").dimmed());
        }
        Ok(None) => {}
        Err(e) => println!("{}", format!("picker: {e}").dimmed()),
    }
}

fn read_prompt() -> String {
    format!("{} ", "›".bold().cyan())
}

async fn run() -> Result<(), client::ApiError> {
    let mut client = Client::new()?;
    let tool_defs = tools::definitions();
    print_banner(&client);

    let mut history: Vec<ChatMessage> = Vec::new();

    let history_path = std::env::var("HOME").map(|h| format!("{h}/.apollo_history")).ok();
    let mut editor: Editor<PathCompleter, DefaultHistory> =
        Editor::with_default(completion::config())
            .map_err(|e| client::ApiError(format!("failed to init input: {e}")))?;
    if let Some(p) = &history_path {
        let _ = editor.load_history(p);
    }

    loop {
        let line = tokio::task::block_in_place(|| editor.readline(&read_prompt()));
        let line = match line {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => return Err(client::ApiError(format!("failed to read input: {e}"))),
        };
        let input = line.trim();
        if !input.is_empty() {
            let _ = editor.add_history_entry(input);
        }
        if input.is_empty() {
            continue;
        }
        match input {
            "/exit" | "/quit" | "exit" | "quit" => break,
            "/models" | "/model" => {
                tokio::task::block_in_place(|| pick_model(&mut client));
                continue;
            }
            "/effort" => {
                tokio::task::block_in_place(|| pick_effort(&mut client));
                continue;
            }
            "/clear" => {
                history.clear();
                println!("{}", "context cleared".dimmed());
                continue;
            }
            _ => {}
        }

        history.push(ChatMessage::user(input));

        for _ in 0..MAX_TOOL_STEPS {
            let mut thought_open = false;
            let mut content_started = false;
            let mut thought_at_line_start = true;
            let mut renderer = StreamRenderer::new();
            let spinning = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let spin_flag = spinning.clone();
            let spinner = std::thread::spawn(move || {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let mut i = 0usize;
                while spin_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    print!("\r{} Generating…", frames[i % frames.len()]);
                    let _ = std::io::stdout().flush();
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    i += 1;
                }
            });
            let mut spinner_shown = true;

            let result = client
                .complete_stream(&history, Some(&tool_defs), |ev| match ev {
                    client::StreamEvent::Reasoning(t) => {
                        if spinner_shown {
                            spinning.store(false, std::sync::atomic::Ordering::SeqCst);
                            print!("\r\x1b[2K");
                            spinner_shown = false;
                        }
                        thought_open = true;
                        thought_at_line_start = t.ends_with('\n');
                        print!("{}", t.dimmed());
                        let _ = std::io::stdout().flush();
                    }
                    client::StreamEvent::Content(t) => {
                        if spinner_shown {
                            spinning.store(false, std::sync::atomic::Ordering::SeqCst);
                            print!("\r\x1b[2K");
                            spinner_shown = false;
                        }
                        if !content_started {
                            content_started = true;
                            if thought_open && !thought_at_line_start {
                                println!();
                            }
                        }
                        print!("{}", renderer.push(t));
                        let _ = std::io::stdout().flush();
                    }
                })
                .await;

            spinning.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = spinner.join();
            if spinner_shown {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
            }

            let completion = match result {
                Ok(c) => c,
                Err(e) => {
                    println!();
                    eprintln!("{} {}", "error:".bold().red(), e.to_string().red());
                    println!();
                    break;
                }
            };

            print!("{}", renderer.flush());
            println!();
            if completion.content.trim().is_empty()
                && completion.tool_calls.is_empty()
                && completion.reasoning.is_some()
            {
                println!(
                    "{}",
                    "(empty reply — model returned only reasoning)".yellow()
                );
            }

            if completion.tool_calls.is_empty() {
                history.push(ChatMessage::assistant(
                    completion.content,
                    completion.reasoning,
                ));
                println!();
                break;
            }

            let calls = completion.tool_calls.clone();
            history.push(ChatMessage::assistant_tool_calls(
                completion.content,
                completion.reasoning,
                calls.clone(),
            ));

            for call in &calls {
                let head = format!(
                    "→ {} {}",
                    call.function.name,
                    tools::summary(&call.function.name, &call.function.arguments)
                );
                println!("{}", head.dimmed());
                let output = tools::execute(&call.function.name, &call.function.arguments);
                render_tool_result(&output.display);
                history.push(ChatMessage::tool_result(&call.id, output.model_text));
            }
            println!();
        }
    }

    if let Some(p) = &history_path {
        let _ = editor.save_history(p);
    }
    println!("{}", "bye".dimmed());
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{} {}", "apollo:".bold().red(), e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(s: &str) -> String {
        let mut o = String::new();
        let mut it = s.chars().peekable();
        while let Some(c) = it.next() {
            if c == '\x1b' {
                for c2 in it.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                o.push(c);
            }
        }
        o
    }

    #[test]
    fn diff_pairs_changed_lines() {
        let rows = diff_rows(&["a", "b", "c"], &["a", "B", "c"]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, ' ');
        assert_eq!(rows[1].0, '~');
        assert_eq!(rows[1].1, Some("b"));
        assert_eq!(rows[1].2, Some("B"));
    }

    #[test]
    fn diff_pure_insert_and_delete() {
        let ins = diff_rows(&["a"], &["a", "b"]);
        assert_eq!(ins.len(), 2);
        assert_eq!(ins[1].0, '+');
        let del = diff_rows(&["a", "b"], &["a"]);
        assert_eq!(del.len(), 2);
        assert_eq!(del[1].0, '-');
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("ab", 4), "ab  ");
        let cut = fit("abcdefgh", 4);
        assert_eq!(cut.width(), 4);
        assert!(cut.ends_with('…'));
    }

    #[test]
    #[ignore]
    fn preview_render() {
        render_code(
            "/tmp/fibonacci.py",
            "def fib(n):\n    return [0, 1][:n]  # hi\n",
            "3 lines",
        );
        println!();
        render_diff("count = 1\nprint(count)", "count = 2\nprint(count)", 12);
    }
}
