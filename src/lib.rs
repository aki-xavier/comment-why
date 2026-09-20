// The WHY-ONLY rule, over one file's source text: a comment has to say something the code under it
// does not.
//
// WHY IT READS TEXT. A comment is not in the compiler's syntax tree, so anything that judges one has
// to read the file. Keeping that here — rather than inside a compiler plugin — is what lets the same
// rules run from a lint, a test, or this crate's own command line, on any toolchain, with no
// dependencies.
//
// THE THREE SHAPES. Why against what is a reading of a sentence, and nothing here reads. What it can
// do is name the shapes a what-comment comes in, every one of them decidable from the text and the
// code it sits on:
//
//   NARRATION    first-person and step-by-step process, and the filler that carries no claim at all
//   RESTATEMENT  a comment line whose content words are all in the code below, so it says the
//                statement over again rather than why the statement is there
//   DEFINITION   a short doc comment that opens by re-saying the item's own name and stops
//
// WHY THE THREE ARE NARROW. Most of a codebase's comment blocks run past eight words and most of those
// carry no causal connective at all — units, index order, degenerate cases: what a caller cannot read
// off the code. A rule demanding a "because" of every long comment would have to exempt nearly all of
// them, and a rule that exempts most of its own corpus measures nothing. A what-comment that is none of
// the three shapes above is a reviewer's call — or a model's, over the residue this file does not
// claim (see `tools/comment_review.py`).
//
// OUT OF SCOPE: a file's first block and a module header name what they open. That is orientation, not
// explanation.
//
// WHY THE GATE IS IN HERE. `gate.rs` is the walk a project runs from its own suite: its roots, the
// floor its corpus has to clear, and the exceptions it records. Every project runs the same walk, so it
// is stated once rather than copied into each repository — where the copies would drift from the rules
// they are meant to hold that project to, silently, since a gate that measures the wrong thing still
// passes.

mod lexer;

pub mod gate;

pub mod sense;

use std::path::Path;

pub use lexer::CommentLine;

/// The verbs a restatement is built out of: the comment hands the code's own verb back to it, so only
/// the words left after these go decide the question. `// increment i` over `i += 1` is left holding
/// `i`, which the code says, so the comment said nothing.
const NOISE: &[&str] = &[
    "add",
    "adds",
    "append",
    "appends",
    "apply",
    "applies",
    "assign",
    "assigned",
    "assigns",
    "build",
    "builds",
    "call",
    "calls",
    "calculate",
    "calculates",
    "check",
    "checks",
    "clear",
    "clears",
    "compute",
    "computes",
    "create",
    "creates",
    "decrement",
    "decrements",
    "each",
    "fetch",
    "fetches",
    "finally",
    "first",
    "get",
    "gets",
    "handle",
    "handles",
    "increment",
    "increments",
    "init",
    "initialize",
    "initializes",
    "iterate",
    "iterates",
    "keep",
    "keeps",
    "loop",
    "loops",
    "next",
    "now",
    "over",
    "pop",
    "pops",
    "push",
    "pushes",
    "read",
    "reads",
    "reset",
    "resets",
    "return",
    "returns",
    "set",
    "sets",
    "setting",
    "step",
    "steps",
    "store",
    "stores",
    "take",
    "takes",
    "then",
    "update",
    "updates",
    "use",
    "uses",
    "value",
    "values",
    "write",
    "writes",
];

/// Function words: they carry no claim, so they neither make a restatement nor stop one.
const STOPWORD: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "been", "by", "did", "do", "does", "for", "from",
    "had", "has", "have", "her", "him", "his", "in", "into", "is", "it", "its", "of", "on", "or",
    "our", "out", "the", "their", "them", "they", "this", "those", "to", "up", "us", "was", "were",
    "with", "you", "your",
];

