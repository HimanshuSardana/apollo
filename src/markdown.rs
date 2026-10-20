use owo_colors::Style;
use unicode_width::UnicodeWidthStr;

use crate::highlight::{Lang, State as Highlighter};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
}

impl Segment {
    fn plain(text: &str) -> Self {
        Self {
            text: text.to_string(),
            bold: false,
            italic: false,
            code: false,
        }
    }
}

fn paint(text: &str, bold: bool, italic: bool, code: bool) -> String {
    let mut style = Style::new();
    if bold {
        style = style.bold();
    }
    if italic {
        style = style.italic();
    }
    if code {
        style = style.green();
    }
    style.style(text).to_string()
}

fn dim_str(s: &str) -> String {
    Style::new().dimmed().style(s).to_string()
}

fn render_segments(segs: &[Segment]) -> String {
    let mut out = String::new();
    for s in segs {
        if s.text.is_empty() {
            continue;
        }
        out.push_str(&paint(&s.text, s.bold, s.italic, s.code));
    }
    out
}

fn parse_inline(input: &str) -> Vec<Segment> {
    let mut segs = Vec::new();
    parse_into(input, None, &mut segs);
    segs
}

pub fn render_inline(line: &str) -> String {
    render_segments(&parse_inline(line))
}

fn plain_width(cell: &str) -> usize {
    parse_inline(cell).iter().map(|s| s.text.width()).sum()
}

