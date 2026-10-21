use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const MAX_READ_LINES: usize = 2000;
const MAX_READ_BYTES: usize = 50 * 1024;
const MAX_OUTPUT_BYTES: usize = 30 * 1024;
const DEFAULT_BASH_TIMEOUT: u64 = 120;

/// Every tool apollo-core can compile in. Used by the wizard to list choices.
pub const ALL_TOOLS: &[&str] = &["read", "write", "edit", "bash"];

/// Is `name` compiled into this binary? (feature `tool-<name>` enabled)
pub fn is_enabled(name: &str) -> bool {
    match name {
        #[cfg(feature = "tool-read")]
        "read" => true,
        #[cfg(feature = "tool-write")]
        "write" => true,
        #[cfg(feature = "tool-edit")]
        "edit" => true,
        #[cfg(feature = "tool-bash")]
        "bash" => true,
        _ => false,
    }
}

fn disabled(name: &str) -> Outcome {
    Outcome::error(format!(
        "error: tool `{name}` is not enabled in this build (rebuild with --features tool-{name})"
    ))
}

#[derive(Debug, Clone)]
pub enum Display {
    Code {
        path: String,
        content: String,
        note: String,
    },
    Diff {
        old: String,
        new: String,
        start_line: usize,
    },
    Text {
        body: String,
    },
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub model_text: String,
    pub display: Display,
}

impl Outcome {
    fn error(msg: String) -> Self {
        Self {
            model_text: msg.clone(),
            display: Display::Text { body: msg },
        }
    }
}

pub fn definitions() -> Value {
    let mut defs: Vec<Value> = Vec::new();
    #[cfg(feature = "tool-read")]
    defs.push(json!({
            "type": "function",
            "function": {
                "name": "read",
                "description": "Read a text file from disk. Returns the contents with line numbers.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Path to the file (relative to the working directory, or absolute)."},
                        "offset": {"type": "integer", "description": "1-based line number to start reading from."},
                        "limit": {"type": "integer", "description": "Maximum number of lines to return."}
                    },
                    "required": ["path"]
                }
            }
        }));
    #[cfg(feature = "tool-write")]
    defs.push(json!({
            "type": "function",
            "function": {
                "name": "write",
                "description": "Create or overwrite a file with the given content. Parent directories are created as needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Path of the file to write."},
                        "content": {"type": "string", "description": "Full content to write to the file."}
                    },
                    "required": ["path", "content"]
                }
            }
        }));
    #[cfg(feature = "tool-edit")]
    defs.push(json!({
            "type": "function",
            "function": {
                "name": "edit",
                "description": "Replace an exact string in a file. old_string must match exactly, including whitespace and indentation.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Path of the file to edit."},
                        "old_string": {"type": "string", "description": "Exact text to replace."},
                        "new_string": {"type": "string", "description": "Replacement text."},
                        "replace_all": {"type": "boolean", "description": "Replace every occurrence instead of requiring a unique match."}
                    },
                    "required": ["path", "old_string", "new_string"]
                }
            }
        }));
    #[cfg(feature = "tool-bash")]
    defs.push(json!({
            "type": "function",
            "function": {
                "name": "bash",
                "description": "Run a shell command with bash and return its combined output and exit code.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "Shell command to execute."},
                        "workdir": {"type": "string", "description": "Directory to run the command in."},
                        "timeout_secs": {"type": "integer", "description": "Timeout in seconds (default 120, max 600)."}
                    },
                    "required": ["command"]
                }
            }
        }));
    Value::Array(defs)
}

