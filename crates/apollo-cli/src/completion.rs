use std::path::PathBuf;

use rustyline::{
    CompletionType, Config, Context, Helper,
    config::Builder,
    completion::{Completer, Pair},
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
};

#[derive(Default)]
pub struct PathCompleter;

impl Highlighter for PathCompleter {}
impl Validator for PathCompleter {}
impl Helper for PathCompleter {}

impl Hinter for PathCompleter {
    type Hint = String;
}

impl Completer for PathCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let before = &line[..pos.min(line.len())];
        let start = before
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let token = &before[start..];

        let (dir_part, prefix) = match token.rfind('/') {
            Some(i) => (&token[..=i], &token[i + 1..]),
            None => ("", token),
        };

        let base = if dir_part.is_empty() {
            PathBuf::from(".")
        } else if dir_part == "~/" {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        } else {
            PathBuf::from(dir_part)
        };

        let mut out: Vec<Pair> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') && !prefix.starts_with('.') {
                    continue;
                }
                if !name.starts_with(prefix) {
                    continue;
                }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let suffix = if is_dir { "/" } else { "" };
                out.push(Pair {
                    display: format!("{name}{suffix}"),
                    replacement: format!("{dir_part}{name}{suffix}"),
                });
            }
        }
        out.sort_by(|a, b| a.display.cmp(&b.display));
        Ok((start, out))
    }
}

pub fn config() -> Config {
    Builder::new()
        .completion_type(CompletionType::List)
        .completion_prompt_limit(80)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::history::DefaultHistory;

    fn complete(line: &str, pos: usize) -> (usize, Vec<String>) {
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, pairs) = PathCompleter.complete(line, pos, &ctx).unwrap();
        (start, pairs.into_iter().map(|p| p.replacement).collect())
    }

    #[test]
    fn completes_top_level_entries() {
        let dir = std::env::temp_dir().join("apollo-complete-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("file.txt"), "x").unwrap();
        std::fs::write(dir.join("other.md"), "x").unwrap();

        let line = format!("{}/f", dir.display());
        let (start, got) = complete(&line, line.len());
        assert_eq!(&line[start..], "f");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].ends_with("file.txt"), "{got:?}");
    }

    #[test]
    fn directories_get_trailing_slash() {
        let dir = std::env::temp_dir().join("apollo-complete-test");
        let line = format!("{}/s", dir.display());
        let (_, got) = complete(&line, line.len());
        assert_eq!(got, vec![format!("{}/sub/", dir.display())]);
    }

    #[test]
    fn token_start_is_after_whitespace() {
        let line = "read src/ma";
        let (start, _) = complete(line, line.len());
        assert_eq!(start, 5);
        assert_eq!(&line[start..], "src/ma");
    }

    #[test]
    fn hidden_entries_skipped_unless_requested() {
        let dir = std::env::temp_dir().join("apollo-complete-test");
        std::fs::write(dir.join(".secret"), "x").unwrap();
        let line = format!("{}/", dir.display());
        let (_, got) = complete(&line, line.len());
        assert!(!got.iter().any(|p| p.contains(".secret")), "{got:?}");
        let line = format!("{}/.", dir.display());
        let (_, got) = complete(&line, line.len());
        assert!(got.iter().any(|p| p.contains(".secret")), "{got:?}");
    }

    #[test]
    fn no_match_returns_empty() {
        let line = "zzzznope";
        let (_, got) = complete(line, line.len());
        assert!(got.is_empty());
    }
}
