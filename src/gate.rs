// gate.rs — the WHY-ONLY rule as a project's own test: the walk over that project's tree, the
// verdicts, and the exceptions it records beside them.
//
// WHY THIS IS A LIBRARY AND NOT A FILE PER PROJECT. Every project runs the same walk and the same
// reporting; what differs between them is DATA — the roots their prose lives in, the floor their
// corpus has to clear, and the comments they keep anyway. A copy of that walk in each repository is a
// copy that can drift from the rules it is supposed to hold that project to, and the drift is silent:
// the gate keeps passing while measuring something else. So the walk lives here once, and a project's
// own test is a few lines that state its three pieces of data and nothing else.
//
// WHAT IT CANNOT DO is unchanged by being shared: it decides three shapes and suspects the rest (see
// `lib.rs` and `sense.rs`). A suspect is never a failure, and `make comments` is where it is printed.

use crate::{comments, line_of, scan, sources, words_of};
use std::path::PathBuf;

/// One project's gate: where its prose is, what it keeps anyway, and how small a corpus is too small
/// to have been read.
pub struct Gate<'a> {
    root: PathBuf,
    roots: Vec<String>,
    tolerated: &'a [(&'a str, &'a str, &'a str)],
    min_files: usize,
    min_blocks: usize,
}

impl<'a> Gate<'a> {
    /// A gate over the project rooted at `root` — a project's own test passes its `CARGO_MANIFEST_DIR`,
    /// so the walk can be pointed at that project and no other. `roots` are the directories under it
    /// its prose lives in, and `tolerated` the comments it keeps: (a tail of the file's path, the
    /// comment's own words, the reason it stays).
    pub fn new(
        root: PathBuf,
        roots: &[&str],
        tolerated: &'a [(&'a str, &'a str, &'a str)],
    ) -> Gate<'a> {
        Gate {
            root,
            roots: roots.iter().map(|r| r.to_string()).collect(),
            tolerated,
            min_files: 0,
            min_blocks: 0,
        }
    }

    /// The floor the walk has to clear, in source files and comment blocks. A walk that found the wrong
    /// tree has to fail LOUDLY rather than pass on an empty corpus — that is the one failure mode a
    /// comment rule cannot report from its own findings, since finding nothing is what it wants.
    pub fn at_least(mut self, files: usize, blocks: usize) -> Gate<'a> {
        self.min_files = files;
        self.min_blocks = blocks;
        self
    }

    /// Hold the project to the rule: `Ok` when every comment says something the code under it does
    /// not, `Err` with the whole report when one does not — or when the walk or a recorded exception is
    /// itself wrong, which is checked first because a gate reading the wrong tree says nothing.
    pub fn check(&self) -> Result<(), String> {
        let files = sources(&self.root, &self.roots);
        let mut report = String::new();
        let mut blocks = 0usize;
        let mut flagged = 0usize;

        for file in &files {
            let src = self.read(file);
            blocks += comments(&src).len();
            for finding in scan(&src) {
                if kept(self.tolerated, file, &finding.text) {
                    continue;
                }
                flagged += 1;
                report.push_str(&format!(
                    "{}:{}  {}\n      \"{}\"\n      {}: {}\n      under it: {}\n",
                    file,
                    line_of(&src, finding.start),
                    finding.rule.name(),
                    finding.text,
                    finding.rule.name(),
                    finding.rule.hint(),
                    finding.under
                ));
            }
        }

        self.the_walk_reached_the_project(&files, blocks)?;
        self.every_record_still_matters(&files)?;

        if flagged > 0 {
            return Err(format!(
                "{flagged} comment(s) say what the code already says:\n\n{report}\n\
                 Reword each one to state why the line is there, or delete it. A comment the rule \
                 should keep as it is goes in this project's TOLERATED with the reason it stays."
            ));
        }
        Ok(())
    }

    /// The walk really read the project: every root it names held sources, and the corpus cleared the
    /// floor the project stated. Read before the verdicts, because a gate on the wrong tree reports
    /// nothing and looks like a pass.
    fn the_walk_reached_the_project(&self, files: &[String], blocks: usize) -> Result<(), String> {
        for root in &self.roots {
            let prefix = format!("{root}/");
            if !files.iter().any(|f| f.starts_with(&prefix)) {
                return Err(format!(
                    "the walk lost {root}/ entirely, so this gate is reading the wrong tree"
                ));
            }
        }
        if files.len() < self.min_files {
            return Err(format!(
                "the walk found only {} sources, so this gate is reading the wrong tree",
                files.len()
            ));
        }
        if blocks < self.min_blocks {
            return Err(format!(
                "the walk found only {blocks} comment blocks, so this gate is reading nothing"
            ));
        }
        Ok(())
    }

    /// Every recorded exception still names a comment that exists and still needs keeping: a record
    /// whose comment is gone, or which the rules now leave alone, is dead weight that would silently
    /// excuse a future comment.
    fn every_record_still_matters(&self, files: &[String]) -> Result<(), String> {
        for (file, text, reason) in self.tolerated {
            if words_of(reason).len() < 4 {
                return Err(format!(
                    "{file}: \"{text}\" needs a reason a reader can weigh, not \"{reason}\""
                ));
            }
            let mut present = false;
            let mut flagged = false;
            for f in files.iter().filter(|f| f.ends_with(file)) {
                let src = self.read(f);
                present |= comments(&src)
                    .iter()
                    .any(|c| kept(self.tolerated, file, &c.text));
                flagged |= scan(&src)
                    .iter()
                    .any(|finding| kept(self.tolerated, file, &finding.text));
            }
            if !present {
                return Err(format!(
                    "{file}: the record \"{text}\" matches no comment any more — delete it"
                ));
            }
            if !flagged {
                return Err(format!(
                    "{file}: \"{text}\" escapes every rule now, so the record is dead weight — delete it"
                ));
            }
        }
        Ok(())
    }

    fn read(&self, file: &str) -> String {
        let path = self.root.join(file);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }
}

