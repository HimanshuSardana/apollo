//! Markdown segmentation for chat rendering.
//!
//! Splits assistant text into prose and fenced code blocks so prose can go
//! through `TextView::markdown` while fences render in the (highlighted,
//! line-numbered) `Editor` component. Mirrors the fence rules of the CLI
//! renderer: ``` or ~~~ fences, up to 3 leading spaces, info string optional.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    pub lang: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Prose(String),
    Code(CodeBlock),
}

fn fence_marker(line: &str) -> Option<(char, String)> {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let fc = trimmed.chars().next()?;
    if fc != '`' && fc != '~' {
        return None;
    }
    let n = trimmed.chars().take_while(|&c| c == fc).count();
    if n < 3 {
        return None;
    }
    let info: String = trimmed[n..].trim().to_string();
    Some((fc, info))
}

/// Language id for the editor from a fence info string (`"rs"` → `"rust"`).
/// Unknown/empty info falls back to plain text.
pub fn editor_lang(info: &str) -> &'static str {
    match info
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "text" | "txt" | "plain" | "output" | "log" | "console" => "text",
        "rs" | "rust" => "rust",
        "py" | "pyi" | "python" | "python3" => "python",
        "js" | "jsx" | "javascript" => "javascript",
        "ts" | "tsx" | "typescript" => "typescript",
        "json" | "jsonc" => "json",
        "sh" | "bash" | "zsh" | "fish" | "shell" => "bash",
        "go" | "golang" => "go",
        "c" | "h" | "cc" | "cpp" | "c++" | "hpp" | "java" => "cpp",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        other if other.is_empty() => "text",
        _ => "text",
    }
}

pub fn segments(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut prose = String::new();
    let mut fence: Option<(char, String, Vec<String>)> = None;

    let flush_prose = |prose: &mut String, out: &mut Vec<Segment>| {
        if !prose.trim().is_empty() {
            out.push(Segment::Prose(std::mem::take(prose)));
        } else {
            prose.clear();
        }
    };

    for line in text.lines() {
        if fence.is_some() {
            let fc = fence.as_ref().map(|(fc, _, _)| *fc).unwrap();
            let trimmed = line.trim();
            let is_close =
                trimmed.len() >= 3 && trimmed.chars().all(|c| c == fc);
            if is_close {
                let (_, lang, buf) = fence.take().unwrap();
                out.push(Segment::Code(CodeBlock {
                    lang: editor_lang(&lang).to_string(),
                    code: buf.join("\n"),
                }));
            } else if let Some((_, _, buf)) = fence.as_mut() {
                buf.push(line.to_string());
            }
            continue;
        }
        if let Some((fc, info)) = fence_marker(line) {
            flush_prose(&mut prose, &mut out);
            fence = Some((fc, info, Vec::new()));
        } else {
            prose.push_str(line);
            prose.push('\n');
        }
    }

    // Unclosed fence: keep the raw text as prose (mirrors CLI behavior).
    if let Some((fc, info, buf)) = fence.take() {
        let fence_char = fc.to_string().repeat(3);
        prose.push_str(&fence_char);
        if !info.is_empty() {
            prose.push(' ');
            prose.push_str(&info);
        }
        prose.push('\n');
        for line in buf {
            prose.push_str(&line);
            prose.push('\n');
        }
    }
    flush_prose(&mut prose, &mut out);

    // Merge adjacent prose (e.g. text around an unclosed fence) so the
    // output never holds two `Prose` segments in a row.
    let mut merged: Vec<Segment> = Vec::with_capacity(out.len());
    for seg in out {
        match (merged.last_mut(), seg) {
            (Some(Segment::Prose(acc)), Segment::Prose(text)) => acc.push_str(&text),
            (_, seg) => merged.push(seg),
        }
    }

    merged
}

/// Cap displayed lines, mirroring the CLI preview limits.
pub fn cap_lines(text: &str, max: usize) -> (String, Option<usize>) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max {
        return (text.to_string(), None);
    }
    let hidden = lines.len() - max;
    (lines[..max].join("\n"), Some(hidden))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_one_prose_segment() {
        assert_eq!(
            segments("hello\nworld\n"),
            vec![Segment::Prose("hello\nworld\n".to_string())]
        );
    }

    #[test]
    fn fence_splits_prose_and_code() {
        let segs = segments("before\n```rust\nlet x = 1;\n```\nafter\n");
        assert_eq!(segs.len(), 3);
        assert!(matches!(segs[0], Segment::Prose(_)));
        assert_eq!(
            segs[1],
            Segment::Code(CodeBlock {
                lang: "rust".to_string(),
                code: "let x = 1;".to_string()
            })
        );
        assert!(matches!(segs[2], Segment::Prose(_)));
    }

    #[test]
    fn unclosed_fence_stays_prose() {
        let segs = segments("hi\n```py\nprint(1)\n");
        assert_eq!(segs.len(), 1);
        assert!(matches!(segs[0], Segment::Prose(_)));
    }

    #[test]
    fn lang_aliases() {
        assert_eq!(editor_lang("rs"), "rust");
        assert_eq!(editor_lang("py"), "python");
        assert_eq!(editor_lang(""), "text");
        assert_eq!(editor_lang("brainfuck"), "text");
    }

    #[test]
    fn cap_lines_reports_hidden() {
        let (head, hidden) = cap_lines("a\nb\nc\nd", 2);
        assert_eq!(head, "a\nb");
        assert_eq!(hidden, Some(2));
    }
}