/// Narration and filler, as phrases over the padded word stream. Every one is a claim about the act of
/// writing the code rather than about the code, so none of them can say why a line is there.
///
/// The "this <noun>" half stops at the nouns a codebase uses for real things: the loop is the control
/// loop and a line is a line of source, so both are left out, and only a noun that cannot name
/// anything — a function, a method, a variable, a snippet — is banned.
///
/// A comment that QUOTES one of these phrases still trips the rule, since a mention and a use read the
/// same. Say it without the phrase.
const NARRATION: &[&str] = &[
    "as you can see",
    "first we",
    "fixme",
    "finally we",
    "for now",
    "just in case",
    "make sure",
    "next we",
    "note that",
    "now we",
    "obviously",
    "of course",
    "please note",
    "remember to",
    "simply",
    "temporarily",
    "then we",
    "this code",
    "this function",
    "this method",
    "this snippet",
    "this variable",
    "todo",
    "we first",
    "we next",
    "we then",
];

/// Marks a claim past the name: a doc comment carrying one is saying something, whatever it opens
/// with. Deliberately generous, because DEFINITION is only ever about what is left over.
const RATIONALE: &[&str] = &[
    "at least",
    "at most",
    "because",
    "but not",
    "cannot",
    "hence",
    "in order to",
    "instead of",
    "must",
    "no longer",
    "not a",
    "not the",
    "only",
    "otherwise",
    "rather than",
    "reason",
    "since",
    "so a",
    "so as to",
    "so it",
    "so its",
    "so that",
    "so the",
    "the point",
    "therefore",
    "this is why",
    "to avoid",
    "to keep",
    "to leave",
    "to make",
    "to preserve",
    "to prevent",
    "unlike",
    "which is what",
    "which is why",
    "why",
];

/// The longest a doc comment may be and still count as a definition: a paragraph opening with the
/// item's name has usually gone on to say something by its end.
const DEFINITION_WORDS: usize = 12;

/// How far past a comment its code reaches. A statement is rarely longer.
const WINDOW: usize = 4;

/// The declaration keywords whose following word is the item a doc comment documents.
const KINDS: &[&str] = &["const", "enum", "fn", "static", "struct", "trait", "type"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Narration,
    Restatement,
    Definition,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::Narration => "NARRATION",
            Rule::Restatement => "RESTATEMENT",
            Rule::Definition => "DEFINITION",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Rule::Narration => {
                "process and filler, which no reader needs: name why the line is there"
            }
            Rule::Restatement => "every content word of it is in the code below: say why, not what",
            Rule::Definition => {
                "it re-says the item's own name and stops: the name already said this"
            }
        }
    }
}

/// A comment that says what the code already says, and where it is.
pub struct Finding {
    pub rule: Rule,
    /// Byte range of the offence in the file: the line that did it, or the whole doc comment.
    pub start: usize,
    pub end: usize,
    pub text: String,
    /// The first line of the code the comment sits on, for the message.
    pub under: String,
}

/// One line of a comment, as the file wrote it.
#[derive(Clone)]
struct Line {
    start: usize,
    end: usize,
    text: String,
}

/// One comment, as the file wrote it: comment lines that follow one another are ONE comment, because
/// a reader takes in a paragraph at once.
pub struct Comment {
    /// 1-based, the line the comment opens on.
    pub line: usize,
    /// 1-based, the line it closes on.
    pub last_line: usize,
    /// Byte range of the whole comment in the file, marker included: what a diagnostic points at.
    pub start: usize,
    pub end: usize,
    /// True for `///`, `//!`, `/**` and `/*!`.
    pub doc: bool,
    /// True for a `/* */` block, which is its own comment rather than a line of the one above it.
    pub block: bool,
    pub full_line: bool,
    pub text: String,
    /// The comment line by line, which is what NARRATION and RESTATEMENT judge: a what-line appended
    /// to a paragraph is still a what-line, and a phrase cannot be invented across a line break.
    lines: Vec<Line>,
}

