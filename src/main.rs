mod client;
mod highlight;
mod markdown;

use std::io::Write;

use owo_colors::OwoColorize;
use rustyline::{DefaultEditor, error::ReadlineError};

use client::{ChatMessage, Client};
use markdown::StreamRenderer;

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
            "/clear" => {
                history.clear();
                println!("{}", "context cleared".dimmed());
                continue;
            }
            _ => {}
        }

        history.push(ChatMessage::user(input));

        let mut thought_open = false;
        let mut content_started = false;
        let mut thought_at_line_start = true;
        let mut renderer = StreamRenderer::new();

        let result = client
            .complete_stream(&history, |ev| {
                match ev {
                    client::StreamEvent::Reasoning(t) => {
                        thought_open = true;
                        thought_at_line_start = t.ends_with('\n');
                        print!("{}", t.dimmed());
                        let _ = std::io::stdout().flush();
                    }
                    client::StreamEvent::Content(t) => {
                        if !content_started {
                            content_started = true;
                            if thought_open && !thought_at_line_start {
                                println!();
                            }
                        }
                        print!("{}", renderer.push(&t));
                        let _ = std::io::stdout().flush();
                    }
                }
            })
            .await;

        match result {
            Ok((content, reasoning)) => {
                let empty = content.trim().is_empty();
                history.push(ChatMessage::assistant(content, reasoning));
                print!("{}", renderer.flush());
                println!();
                if empty {
                    println!("{}", "(empty reply — model returned only reasoning)".yellow());
                }
                println!();
            }
            Err(e) => {
                println!();
                eprintln!("{} {}", "error:".bold().red(), e.to_string().red());
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
