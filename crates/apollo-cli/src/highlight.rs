use owo_colors::OwoColorize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Plain,
    Rust,
    Python,
    JavaScript,
    Json,
    Shell,
    Go,
    C,
    Toml,
    Other,
}

impl Lang {
    pub fn from_info(info: &str) -> Self {
        let lower = info.trim().to_ascii_lowercase();
        let first = lower.split_whitespace().next().unwrap_or("");
        match first {
            "" | "text" | "txt" | "plain" | "output" | "log" => Lang::Plain,
            "rust" | "rs" => Lang::Rust,
            "python" | "py" | "python3" => Lang::Python,
            "js" | "javascript" | "jsx" | "ts" | "typescript" | "tsx" => Lang::JavaScript,
            "json" | "jsonc" => Lang::Json,
            "bash" | "sh" | "shell" | "zsh" | "fish" => Lang::Shell,
            "go" | "golang" => Lang::Go,
            "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "java" => Lang::C,
            "toml" => Lang::Toml,
            _ => Lang::Other,
        }
    }

    pub fn from_path(path: &str) -> Self {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rs" => Lang::Rust,
            "py" | "pyi" => Lang::Python,
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => Lang::JavaScript,
            "json" => Lang::Json,
            "sh" | "bash" | "zsh" | "fish" => Lang::Shell,
            "go" => Lang::Go,
            "c" | "h" | "cc" | "cpp" | "hpp" | "java" => Lang::C,
            "toml" => Lang::Toml,
            "md" | "markdown" | "txt" | "log" | "" => Lang::Plain,
            _ => Lang::Other,
        }
    }

    fn line_comments(self) -> &'static [&'static str] {
        match self {
            Lang::Plain => &[],
            Lang::Rust | Lang::JavaScript | Lang::Go | Lang::C | Lang::Json => &["//"],
            Lang::Python | Lang::Shell | Lang::Toml => &["#"],
            Lang::Other => &["//", "#"],
        }
    }

    fn block_comment(self) -> Option<(&'static str, &'static str)> {
        match self {
            Lang::Rust | Lang::JavaScript | Lang::Go | Lang::C => Some(("/*", "*/")),
            _ => None,
        }
    }

    fn triple_quotes(self) -> &'static [&'static str] {
        match self {
            Lang::Python => &["'''", "\"\"\""],
            _ => &[],
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
                "super", "trait", "true", "type", "unsafe", "use", "where", "while",
            ],
            Lang::Python => &[
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def",
                "del", "elif", "else", "except", "False", "finally", "for", "from", "global", "if",
                "import", "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise",
                "return", "True", "try", "while", "with", "yield", "self",
            ],
            Lang::JavaScript => &[
                "async", "await", "break", "case", "catch", "class", "const", "continue", "default",
                "delete", "do", "else", "export", "extends", "false", "finally", "for", "from",
                "function", "if", "import", "in", "instanceof", "let", "new", "null", "of",
                "return", "static", "super", "switch", "this", "throw", "true", "try", "typeof",
                "undefined", "var", "void", "while", "yield", "interface", "type", "enum",
                "implements", "public", "private", "readonly",
            ],
            Lang::Json => &["true", "false", "null"],
            Lang::Shell => &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "return", "export", "local", "in", "echo", "cd", "set", "source",
            ],
            Lang::Go => &[
                "break", "case", "chan", "const", "continue", "default", "defer", "else",
                "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map",
                "package", "range", "return", "select", "struct", "switch", "type", "var", "nil",
                "true", "false",
            ],
            Lang::C => &[
                "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
                "else", "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long",
                "register", "restrict", "return", "short", "signed", "sizeof", "static", "struct",
                "switch", "typedef", "union", "unsigned", "void", "volatile", "while", "NULL",
                "true", "false", "class", "public", "private", "protected", "namespace", "new",
                "delete", "template", "using", "virtual",
            ],
            Lang::Toml => &["true", "false"],
            Lang::Plain | Lang::Other => &[],
        }
    }
}

pub struct State {
    lang: Lang,
    in_block: bool,
    triple: Option<&'static str>,
}

fn starts(chars: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    i + p.len() <= chars.len() && chars[i..i + p.len()] == p[..]
}