/// Whether a record keeps this comment: the file's tail and the comment's words, and nothing about
/// where it sits — so an edit above a comment cannot drop its record.
fn kept(tolerated: &[(&str, &str, &str)], file: &str, text: &str) -> bool {
    let said = words_of(text).join(" ");
    tolerated
        .iter()
        .any(|(f, kept, _)| file.ends_with(f) && words_of(kept).join(" ") == said)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A scratch project, emptied first: a test's tree has to hold nothing but what that test wrote.
    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("comment_why_gate_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("create scratch src");
        root
    }

    fn write(root: &Path, file: &str, text: &str) {
        std::fs::write(root.join(file), text).expect("write scratch file");
    }

    /// A why-comment and a what-comment, side by side: one gate has to tell them apart.
    const WHY: &str = "fn hold(u: f64) -> f64 {\n    // the bound is one no command reaches, so the machine reads as welded\n    let u = u;\n    u\n}\n";
    const WHAT: &str = "fn tick() {\n    let mut i = 0;\n    // increment i\n    i += 1;\n}\n";

    #[test]
    fn a_project_whose_comments_say_why_passes() {
        let root = scratch("clean");
        write(&root, "src/lib.rs", WHY);
        let gate = Gate::new(root, &["src"], &[]).at_least(1, 1);
        assert!(gate.check().is_ok(), "{:?}", gate.check());
    }

    #[test]
    fn a_what_comment_fails_with_the_line_that_did_it() {
        let root = scratch("flagged");
        write(&root, "src/lib.rs", WHAT);
        let report = Gate::new(root, &["src"], &[])
            .at_least(1, 1)
            .check()
            .unwrap_err();
        assert!(
            report.contains("RESTATEMENT") && report.contains("increment i"),
            "the report has to name the shape and the comment: {report}"
        );
    }

    #[test]
    fn a_recorded_exception_keeps_the_comment_it_names() {
        let root = scratch("tolerated");
        write(&root, "src/lib.rs", WHAT);
        let tolerated: &[(&str, &str, &str)] = &[(
            "src/lib.rs",
            "increment i",
            "the loop counter is the fixture this test is about",
        )];
        assert!(Gate::new(root, &["src"], tolerated)
            .at_least(1, 1)
            .check()
            .is_ok());
    }

    #[test]
    fn a_record_that_matches_nothing_fails() {
        let root = scratch("stale-record");
        write(&root, "src/lib.rs", WHAT);
        let tolerated: &[(&str, &str, &str)] = &[(
            "src/lib.rs",
            "a comment that is not here",
            "kept for a reason that has expired",
        )];
        let report = Gate::new(root, &["src"], tolerated)
            .at_least(1, 1)
            .check()
            .unwrap_err();
        assert!(report.contains("matches no comment any more"), "{report}");
    }

    #[test]
    fn a_walk_that_missed_the_tree_fails_rather_than_passing_on_nothing() {
        let root = scratch("empty");
        let report = Gate::new(root, &["src"], &[])
            .at_least(1, 1)
            .check()
            .unwrap_err();
        assert!(report.contains("reading the wrong tree"), "{report}");
        let root = scratch("short-corpus");
        write(&root, "src/lib.rs", WHY);
        let report = Gate::new(root, &["src"], &[])
            .at_least(5, 1)
            .check()
            .unwrap_err();
        assert!(report.contains("reading the wrong tree"), "{report}");
    }

    #[test]
    fn a_root_the_walk_never_reached_fails() {
        let root = scratch("lost-root");
        write(&root, "src/lib.rs", WHY);
        let report = Gate::new(root, &["src", "tests"], &[])
            .at_least(1, 1)
            .check()
            .unwrap_err();
        assert!(report.contains("lost tests/ entirely"), "{report}");
    }
}