pub fn execute(name: &str, raw_args: &str) -> Outcome {
    let trimmed = raw_args.trim();
    let args: Value = if trimmed.is_empty() {
        json!({})
    } else {
        match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => return Outcome::error(format!("error: invalid JSON arguments: {e}")),
        }
    };
    match name {
        "read" => {
            #[cfg(feature = "tool-read")]
            return read(&args);
            #[allow(unreachable_code)]
            disabled("read")
        }
        "write" => {
            #[cfg(feature = "tool-write")]
            return write(&args);
            #[allow(unreachable_code)]
            disabled("write")
        }
        "edit" => {
            #[cfg(feature = "tool-edit")]
            return edit(&args);
            #[allow(unreachable_code)]
            disabled("edit")
        }
        "bash" => {
            #[cfg(feature = "tool-bash")]
            return bash(&args);
            #[allow(unreachable_code)]
            disabled("bash")
        }
        other => Outcome::error(format!("error: unknown tool `{other}`")),
    }
}

pub fn summary(name: &str, raw_args: &str) -> String {
    let args: Value = serde_json::from_str(raw_args).unwrap_or(Value::Null);
    let pick = match name {
        "read" | "write" | "edit" => args.get("path").and_then(Value::as_str),
        "bash" => args.get("command").and_then(Value::as_str),
        _ => None,
    };
    let text = pick.unwrap_or(raw_args).trim().replace('\n', " ");
    let max = 70;
    if text.chars().count() > max {
        let short: String = text.chars().take(max).collect();
        format!("{short}…")
    } else {
        text
    }
}

fn get_str(args: &Value, key: &str) -> Result<String, String> {
    match args.get(key).and_then(Value::as_str) {
        Some(v) => Ok(v.to_string()),
        None => Err(format!("error: missing required string argument `{key}`")),
    }
}

fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n… truncated ({} of {} bytes shown)",
        &s[..end],
        end,
        s.len()
    )
}

#[allow(dead_code)]
fn read(args: &Value) -> Outcome {
    let path = match get_str(args, "path") {
        Ok(p) => p,
        Err(e) => return Outcome::error(e),
    };
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => return Outcome::error(format!("error: cannot read {path}: {e}")),
    };
    let text = match String::from_utf8(data) {
        Ok(t) => t,
        Err(_) => return Outcome::error(format!("error: {path} is not valid UTF-8 text")),
    };
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let start = args
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(MAX_READ_LINES);

    if total == 0 {
        return Outcome {
            model_text: format!("(empty file: {path})"),
            display: Display::Text {
                body: format!("{path} is empty"),
            },
        };
    }
    if start > total {
        return Outcome::error(format!(
            "error: offset {start} is past end of file ({total} lines)"
        ));
    }

    let mut numbered = String::new();
    let mut shown = String::new();
    let mut emitted = 0usize;
    let mut bytes = 0usize;
    let mut truncated = None;
    for (i, line) in lines.iter().enumerate().skip(start - 1) {
        if emitted >= limit {
            truncated = Some(format!(
                "… stopped at line limit ({limit}); file has {total} lines"
            ));
            break;
        }
        let rendered = format!("{:>6}\t{}\n", i + 1, line);
        if bytes + rendered.len() > MAX_READ_BYTES {
            truncated = Some(format!(
                "… stopped at {} of {total} lines (byte limit)",
                i + 1
            ));
            break;
        }
        bytes += rendered.len();
        numbered.push_str(&rendered);
        shown.push_str(line);
        shown.push('\n');
        emitted += 1;
    }
    if let Some(note) = &truncated {
        numbered.push_str(&format!("[{note}]\n"));
    }

    let plural = if emitted == 1 { "line" } else { "lines" };
    let note = match truncated {
        Some(t) => format!(
            "{emitted} of {total} {plural} · {}",
            t.trim_start_matches('…')
        ),
        None => format!("{emitted} {plural}"),
    };
    Outcome {
        model_text: numbered,
        display: Display::Code {
            path,
            content: shown,
            note,
        },
    }
}

