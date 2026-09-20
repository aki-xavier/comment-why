// comment-why — the comment gate: what the rules can decide, and what they can only suspect.
//
// Two jobs, one pass over the text. The first is the gate: a comment that says what the code under it
// already says is a defect here, and it fails. The second, under `--review`, is the local approximation
// in `sense.rs` for the comments the rules leave alone — a rationale marker, and how much of the
// comment is the code's own vocabulary. A suspect never fails anything by itself: it is a place to look.

use comment_why::{first_code_line, line_of, review, sense::Verdict, sources, Finding, Reviewed};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The roots a project's prose lives in, when the command line names none.
const ROOTS: &[&str] = &["src", "tests", "examples"];

fn main() {
    let mut roots: Vec<String> = Vec::new();
    let mut changed: Option<String> = None;
    let mut review_all = false;
    let mut with_review = false;
    let mut strict_suspect = false;
    let mut limit = usize::MAX;
    let mut format = "text".to_string();
    let mut root = PathBuf::from(".");

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--changed-only" => changed = Some("HEAD".to_string()),
            "--base" => changed = args.next(),
            "--review" => with_review = true,
            "--review-all" => {
                with_review = true;
                review_all = true;
            }
            "--strict-suspect" => strict_suspect = true,
            "--limit" => {
                limit = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(usize::MAX)
            }
            "--format" => format = args.next().unwrap_or(format),
            "--root" => root = args.next().map(PathBuf::from).unwrap_or(root),
            "--help" | "-h" => {
                println!(
                    "usage: comment-why [ROOTS...] [--changed-only [--base REF]] [--review]\n\
                     \x20                  [--review-all] [--strict-suspect] [--limit N] [--format json|text]\n\
                     \x20                  [--root DIR]\n\
                     ROOTS default to src tests examples. The exit is 1 when a comment says what the code\n\
                     already says; --review adds the local approximation for the comments the rules leave\n\
                     alone, and --strict-suspect is what makes those fail too."
                );
                return;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option: {other}");
                std::process::exit(2);
            }
            other => roots.push(other.to_string()),
        }
    }
    if roots.is_empty() {
        roots = ROOTS.iter().map(|r| r.to_string()).collect();
    }

    let units = disk_units(&root, &roots);
    let touched = changed
        .as_deref()
        .map(|base| changed_lines(&root, base, &roots));
    let mut failures: Vec<Out> = Vec::new();
    let mut suspects: Vec<Out> = Vec::new();
    let mut judged = 0usize;

    'units: for (file, src) in &units {
        for one in review(src) {
            if let Some(lines) = &touched {
                let hit = lines.get(file).is_some_and(|set| {
                    (one.comment.line..=one.comment.last_line).any(|l| set.contains(&l))
                });
                if !hit {
                    continue;
                }
            }
            judged += 1;
            if judged > limit {
                break 'units;
            }
            if one.findings.is_empty() {
                if with_review && (review_all || one.sense.verdict == Verdict::Suspect) {
                    suspects.push(Out::suspect(file, &one));
                }
            } else {
                for finding in &one.findings {
                    failures.push(Out::offence(file, src, finding));
                }
            }
        }
    }

    if format == "json" {
        print_json(&failures, &suspects);
    } else {
        for out in failures.iter().chain(suspects.iter()) {
            println!("{}", out.display);
        }
        if !failures.is_empty() {
            println!(
                "\n{} comment(s) say what the code already says.",
                failures.len()
            );
        }
        if with_review && !suspects.is_empty() {
            println!(
                "{} comment(s) read as the code's own words: advice, not a failure{}.",
                suspects.len(),
                if strict_suspect {
                    ""
                } else {
                    " (--strict-suspect fails on them)"
                }
            );
        }
    }

    if !failures.is_empty() || (strict_suspect && !suspects.is_empty()) {
        std::process::exit(1);
    }
}

/// One line of output: where it is, what it is, and the line to print.
struct Out {
    file: String,
    line: usize,
    kind: String,
    under: String,
    display: String,
}

impl Out {
    fn offence(file: &str, src: &str, finding: &Finding) -> Out {
        let line = line_of(src, finding.start);
        let rule = finding.rule.name();
        // A comment whose four lines below carry no code at all — a file's opening block, say — has
        // nothing to show, and an empty field reads like a bug. Say so instead.
        let under = if finding.under.is_empty() {
            "(no code within four lines)".to_string()
        } else {
            finding.under.clone()
        };
        Out {
            file: file.to_string(),
            line,
            kind: rule.to_string(),
            under: finding.under.clone(),
            display: format!(
                "{file}:{line}  {rule}\n      \"{}\"\n      under it: {under}",
                finding.text
            ),
        }
    }

    fn suspect(file: &str, one: &Reviewed) -> Out {
        let verdict = one.sense.verdict.name();
        Out {
            file: file.to_string(),
            line: one.comment.line,
            kind: verdict.to_string(),
            under: first_code_line(&one.window).to_string(),
            display: format!(
                "{file}:{}  {verdict} (overlap {:.2}, {} words)\n      \"{}\"",
                one.comment.line, one.sense.overlap, one.sense.words, one.comment.text
            ),
        }
    }
}

fn print_json(failures: &[Out], suspects: &[Out]) {
    let rows: Vec<String> = failures
        .iter()
        .chain(suspects.iter())
        .map(|out| {
            format!(
                "{{\"file\":{},\"line\":{},\"kind\":{},\"under\":{}}}",
                quote(&out.file),
                out.line,
                quote(&out.kind),
                quote(&out.under)
            )
        })
        .collect();
    println!("[\n  {}\n]", rows.join(",\n  "));
}

/// The files the working tree has, read from disk.
fn disk_units(root: &Path, roots: &[String]) -> Vec<(String, String)> {
    sources(root, roots)
        .into_iter()
        .map(|file| {
            let src = std::fs::read_to_string(root.join(&file)).unwrap_or_default();
            (file, src)
        })
        .collect()
}

/// The lines this change touched, per file, from `git diff --unified=0`. A hunk header names the range
/// in the new file; a pure deletion touches nothing a comment can sit on, and is skipped.
fn changed_lines(root: &Path, base: &str, roots: &[String]) -> HashMap<String, BTreeSet<usize>> {
    let mut map: HashMap<String, BTreeSet<usize>> = HashMap::new();
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .args(["diff", "--unified=0", base, "--"])
        .args(roots);
    let Ok(out) = cmd.output() else { return map };
    if !out.status.success() {
        eprintln!(
            "git diff against `{base}` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        std::process::exit(2);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut file = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            file = rest.to_string();
        } else if line.starts_with("@@") {
            // @@ -a,b +c,d @@ — c and d are the line and count in the new file.
            if let Some(plus) = line.split(' ').find(|p| p.starts_with('+')) {
                let mut it = plus.trim_start_matches('+').split(',');
                let start: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                let count: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
                if start > 0 && count > 0 {
                    let set = map.entry(file.clone()).or_default();
                    for l in start..start + count {
                        set.insert(l);
                    }
                }
            }
        }
    }
    map
}

/// A JSON string, enough of one for the fields above: quotes, backslashes and controls are escaped.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