/// Every comment in `src`, in source order.
pub fn comments(src: &str) -> Vec<Comment> {
    let (raw, _) = lexer::read(src);
    let mut out: Vec<Comment> = Vec::new();
    for c in raw {
        let merges = out.last().is_some_and(|last| {
            last.doc == c.doc
                && last.block == c.block
                && last.full_line == c.full_line
                && c.line == last.last_line + 1
        });
        if merges {
            let last = out.last_mut().expect("matched above");
            last.text.push(' ');
            last.text.push_str(&c.text);
            last.lines.push(Line {
                start: c.start,
                end: c.end,
                text: c.text,
            });
            last.last_line = c.line;
            last.end = c.end;
        } else {
            out.push(Comment {
                line: c.line,
                last_line: c.line,
                start: c.start,
                end: c.end,
                doc: c.doc,
                block: c.block,
                full_line: c.full_line,
                text: c.text.clone(),
                lines: vec![Line {
                    start: c.start,
                    end: c.end,
                    text: c.text,
                }],
            });
        }
    }
    out
}

/// The first line of the code below a reader would call the statement: an attribute or a blank line is
/// the wrapper around it, and a diagnostic that points at `#[derive(...)]` points at the wrong thing.
pub fn first_code_line(window: &str) -> &str {
    window
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('/'))
        .unwrap_or("")
}

/// The item a doc comment documents: the named thing on the next code line that carries a declaration
/// keyword, or the field of a struct.
fn item_in(window: &str) -> Option<String> {
    for raw in window.lines().take(WINDOW) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('/') || line.starts_with('#') {
            continue;
        }
        let toks: Vec<&str> = line
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|s| !s.is_empty())
            .collect();
        let rest: &[&str] = if toks.first() == Some(&"pub") {
            &toks[1..]
        } else {
            &toks
        };
        if let Some(p) = rest.iter().position(|t| KINDS.contains(t)) {
            return rest.get(p + 1).map(|s| s.to_string());
        }
        if line.ends_with(',') || line.ends_with(':') {
            return rest.first().map(|s| s.to_string());
        }
        return None;
    }
    None
}

/// Every word in `text` with the apostrophes dropped, which is how one comment is compared with
/// another: `plane's` reads as two words either way.
pub fn words_of(text: &str) -> Vec<String> {
    words(&text.replace(['\'', '\u{2019}'], ""))
}

/// Every word in `text`, lowercased, with camel case and underscores split, so a comment writing
/// `u_lim` and code writing `u_lim` land on the same tokens.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            run.push(c);
        } else {
            split_ident(&mut out, &run);
            run.clear();
        }
    }
    split_ident(&mut out, &run);
    out
}

fn split_ident(out: &mut Vec<String>, run: &str) {
    let mut cur = String::new();
    let mut lower_before = false;
    for c in run.chars() {
        if c == '_' {
            flush(out, &mut cur);
            lower_before = false;
            continue;
        }
        if c.is_uppercase() && lower_before {
            flush(out, &mut cur);
        }
        cur.push(c.to_ascii_lowercase());
        lower_before = c.is_lowercase() || c.is_numeric();
    }
    flush(out, &mut cur);
}

fn flush(out: &mut Vec<String>, cur: &mut String) {
    if !cur.is_empty() {
        out.push(std::mem::take(cur));
    }
}

/// The text as a padded word stream with apostrophes dropped, so a NARRATION phrase matches a whole
/// word and an elision still reads as one.
fn padded(text: &str) -> String {
    let mut s = String::from(" ");
    for w in words_of(text) {
        s.push_str(&w);
        s.push(' ');
    }
    s
}

fn says(text: &str, phrases: &[&str]) -> bool {
    let padded = padded(text);
    phrases.iter().any(|p| padded.contains(&format!(" {p} ")))
}

/// Process narration and filler, on the line that carries it: a phrase is judged inside its line, so
/// one cannot be invented across a line break.
fn narration(line: &Line) -> bool {
    says(&line.text, NARRATION)
}