fn find(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let p: Vec<char> = pat.chars().collect();
    if p.is_empty() {
        return None;
    }
    let mut i = from;
    while i + p.len() <= chars.len() {
        if chars[i..i + p.len()] == p[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn flush(out: &mut String, plain: &mut String) {
    if !plain.is_empty() {
        out.push_str(plain);
        plain.clear();
    }
}

fn take(chars: &[char], from: usize, to: usize) -> String {
    chars[from..to.min(chars.len())].iter().collect()
}

impl State {
    pub fn new(lang: Lang) -> Self {
        Self {
            lang,
            in_block: false,
            triple: None,
        }
    }

    pub fn highlight(&mut self, line: &str) -> String {
        if self.lang == Lang::Plain {
            return line.to_string();
        }
        let c: Vec<char> = line.chars().collect();
        let n = c.len();
        let mut out = String::new();
        let mut plain = String::new();
        let mut i = 0usize;

        while i < n {
            if self.in_block {
                match find(&c, i, "*/") {
                    Some(k) => {
                        let s = take(&c, i, k + 2);
                        flush(&mut out, &mut plain);
                        out.push_str(&s.bright_black().to_string());
                        i = k + 2;
                        self.in_block = false;
                    }
                    None => {
                        let s = take(&c, i, n);
                        flush(&mut out, &mut plain);
                        out.push_str(&s.bright_black().to_string());
                        i = n;
                    }
                }
                continue;
            }

            if let Some(q) = self.triple {
                let len = q.chars().count();
                match find(&c, i, q) {
                    Some(k) => {
                        let s = take(&c, i, k + len);
                        flush(&mut out, &mut plain);
                        out.push_str(&s.green().to_string());
                        i = k + len;
                        self.triple = None;
                    }
                    None => {
                        let s = take(&c, i, n);
                        flush(&mut out, &mut plain);
                        out.push_str(&s.green().to_string());
                        i = n;
                    }
                }
                continue;
            }

            let mut opened_triple = None;
            for q in self.lang.triple_quotes() {
                if starts(&c, i, q) {
                    opened_triple = Some(*q);
                    break;
                }
            }
            if let Some(q) = opened_triple {
                flush(&mut out, &mut plain);
                out.push_str(&q.green().to_string());
                i += q.chars().count();
                self.triple = Some(q);
                continue;
            }

            let mut is_line_comment = false;
            for p in self.lang.line_comments() {
                if starts(&c, i, p) {
                    is_line_comment = true;
                    break;
                }
            }
            if is_line_comment {
                let s = take(&c, i, n);
                flush(&mut out, &mut plain);
                out.push_str(&s.bright_black().to_string());
                i = n;
                continue;
            }

            if let Some((open, _)) = self.lang.block_comment() {
                if starts(&c, i, open) {
                    flush(&mut out, &mut plain);
                    out.push_str(&open.bright_black().to_string());
                    i += open.chars().count();
                    self.in_block = true;
                    continue;
                }
            }

            let ch = c[i];

            if ch == '"' || ch == '\'' || ch == '`' {
                let start = i;
                i += 1;
                while i < n {
                    if c[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if c[i] == ch {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let s = take(&c, start, i.min(n));
                flush(&mut out, &mut plain);
                out.push_str(&s.green().to_string());
                continue;
            }

            if ch.is_ascii_digit() {
                let start = i;
                while i < n {
                    let d = c[i];
                    if d.is_ascii_alphanumeric() || d == '_' {
                        i += 1;
                    } else if d == '.' && i + 1 < n && c[i + 1].is_ascii_digit() {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let s = take(&c, start, i);
                flush(&mut out, &mut plain);
                out.push_str(&s.yellow().to_string());
                continue;
            }

            if ch.is_alphabetic() || ch == '_' {
                let start = i;
                while i < n && (c[i].is_alphanumeric() || c[i] == '_') {
                    i += 1;
                }
                let word = take(&c, start, i);
                if self.lang.keywords().contains(&word.as_str()) {
                    flush(&mut out, &mut plain);
                    out.push_str(&word.magenta().to_string());
                } else if i < n && (c[i] == '(' || c[i] == '!') {
                    flush(&mut out, &mut plain);
                    out.push_str(&word.bright_blue().to_string());
                } else if word.chars().next().is_some_and(|x| x.is_uppercase()) {
                    flush(&mut out, &mut plain);
                    out.push_str(&word.cyan().to_string());
                } else {
                    plain.push_str(&word);
                }
                continue;
            }

            plain.push(ch);
            i += 1;
        }

        flush(&mut out, &mut plain);
        out
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
    fn plain_lang_is_untouched() {
        let mut st = State::new(Lang::from_info(""));
        assert_eq!(st.highlight("[0, 1, 2]"), "[0, 1, 2]");
        let mut st = State::new(Lang::from_info("text"));
        assert_eq!(st.highlight("[0, 1, 2]"), "[0, 1, 2]");
    }

    #[test]
    fn text_is_preserved() {
        let mut st = State::new(Lang::Rust);
        let src = "fn main() { let x: u32 = 42; }";
        let out = st.highlight(src);
        assert_eq!(strip_ansi(&out), src);
    }

    #[test]
    fn rust_keyword_string_number_colors() {
        let mut st = State::new(Lang::from_info("rust"));
        let out = st.highlight("let name = \"apollo\";");
        assert!(out.contains("\x1b[35m"), "keyword should be magenta: {out:?}");
        assert!(out.contains("\x1b[32m"), "string should be green: {out:?}");
        let out2 = st.highlight("let n = 42;");
        assert!(out2.contains("\x1b[33m"), "number should be yellow: {out2:?}");
    }

    #[test]
    fn comments_are_bright_black() {
        let mut st = State::new(Lang::Rust);
        let out = st.highlight("// note");
        assert!(out.contains("\x1b[90m"), "comment should be bright black: {out:?}");
        let mut py = State::new(Lang::from_info("python"));
        assert!(py.highlight("# note").contains("\x1b[90m"));
    }

    #[test]
    fn block_comment_spans_lines() {
        let mut st = State::new(Lang::Rust);
        assert!(st.highlight("/* start").contains("\x1b[90m"));
        let mid = st.highlight("still comment");
        assert!(mid.contains("\x1b[90m"), "continuation should stay comment: {mid:?}");
        assert!(!mid.contains("\x1b[35m"));
        let end = st.highlight("end */ let x = 1;");
        assert!(end.contains("\x1b[35m"), "keyword after close: {end:?}");
    }

    #[test]
    fn python_triple_quote_spans_lines() {
        let mut st = State::new(Lang::from_info("py"));
        assert!(st.highlight("s = \"\"\"hello").contains("\x1b[32m"));
        let mid = st.highlight("def not_a_keyword()");
        assert!(mid.contains("\x1b[32m"), "inside docstring: {mid:?}");
        assert!(!mid.contains("\x1b[35m"));
        let end = st.highlight("bye\"\"\"\ndef f(): pass");
        assert!(end.contains("\x1b[32m"));
        assert!(end.contains("\x1b[35m"), "after docstring: {end:?}");
    }

    #[test]
    fn function_calls_are_bright_blue() {
        let mut st = State::new(Lang::from_info("python"));
        let out = st.highlight("print(x)");
        assert!(out.contains("\x1b[94m"), "function should be bright blue: {out:?}");
    }

    #[test]
    fn types_are_cyan() {
        let mut st = State::new(Lang::Rust);
        let out = st.highlight("let v: Vec<String> = Vec::new();");
        assert!(out.contains("\x1b[36m"), "type should be cyan: {out:?}");
    }

    #[test]
    fn url_inside_string_not_a_comment() {
        let mut st = State::new(Lang::from_info("js"));
        let out = st.highlight("const u = \"http://x\";");
        let code = &out[out.find("\"http").unwrap_or(0)..];
        assert!(!code.starts_with("\x1b[90m"), "url should stay a string: {out:?}");
    }

    #[test]
    fn escaped_quote_terminates_correctly() {
        let mut st = State::new(Lang::from_info("rust"));
        let src = "let s = \"a\\\"b\"; let t = 1;";
        let out = st.highlight(src);
        assert_eq!(strip_ansi(&out), src);
        assert!(out.contains("\x1b[35m"));
    }
}
