mod client;

use std::io::IsTerminal;

use crossterm::{
    cursor::{MoveDown, MoveToColumn, MoveUp},
    execute,
    terminal::{Clear, ClearType},
};
use owo_colors::OwoColorize;
use rustyline::{DefaultEditor, error::ReadlineError};

use client::{ChatMessage, Client};

fn print_banner() {
    println!(
        "{} {}",
        "apollo".bold().cyan(),
        format!("— {} via opencode-go", client::MODEL).dimmed()
    );
    println!(
        "  {}",
        "type a message · /clear resets context · /exit quits".dimmed()
    );
    println!();
}

fn read_prompt() -> String {
    format!("{} ", "›".bold().cyan())
}

fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(60)
        .max(20)
}

fn print_bar() {
    println!("{}", "─".repeat(term_width()).dimmed());
}

fn step_below_bar() {
    let mut out = std::io::stdout();
    let _ = execute!(out, MoveToColumn(0));
    println!();
}

fn drop_below_box() {
    let mut out = std::io::stdout();
    let _ = execute!(out, MoveDown(1), MoveToColumn(0));
    println!();
}

fn clear_status_line() {
    let mut out = std::io::stdout();
    let _ = execute!(out, MoveToColumn(0), Clear(ClearType::CurrentLine));
}

async fn run() -> Result<(), client::ApiError> {
    let client = Client::new()?;
    print_banner();

    let mut history: Vec<ChatMessage> = Vec::new();

    let history_path = std::env::var("HOME").map(|h| format!("{h}/.apollo_history")).ok();
    let mut editor = DefaultEditor::new()
        .map_err(|e| client::ApiError(format!("failed to init input: {e}")))?;
    if let Some(p) = &history_path {
        let _ = editor.load_history(p);
    }

    let interactive = std::io::stdout().is_terminal();
    loop {
        print_bar();
        if interactive {
            println!();
            print_bar();
            let mut out = std::io::stdout();
            let _ = execute!(out, MoveUp(2), MoveToColumn(0));
        }
        let line = tokio::task::block_in_place(|| editor.readline(&read_prompt()));
        let line = match line {
            Ok(l) => {
                if interactive {
                    step_below_bar();
                } else {
                    print_bar();
                }
                l
            }
            Err(ReadlineError::Interrupted) => {
                if interactive {
                    drop_below_box();
                } else {
                    print_bar();
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                if interactive {
                    drop_below_box();
                }
                break;
            }
            Err(e) => {
                if interactive {
                    drop_below_box();
                }
                return Err(client::ApiError(format!("failed to read input: {e}")));
            }
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
            "/clear" => {
                history.clear();
                println!("  {}", "context cleared".dimmed());
                continue;
            }
            _ => {}
        }

        history.push(ChatMessage::user(input));

        println!("  {}", "thinking…".dimmed().italic());

        match client.complete(&history).await {
            Ok((content, reasoning)) => {
                history.push(ChatMessage::assistant(
                    content.clone(),
                    reasoning.clone(),
                ));
                clear_status_line();
                println!("{} {}", "●".magenta(), "apollo".bold().magenta());
                if let Some(r) = reasoning {
                    println!("  {}", "thought:".dimmed());
                    for rl in r.lines() {
                        println!("  {}", rl.dimmed());
                    }
                }
                if content.trim().is_empty() {
                    println!("  {}", "(empty reply — model returned only reasoning)".yellow());
                } else {
                    for cl in content.lines() {
                        println!("  {cl}");
                    }
                }
                println!();
            }
            Err(e) => {
                clear_status_line();
                eprintln!("  {} {}", "error:".bold().red(), e.to_string().red());
                println!();
            }
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