/// A line of the comment whose content words are all in the code below: it said the statement over
/// again. The code read is the comment's whole window, so a line anywhere in a paragraph is judged
/// against the same code.
fn restatement(comment: &Comment, line: &Line, window: &str) -> bool {
    if comment.doc || !comment.full_line {
        return false;
    }
    let content: Vec<String> = words(&line.text)
        .into_iter()
        .filter(|w| !STOPWORD.contains(&w.as_str()))
        .collect();
    if content.len() < 2 {
        return false;
    }
    // The verbs go, and what is left has to be the code's own vocabulary. A line that was nothing but
    // verbs has nothing left to judge, and is left to its author.
    let named: Vec<String> = content
        .iter()
        .filter(|w| !NOISE.contains(&w.as_str()))
        .cloned()
        .collect();
    if named.is_empty() {
        return false;
    }
    let code: Vec<String> = words(window);
    named.iter().all(|w| code.contains(w))
}

/// A short doc comment that opens with the item's own name and says nothing past it.
fn definition(comment: &Comment, window: &str) -> bool {
    if !comment.doc {
        return false;
    }
    let Some(item) = item_in(window) else {
        return false;
    };
    let ws = words_of(&comment.text);
    if ws.len() > DEFINITION_WORDS {
        return false;
    }
    let name = words(&item);
    if name.is_empty() || !name.iter().all(|w| ws.iter().take(3).any(|x| x == w)) {
        return false;
    }
    let lead = &ws[..ws.len().min(6)];
    if !lead
        .iter()
        .any(|w| w == "is" || w == "are" || w == "holds" || w == "means")
    {
        return false;
    }
    !says(&comment.text, RATIONALE)
}

/// One comment with everything known about it: the shapes it hits, and what the local approximation
/// makes of it when it hits none.
pub struct Reviewed {
    pub comment: Comment,
    /// The code below it, with the comments and the literals of that code blanked out.
    pub window: String,
    /// The shapes it hits, each pointing at the line that did it.
    pub findings: Vec<Finding>,
    /// The local approximation of "does this say something the code under it does not".
    pub sense: sense::Sense,
}

/// Every comment in `src`, judged: by the shapes first, and by the local approximation where the shapes
/// have nothing to say. This is the one place both callers — a gate, and the command line — read a file.
pub fn review(src: &str) -> Vec<Reviewed> {
    let (_, code) = lexer::read(src);
    // Comments and literals blanked out, so one comment can never satisfy a rule on the words of the
    // next, and a literal is never read as the statement.
    let blank: Vec<&str> = code.lines().collect();
    comments(src)
        .into_iter()
        .map(|comment| {
            let window = window_of(&blank, comment.last_line);
            let under = first_code_line(&window).to_string();
            let findings = offences(&comment, &window)
                .into_iter()
                .map(|(rule, line)| Finding {
                    rule,
                    start: line.start,
                    end: line.end,
                    text: line.text,
                    under: under.clone(),
                })
                .collect();
            let sense = sense::sense(&comment.text, &window);
            Reviewed {
                comment,
                window,
                findings,
                sense,
            }
        })
        .collect()
}

/// Every comment in `src` that says what the code already says, each pointing at the line that does.
pub fn scan(src: &str) -> Vec<Finding> {
    review(src).into_iter().flat_map(|r| r.findings).collect()
}

