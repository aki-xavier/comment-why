// lexer.rs — where a comment begins and ends, and what the code under it says.
//
// A comment is not a token the compiler hands out, so the file has to be read. This is the one place
// that decides that, and it is deliberately small: strings, raw strings, character literals and block
// comments are the states Rust needs for it, and nothing else.

/// One line of one comment, as the file wrote it.
pub struct CommentLine {
    /// 1-based, the line the comment opens on.
    pub line: usize,
    /// Byte offsets of the comment in the file, marker included.
    pub start: usize,
    pub end: usize,
    /// The text with the marker stripped.
    pub text: String,
    /// True for `///`, `//!`, `/**` and `/*!`.
    pub doc: bool,
    /// True for a `/* */` block: a block is its own comment, and never a line of the one above it.
    pub block: bool,
    /// True when only whitespace precedes the marker, so the comment stands on its own line.
    pub full_line: bool,
}

/// The comments of `src`, in source order, and the same source with every comment and every string
/// literal blanked to spaces. Newlines are kept, so line numbers still line up with the file.
///
/// A string literal is blanked because it is data: a comment echoing one is not restating the
/// statement under it, and an assert message must never read as the code a comment sits on.
pub fn read(src: &str) -> (Vec<CommentLine>, String) {
    let ch: Vec<char> = src.chars().collect();
    let n = ch.len();
    let mut comments = Vec::new();
    let mut code = String::with_capacity(src.len());
    let mut i = 0usize;
    let mut line = 1usize;
    let mut blank = true;
    while i < n {
        let c = ch[i];
        if c == '\n' {
            line += 1;
            blank = true;
            code.push(c);
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            code.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < n && ch[i + 1] == '/' {
            let doc = i + 2 < n
                && (ch[i + 2] == '!' || (ch[i + 2] == '/' && !(i + 3 < n && ch[i + 3] == '/')));
            let start = if i + 2 < n { i + 3 } else { n };
            let mut j = start;
            while j < n && ch[j] != '\n' {
                j += 1;
            }
            comments.push(CommentLine {
                line,
                start: i,
                end: j,
                text: ch[start..j].iter().collect::<String>().trim().to_string(),
                doc,
                block: false,
                full_line: blank,
            });
            blank_span(&mut code, &ch[i..j]);
            i = j;
            continue;
        }
        if c == '/' && i + 1 < n && ch[i + 1] == '*' {
            let doc = (i + 2 < n && ch[i + 2] == '*' && !(i + 3 < n && ch[i + 3] == '/'))
                || (i + 2 < n && ch[i + 2] == '!');
            let start = i;
            let start_line = line;
            let mut depth = 1usize;
            let mut body = String::new();
            let mut j = i + 2;
            while j < n && depth > 0 {
                if ch[j] == '/' && j + 1 < n && ch[j + 1] == '*' {
                    depth += 1;
                    j += 2;
                    continue;
                }
                if ch[j] == '*' && j + 1 < n && ch[j + 1] == '/' {
                    depth -= 1;
                    j += 2;
                    continue;
                }
                if ch[j] == '\n' {
                    line += 1;
                }
                if depth == 1 {
                    body.push(ch[j]);
                }
                j += 1;
            }
            // A `*` hanging off each continuation line is decoration, not prose.
            let text: Vec<String> = body
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            comments.push(CommentLine {
                line: start_line,
                start,
                end: j,
                text: text.join(" "),
                doc,
                block: true,
                full_line: blank,
            });
            blank_span(&mut code, &ch[start..j]);
            i = j;
            continue;
        }
        if c == '"' {
            let start = i;
            let mut j = i + 1;
            while j < n {
                if ch[j] == '\\' {
                    if j + 1 < n && ch[j + 1] == '\n' {
                        line += 1;
                    }
                    j += 2;
                    continue;
                }
                if ch[j] == '"' {
                    j += 1;
                    break;
                }
                if ch[j] == '\n' {
                    line += 1;
                }
                j += 1;
            }
            let end = j.min(n);
            blank_span(&mut code, &ch[start..end]);
            i = end;
            continue;
        }
        if c == 'r' {
            let mut hashes = 0usize;
            let mut k = i + 1;
            while k < n && ch[k] == '#' {
                hashes += 1;
                k += 1;
            }
            if k < n && ch[k] == '"' {
                let start = i;
                let mut j = k + 1;
                while j < n {
                    if ch[j] == '\n' {
                        line += 1;
                        j += 1;
                        continue;
                    }
                    if ch[j] == '"' {
                        let mut close = j + 1;
                        let mut seen = 0usize;
                        while seen < hashes && close < n && ch[close] == '#' {
                            seen += 1;
                            close += 1;
                        }
                        if seen == hashes {
                            j = close;
                            break;
                        }
                    }
                    j += 1;
                }
                let end = j.min(n);
                blank_span(&mut code, &ch[start..end]);
                i = end;
                continue;
            }
        }
        if c == '\'' {
            // `'a'` and `'\n'` are character literals; the `'a` of a lifetime is not, and a lifetime is
            // left to the code path below.
            if i + 1 < n && ch[i + 1] == '\\' {
                let mut j = i + 2;
                while j < n && ch[j] != '\'' && ch[j] != '\n' && j < i + 12 {
                    j += 1;
                }
                if j < n && ch[j] == '\'' {
                    blank_span(&mut code, &ch[i..=j]);
                    i = j + 1;
                    continue;
                }
            } else if i + 2 < n && ch[i + 2] == '\'' && ch[i + 1] != '\'' && ch[i + 1] != '\\' {
                blank_span(&mut code, &ch[i..=i + 2]);
                i += 3;
                continue;
            }
        }
        code.push(c);
        blank = false;
        i += 1;
    }
    (comments, code)
}

/// Exactly as many characters as were taken, as spaces, so the byte offsets of everything after them
/// are unchanged — except newlines, which stay newlines so line numbers keep counting.
fn blank_span(code: &mut String, span: &[char]) {
    for c in span {
        code.push(if *c == '\n' { '\n' } else { ' ' });
    }
}