fn parse_into(s: &str, prev: Option<char>, out: &mut Vec<Segment>) {
    let chars: Vec<char> = s.chars().collect();
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], '*' | '_' | '`' | '\\') {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let (marker, mlen): (&str, usize) = if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            ("**", 2)
        } else if c == '_' && i + 1 < chars.len() && chars[i + 1] == '_' {
            ("__", 2)
        } else if c == '*' || c == '_' || c == '`' {
            (if c == '*' { "*" } else if c == '_' { "_" } else { "`" }, 1)
        } else {
            buf.push(c);
            i += 1;
            continue;
        };

        if marker == "`" {
            let mut n = 1;
            while i + n < chars.len() && chars[i + n] == '`' {
                n += 1;
            }
            if n >= 3 {
                for _ in 0..n {
                    buf.push('`');
                }
                i += n;
                continue;
            }
            let mut j = i + n;
            let mut found = None;
            while j < chars.len() {
                if chars[j] == '`' {
                    let mut m = 1;
                    while j + m < chars.len() && chars[j + m] == '`' {
                        m += 1;
                    }
                    if m == n {
                        found = Some(j);
                        break;
                    }
                    j += m;
                } else {
                    j += 1;
                }
            }
            match found {
                Some(j) => {
                    if !buf.is_empty() {
                        out.push(Segment::plain(&buf));
                        buf.clear();
                    }
                    let inner: String = chars[i + n..j].iter().collect();
                    out.push(Segment {
                        text: inner,
                        bold: false,
                        italic: false,
                        code: true,
                    });
                    i = j + n;
                }
                None => {
                    for _ in 0..n {
                        buf.push('`');
                    }
                    i += n;
                }
            }
            continue;
        }

        let before = if i > 0 { Some(chars[i - 1]) } else { prev };
        let after = chars.get(i + mlen).copied();
        let opener_ok = match after {
            Some(ch) => !ch.is_whitespace(),
            None => false,
        } && match marker {
            "_" | "__" => match before {
                None => true,
                Some(b) => !b.is_alphanumeric(),
            },
            _ => true,
        };
        if !opener_ok {
            buf.push_str(marker);
            i += mlen;
            continue;
        }

        let mut j = i + mlen;
        let mut found = None;
        while j < chars.len() {
            if mlen == 2 {
                if chars[j] == c && j + 1 < chars.len() && chars[j + 1] == c {
                    let p = chars[j - 1];
                    let nx = chars.get(j + 2).copied();
                    let ok = !p.is_whitespace()
                        && match marker {
                            "__" => match nx {
                                Some(nc) => !nc.is_alphanumeric(),
                                None => true,
                            },
                            _ => true,
                        };
                    if ok {
                        found = Some(j);
                        break;
                    }
                    j += 2;
                } else {
                    j += 1;
                }
            } else if chars[j] == c {
                if j + 1 < chars.len() && chars[j + 1] == c {
                    j += 2;
                    continue;
                }
                let p = chars[j - 1];
                let nx = chars.get(j + 1).copied();
                let ok = !p.is_whitespace()
                    && match marker {
                        "_" => match nx {
                            Some(nc) => !nc.is_alphanumeric(),
                            None => true,
                        },
                        _ => true,
                    };
                if ok {
                    found = Some(j);
                    break;
                }
                j += 1;
            } else {
                j += 1;
            }
        }

        match found {
            Some(j) => {
                if !buf.is_empty() {
                    out.push(Segment::plain(&buf));
                    buf.clear();
                }
                let inner: String = chars[i + mlen..j].iter().collect();
                let mut inner_segs = Vec::new();
                parse_into(&inner, Some(c), &mut inner_segs);
                for s in &mut inner_segs {
                    if marker == "**" || marker == "__" {
                        s.bold = true;
                    } else {
                        s.italic = true;
                    }
                }
                out.extend(inner_segs);
                i = j + mlen;
            }
            None => {
                buf.push_str(marker);
                i += mlen;
            }
        }
    }
    if !buf.is_empty() {
        out.push(Segment::plain(&buf));
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

fn split_row(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if let Some(s) = t.strip_prefix('|') {
        t = s;
    }
    if let Some(s) = t.strip_suffix('|') {
        t = s;
    }
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn is_table_row(line: &str) -> bool {
    line.trim().contains('|')
}

fn is_delimiter_row(line: &str) -> bool {
    if !line.contains('-') {
        return false;
    }
    let cells = split_row(line);
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|c| {
        let mut c = c.as_str();
        if let Some(s) = c.strip_prefix(':') {
            c = s;
        }
        if let Some(s) = c.strip_suffix(':') {
            c = s;
        }
        !c.is_empty() && c.bytes().all(|b| b == b'-')
    })
}

fn cell_align(cell: &str) -> Align {
    let l = cell.starts_with(':');
    let r = cell.ends_with(':');
    if l && r {
        Align::Center
    } else if r {
        Align::Right
    } else {
        Align::Left
    }
}

fn pad_split(diff: usize, align: Align) -> (usize, usize) {
    match align {
        Align::Left => (0, diff),
        Align::Right => (diff, 0),
        Align::Center => (diff / 2, diff - diff / 2),
    }
}

fn render_table(hold: &[String]) -> String {
    let header = split_row(&hold[0]);
    let ncols = header.len().max(1);
    let aligns: Vec<Align> = if hold.len() >= 2 {
        let d = split_row(&hold[1]);
        (0..ncols)
            .map(|j| d.get(j).map(|c| cell_align(c)).unwrap_or(Align::Left))
            .collect()
    } else {
        vec![Align::Left; ncols]
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    for l in hold.iter().skip(2) {
        let mut cells = split_row(l);
        cells.resize(ncols, String::new());
        cells.truncate(ncols);
        rows.push(cells);
    }
    let mut widths = vec![0usize; ncols];
    for (j, h) in header.iter().enumerate().take(ncols) {
        widths[j] = widths[j].max(plain_width(h));
    }
    for row in &rows {
        for (j, c) in row.iter().enumerate() {
            widths[j] = widths[j].max(plain_width(c));
        }
    }

    let mut out = String::new();
    let mut line = String::new();
    for j in 0..ncols {
        line.push_str(&dim_str("|"));
        line.push(' ');
        let cell = header.get(j).map(String::as_str).unwrap_or("");
        let w = plain_width(cell);
        let mut segs = parse_inline(cell);
        for s in &mut segs {
            s.bold = true;
        }
        let (_, after) = pad_split(widths[j].saturating_sub(w), Align::Left);
        line.push_str(&render_segments(&segs));
        line.push_str(&" ".repeat(after));
        line.push(' ');
    }
    line.push_str(&dim_str("|"));
    line.push('\n');
    out.push_str(&line);

    let mut line = String::new();
    for j in 0..ncols {
        line.push_str(&dim_str("|"));
        let inner = match aligns[j] {
            Align::Left => "-".repeat(widths[j] + 2),
            Align::Right => format!("{}:", "-".repeat(widths[j] + 1)),
            Align::Center => format!(":{}:", "-".repeat(widths[j])),
        };
        line.push_str(&dim_str(&inner));
    }
    line.push_str(&dim_str("|"));
    line.push('\n');
    out.push_str(&line);

    for row in &rows {
        let mut line = String::new();
        for j in 0..ncols {
            line.push_str(&dim_str("|"));
            line.push(' ');
            let w = plain_width(&row[j]);
            let (before, after) = pad_split(widths[j].saturating_sub(w), aligns[j]);
            line.push_str(&" ".repeat(before));
            line.push_str(&render_inline(&row[j]));
            line.push_str(&" ".repeat(after));
            line.push(' ');
        }
        line.push_str(&dim_str("|"));
        line.push('\n');
        out.push_str(&line);
    }
    out
}

fn fence_marker(line: &str) -> Option<(char, &str)> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    let fc = t.chars().next()?;
    if fc != '`' && fc != '~' {
        return None;
    }
    let n = t.chars().take_while(|&ch| ch == fc).count();
    if n < 3 {
        return None;
    }
    Some((fc, t[n..].trim()))
}

pub struct StreamRenderer {
    pending: String,
    fence_char: Option<char>,
    code: Option<Highlighter>,
    table_hold: Vec<String>,
    table_confirmed: bool,
    out: String,
}

impl StreamRenderer {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            fence_char: None,
            code: None,
            table_hold: Vec::new(),
            table_confirmed: false,
            out: String::new(),
        }
    }

    pub fn push(&mut self, text: &str) -> String {
        self.pending.push_str(text);
        self.process();
        std::mem::take(&mut self.out)
    }

    pub fn flush(&mut self) -> String {
        self.process();
        if !self.pending.is_empty() {
            let tail = std::mem::take(&mut self.pending);
            self.handle_line(&tail);
        }
        self.resolve_hold();
        std::mem::take(&mut self.out)
    }

    fn process(&mut self) {
        while let Some(pos) = self.pending.find('\n') {
            let chunk: String = self.pending.drain(..=pos).collect();
            let line = chunk.trim_end_matches(['\n', '\r']).to_string();
            self.handle_line(&line);
        }
    }

    fn handle_line(&mut self, line: &str) {
        if let Some(fc) = self.fence_char {
            if fence_marker(line)
                .map(|(c, info)| c == fc && info.is_empty())
                .unwrap_or(false)
            {
                self.fence_char = None;
                self.code = None;
                self.emit_line(&dim_str(line));
            } else {
                let rendered = match self.code.as_mut() {
                    Some(st) => st.highlight(line),
                    None => paint(line, false, false, true),
                };
                self.emit_line(&rendered);
            }
            return;
        }
        if let Some((fc, info)) = fence_marker(line) {
            self.fence_char = Some(fc);
            self.code = Some(Highlighter::new(Lang::from_info(info)));
            self.emit_line(&dim_str(line));
            return;
        }
        if !self.table_hold.is_empty() {
            if is_table_row(line) {
                self.table_hold.push(line.to_string());
                if !self.table_confirmed && self.table_hold.len() >= 2 {
                    self.table_confirmed = is_delimiter_row(&self.table_hold[1]);
                }
                return;
            }
            self.resolve_hold();
        }
        if is_table_row(line) {
            self.table_hold.push(line.to_string());
            return;
        }
        self.emit_line(&render_inline(line));
    }

    fn resolve_hold(&mut self) {
        if self.table_hold.is_empty() {
            return;
        }
        let hold = std::mem::take(&mut self.table_hold);
        let confirmed = std::mem::replace(&mut self.table_confirmed, false);
        if confirmed {
            self.emit_raw(&render_table(&hold));
        } else {
            for l in &hold {
                let rendered = render_inline(l);
                self.emit_line(&rendered);
            }
        }
    }

    fn emit_line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn emit_raw(&mut self, s: &str) {
        self.out.push_str(s);
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

    fn flags(s: &str) -> Vec<(String, bool, bool, bool)> {
        parse_inline(s)
            .iter()
            .map(|seg| (seg.text.clone(), seg.bold, seg.italic, seg.code))
            .collect()
    }

    #[test]
    fn bold_and_plain() {
        assert_eq!(
            flags("a **b** c"),
            vec![
                ("a ".to_string(), false, false, false),
                ("b".to_string(), true, false, false),
                (" c".to_string(), false, false, false),
            ]
        );
    }

    #[test]
    fn italic_underscore_and_star() {
        assert_eq!(
            flags("_a_ and *b*"),
            vec![
                ("a".to_string(), false, true, false),
                (" and ".to_string(), false, false, false),
                ("b".to_string(), false, true, false),
            ]
        );
    }

    #[test]
    fn nested_bold_italic() {
        let segs = parse_inline("**a *b* c**");
        assert_eq!(segs.len(), 3);
        assert!(segs.iter().all(|s| s.bold));
        assert_eq!(segs[1].text, "b");
        assert!(segs[1].italic);
        assert!(!segs[0].italic);
    }

    #[test]
    fn inline_code_is_literal() {
        assert_eq!(
            flags("run `a **b**` now"),
            vec![
                ("run ".to_string(), false, false, false),
                ("a **b**".to_string(), false, false, true),
                (" now".to_string(), false, false, false),
            ]
        );
    }

    #[test]
    fn unclosed_markers_stay_literal() {
        assert_eq!(
            flags("**oops and *uh"),
            vec![("**oops and *uh".to_string(), false, false, false),]
        );
    }

    #[test]
    fn snake_case_untouched() {
        for word in ["my_var_name", "foo_bar", "a_b_c"] {
            assert_eq!(
                flags(word),
                vec![(word.to_string(), false, false, false)],
                "failed for {word}"
            );
        }
    }

    #[test]
    fn delimiter_row_detection() {
        assert!(is_delimiter_row("|---|---|"));
        assert!(is_delimiter_row("| :--- | ---: | :---: |"));
        assert!(is_delimiter_row("---|---|---"));
        assert!(!is_delimiter_row("| a | b |"));
        assert!(is_delimiter_row("| --- |"));
    }

    #[test]
    fn table_renders_aligned() {
        let out = render_table(&[
            "| a | bb |".to_string(),
            "|---|---|".to_string(),
            "| c | d |".to_string(),
        ]);
        assert_eq!(
            strip_ansi(&out),
            "| a | bb |\n|---|----|\n| c | d  |\n"
        );
    }

    #[test]
    fn stream_split_emphasis_across_chunks() {
        let mut r = StreamRenderer::new();
        let a = r.push("a **bo");
        let b = r.push("ld** c\nrest");
        let c = r.flush();
        assert_eq!(strip_ansi(&(a + &b + &c)), "a bold c\nrest\n");
        assert!(format!("{b}{c}").contains("\x1b[1m"));
    }

    #[test]
    fn stream_code_fence() {
        let mut r = StreamRenderer::new();
        let a = r.push("```rs\nlet x = 1;\n");
        let b = r.push("```\n");
        let c = r.flush();
        let all = a + &b + &c;
        assert_eq!(strip_ansi(&all), "```rs\nlet x = 1;\n```\n");
        assert!(all.contains("\x1b[35m"), "keyword highlighted: {all:?}");
        assert!(all.contains("\x1b[33m"), "number highlighted: {all:?}");
    }

    #[test]
    fn fence_python_highlighted_and_text_plain() {
        let mut r = StreamRenderer::new();
        let out = r.push("```python\ndef f():\n    return 1\n```\n```text\n[0, 1]\n```\n");
        let out = out + &r.flush();
        assert!(out.contains("\x1b[35m"), "def highlighted: {out:?}");
        assert!(out.contains("[0, 1]"));
        assert!(!out.contains("\x1b[33m[0"), "text fence left plain: {out:?}");
    }

    #[test]
    fn stream_table_held_until_complete() {
        let mut r = StreamRenderer::new();
        assert_eq!(r.push("| a |\n"), "");
        assert_eq!(r.push("|---|\n"), "");
        assert_eq!(r.push("| b |\n"), "");
        let done = r.push("tail\n");
        assert_eq!(strip_ansi(&done), "| a |\n|---|\n| b |\ntail\n");
    }

    #[test]
    fn stream_false_table_flushed_as_text() {
        let mut r = StreamRenderer::new();
        assert_eq!(r.push("a | b\n"), "");
        let done = r.push("plain\n");
        assert_eq!(strip_ansi(&done), "a | b\nplain\n");
    }
}

#[cfg(test)]
mod preview {
    use super::*;

    #[test]
    #[ignore]
    fn preview() {
        let chunks = [
            "Here is a **bold** claim and an _italic_ one, plus `inline_code`.\n",
            "\n| Name | Type |\n",
            "|:---|---:|\n",
            "| `alpha` | **int** |\n",
            "| beta | _str_ |\n",
            "\n```python\ndef fibonacci(n):\n    if n <= 0:\n        return []\n    seq = [0, 1]\n    while len(seq) < n:\n        seq.append(seq[-1] + seq[-2])  # grow\n    return seq\n\nprint(fibonacci(10))\n```\n",
            "\n```rust\nfn main() {\n    let s: String = \"hi **not bold**\".into();\n}\n```\n",
            "Done.",
        ];
        let mut r = StreamRenderer::new();
        for c in chunks {
            print!("{}", r.push(c));
        }
        print!("{}", r.flush());
        println!();
    }
}