#[allow(dead_code)]
fn write(args: &Value) -> Outcome {
    let path = match get_str(args, "path") {
        Ok(p) => p,
        Err(e) => return Outcome::error(e),
    };
    let content = match get_str(args, "content") {
        Ok(c) => c,
        Err(e) => return Outcome::error(e),
    };
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Outcome::error(format!("error: cannot create {}: {e}", parent.display()));
            }
        }
    }
    let lines = content.lines().count();
    match std::fs::write(&path, &content) {
        Ok(()) => {
            let plural = if lines == 1 { "line" } else { "lines" };
            Outcome {
                model_text: format!("wrote {} bytes to {path} ({lines} {plural})", content.len()),
                display: Display::Code {
                    path,
                    content,
                    note: String::new(),
                },
            }
        }
        Err(e) => Outcome::error(format!("error: cannot write {path}: {e}")),
    }
}

#[allow(dead_code)]
fn edit(args: &Value) -> Outcome {
    let path = match get_str(args, "path") {
        Ok(p) => p,
        Err(e) => return Outcome::error(e),
    };
    let old = match get_str(args, "old_string") {
        Ok(v) => v,
        Err(e) => return Outcome::error(e),
    };
    let new = match get_str(args, "new_string") {
        Ok(v) => v,
        Err(e) => return Outcome::error(e),
    };
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if old.is_empty() {
        return Outcome::error("error: old_string must not be empty".to_string());
    }
    let original = match std::fs::read_to_string(&path) {
        Ok(o) => o,
        Err(e) => return Outcome::error(format!("error: cannot read {path}: {e}")),
    };
    let count = original.matches(&old).count();
    if count == 0 {
        return Outcome::error(format!("error: old_string not found in {path}"));
    }
    if count > 1 && !replace_all {
        return Outcome::error(format!(
            "error: old_string appears {count} times in {path}; add more context or set replace_all=true"
        ));
    }
    let start_line = original
        .find(&old)
        .map(|pos| original[..pos].matches('\n').count() + 1)
        .unwrap_or(1);
    let updated = if replace_all {
        original.replace(&old, &new)
    } else {
        original.replacen(&old, &new, 1)
    };
    if let Err(e) = std::fs::write(&path, updated) {
        return Outcome::error(format!("error: cannot write {path}: {e}"));
    }
    let n = if replace_all { count } else { 1 };
    Outcome {
        model_text: format!(
            "edited {path} ({n} replacement{})",
            if n == 1 { "" } else { "s" }
        ),
        display: Display::Diff {
            old,
            new,
            start_line,
        },
    }
}