/// The code below a comment: the window a rule reads.
fn window_of(blank: &[&str], last_line: usize) -> String {
    blank
        .iter()
        .skip(last_line)
        .take(WINDOW)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// The shapes a comment hits, each with the line of the comment that did it.
fn offences(comment: &Comment, window: &str) -> Vec<(Rule, Line)> {
    // A definition is about the whole doc comment; the other two are about one line of it.
    let whole = Line {
        start: comment.start,
        end: comment.end,
        text: comment.text.clone(),
    };
    let mut hit: Vec<(Rule, Line)> = Vec::new();
    if let Some(line) = comment.lines.iter().find(|l| narration(l)) {
        hit.push((Rule::Narration, line.clone()));
    }
    if let Some(line) = comment
        .lines
        .iter()
        .find(|l| restatement(comment, l, window))
    {
        hit.push((Rule::Restatement, line.clone()));
    }
    if definition(comment, window) {
        hit.push((Rule::Definition, whole));
    }
    hit
}

/// Every `.rs` file under the roots, in a stable order, as paths relative to `root`. Build outputs and
/// dot directories are left out: they are not a project's prose.
pub fn sources(root: &Path, roots: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for r in roots {
        walk(root, &root.join(r), &mut out);
    }
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

/// The 1-based line a byte offset falls on, so a finding can be named the way a reader sees it.
pub fn line_of(src: &str, offset: usize) -> usize {
    src.as_bytes()[..offset.min(src.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_of(src: &str) -> Vec<Rule> {
        scan(src).into_iter().map(|f| f.rule).collect()
    }

    #[test]
    fn the_shapes_are_caught() {
        let cases: &[(&str, &str, Rule)] = &[
            (
                "narration",
                "// First we normalize the vector, then we scale it.\nlet x = a / n;\n",
                Rule::Narration,
            ),
            (
                "restatement",
                "fn tick() {\n    let mut i = 0;\n    // increment i\n    i += 1;\n}\n",
                Rule::Restatement,
            ),
            (
                "definition",
                "/// MotionWindow is the window the theory supports.\npub struct MotionWindow {\n    pub wn: f64,\n}\n",
                Rule::Definition,
            ),
        ];
        for (what, src, want) in cases {
            let got = rules_of(src);
            assert!(
                got.contains(want),
                "the {what} fixture has to fire {}: got {got:?}",
                want.name()
            );
        }
    }

    #[test]
    fn a_why_comment_is_left_alone() {
        let src = r#"
fn hold(u: f64) -> f64 {
    // A bound this side of anything the hold asks for is a floating machine, so the base gives way.
    let u = u;
    // the base rows are not the welded machine's (it has none): they are the wrench the hold costs
    let t = 0.0;
    u + t
}

/// The complement, without which the tests above prove nothing about the number: a bound SHORT of what
/// the hold asks for is a floating machine.
pub fn check() {}
"#;
        assert_eq!(rules_of(src), Vec::new());
    }

    #[test]
    fn a_comment_is_read_only_where_rust_reads_one() {
        let src = r##"
// a line comment
let s = "// not a comment";
let raw = r#"// not a comment either"#;
let c = '/';
let l: &'static str = "x";
/* a block
   comment */
/// a doc comment
pub struct Thing {
    /// a field's doc
    pub f: f64,
}
"##;
        let texts: Vec<String> = comments(src).into_iter().map(|c| c.text).collect();
        assert_eq!(
            texts,
            vec![
                "a line comment",
                "a block comment",
                "a doc comment",
                "a field's doc"
            ],
            "a comment has to be read where Rust reads one, and code where Rust reads code"
        );
        let (_, code) = lexer::read(src);
        assert!(
            !code.contains("not a comment"),
            "a literal is data, not the code under a comment"
        );
        assert!(code.contains("pub struct Thing {"));
    }

    #[test]
    fn a_restatement_inside_a_paragraph_is_still_a_restatement() {
        let src = "// The loop holds the base here, because a weld is a constraint.\n// increment i\nlet mut i = 0;\ni += 1;\n";
        assert!(
            rules_of(src).contains(&Rule::Restatement),
            "got {:?}",
            rules_of(src)
        );
    }

    #[test]
    fn a_doc_comment_is_recognised_by_its_marker() {
        let got: Vec<(usize, bool)> =
            comments("/// doc\n// line\n//! inner\n/** block doc */\n/* block */\nx();\n")
                .into_iter()
                .map(|c| (c.line, c.doc))
                .collect();
        assert_eq!(
            got,
            vec![(1, true), (2, false), (3, true), (4, true), (5, false)]
        );
    }
}
