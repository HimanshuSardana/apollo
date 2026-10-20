mod client;

use std::io::Write;

use crossterm::{
    cursor::MoveToColumn,
    execute,
    terminal::{Clear, ClearType},
};
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};

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

fn clear_status_line() {
    let mut out = std::io::stdout();
    let _ = execute!(out, MoveToColumn(0), Clear(ClearType::CurrentLine));
}

async fn run() -> Result<(), client::ApiError> {
    let client = Client::new()?;
    print_banner();

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut history: Vec<ChatMessage> = Vec::new();

    loop {
        print!("{read_prompt}", read_prompt = read_prompt());
        std::io::stdout().flush().ok();

        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => return Err(client::ApiError(format!("failed to read input: {e}"))),
        };
        let input = line.trim();
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