#[allow(dead_code)]
fn bash(args: &Value) -> Outcome {
    let command = match get_str(args, "command") {
        Ok(c) => c,
        Err(e) => return Outcome::error(e),
    };
    let timeout = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_BASH_TIMEOUT)
        .clamp(1, 600);

    let mut cmd = Command::new("bash");
    cmd.arg("-lc")
        .arg(&command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(wd) = args.get("workdir").and_then(Value::as_str) {
        cmd.current_dir(wd);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Outcome::error(format!("error: failed to spawn bash: {e}")),
    };

    let (tx, rx) = mpsc::channel();
    if let Some(mut out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = out.read_to_end(&mut buf);
            let _ = tx.send(("stdout", buf));
        });
    }
    if let Some(mut err) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = err.read_to_end(&mut buf);
            let _ = tx.send(("stderr", buf));
        });
    }
    drop(tx);

    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait().ok();
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => return Outcome::error(format!("error: waiting for command: {e}")),
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    for _ in 0..2 {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok((tag, buf)) => {
                let text = String::from_utf8_lossy(&buf).to_string();
                if tag == "stdout" {
                    stdout = text;
                } else {
                    stderr = text;
                }
            }
            Err(_) => break,
        }
    }

    let mut body = String::new();
    if !stdout.trim().is_empty() {
        body.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str("[stderr]\n");
        body.push_str(stderr.trim_end());
    }
    if body.is_empty() {
        body.push_str("(no output)");
    }
    if timed_out {
        body.push_str(&format!("\n[timed out after {timeout}s]"));
    }

    let head = if timed_out {
        "timed out".to_string()
    } else {
        match status.and_then(|s| s.code()) {
            Some(0) => "exit 0".to_string(),
            Some(c) => format!("exit {c}"),
            None => "terminated by signal".to_string(),
        }
    };
    let output = truncate_bytes(&body, MAX_OUTPUT_BYTES);
    Outcome {
        model_text: format!("{head}\n{output}"),
        display: Display::Text { body: output },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        let dir = std::env::temp_dir().join("apollo-tool-test");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name).to_string_lossy().to_string()
    }

    #[test]
    fn write_then_read_roundtrip() {
        let p = tmp("a.txt");
        let w = write(&json!({"path": p, "content": "one\ntwo\n"}));
        assert!(w.model_text.starts_with("wrote "), "{}", w.model_text);
        match &w.display {
            Display::Code { note, .. } => assert!(note.is_empty(), "write shows no note: {note}"),
            other => panic!("expected code display, got {other:?}"),
        }
        let r = read(&json!({"path": p}));
        assert!(r.model_text.contains("one") && r.model_text.contains("two"));
        assert!(r.model_text.contains("     1\t"));
        match r.display {
            Display::Code { content, note, .. } => {
                assert_eq!(content, "one\ntwo\n");
                assert!(note.contains("2 lines"), "{note}");
            }
            other => panic!("expected code display, got {other:?}"),
        }
    }

    #[test]
    fn read_offset_and_limit() {
        let p = tmp("b.txt");
        write(&json!({"path": p, "content": "a\nb\nc\nd\n"}));
        let r = read(&json!({"path": p, "offset": 2, "limit": 2}));
        assert!(r.model_text.contains('b') && r.model_text.contains('c'));
        assert!(!r.model_text.contains("\ta\n"));
        assert!(r.model_text.contains("line limit"), "{}", r.model_text);
    }

    #[test]
    fn edit_requires_unique_match() {
        let p = tmp("c.txt");
        write(&json!({"path": p, "content": "x\nx\n"}));
        let dup = edit(&json!({"path": p, "old_string": "x", "new_string": "y"}));
        assert!(dup.model_text.contains("appears 2 times"), "{}", dup.model_text);
        let all = edit(&json!({"path": p, "old_string": "x", "new_string": "y", "replace_all": true}));
        assert!(all.model_text.contains("2 replacements"), "{}", all.model_text);
        let missing = edit(&json!({"path": p, "old_string": "zzz", "new_string": "q"}));
        assert!(missing.model_text.contains("not found"));
    }

    #[test]
    fn edit_reports_diff_display() {
        let p = tmp("d.txt");
        write(&json!({"path": p, "content": "keep\nold\n"}));
        let out = edit(&json!({"path": p, "old_string": "old", "new_string": "new"}));
        match out.display {
            Display::Diff { old, new, start_line } => {
                assert_eq!(old, "old");
                assert_eq!(new, "new");
                assert_eq!(start_line, 2);
            }
            other => panic!("expected diff, got {other:?}"),
        }
    }

    #[test]
    fn bash_captures_output_and_exit_code() {
        let ok = bash(&json!({"command": "echo hi"}));
        assert!(ok.model_text.starts_with("exit 0"), "{}", ok.model_text);
        assert!(ok.model_text.contains("hi"));
        match &ok.display {
            Display::Text { body } => {
                assert_eq!(body.trim(), "hi", "display omits exit code: {body:?}");
            }
            other => panic!("expected text display, got {other:?}"),
        }
        let bad = bash(&json!({"command": "echo oops >&2; exit 3"}));
        assert!(bad.model_text.starts_with("exit 3"), "{}", bad.model_text);
        assert!(bad.model_text.contains("[stderr]") && bad.model_text.contains("oops"));
    }

    #[test]
    fn bash_honours_timeout() {
        let r = bash(&json!({"command": "sleep 5", "timeout_secs": 1}));
        assert!(r.model_text.starts_with("timed out"), "{}", r.model_text);
    }

    #[test]
    fn invalid_json_reports_error() {
        assert!(execute("read", "{not json").model_text.starts_with("error:"));
        assert!(execute("nope", "{}").model_text.contains("unknown tool"));
    }
}
