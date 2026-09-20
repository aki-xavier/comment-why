// sense.rs — a local approximation of the question a model would be asked: does this comment say
// something the code below it does not?
//
// WHY AN APPROXIMATION IS THE HONEST NAME FOR IT. Whether a sentence says something is a reading, and
// there is no offline function that reads. What there is, is two signals that a reader would also use,
// both computable from the words already at hand:
//
//   MARKERS   a rationale lexeme — a cause, a constraint, a contrast, a purpose, a consequence — is
//             what a why-comment is made of. A comment carrying one is saying something the code
//             cannot, whatever else it says.
//   OVERLAP   the share of the comment's own vocabulary that the code below already uses. A long
//             comment whose content words are the code's own words is the statement said again.
//
// Neither signal decides anything alone, so neither is allowed to: a comment is only ever called a
// suspect when it is long enough to carry a claim, carries no marker, and mostly repeats the code.
// Everything else is left to a reader, which is where the judgement belonged all along.
//
// The thresholds are dials, not truths. They were set against this family's own sources (a comment
// block runs past eight words far more often than not, so length alone is not a signal), and a project
// that disagrees turns them, or ignores the verdict: `Suspect` is advice and never a failure.

use crate::{words, RATIONALE, STOPWORD};

/// How long a comment has to be before "it says nothing" is worth saying. Below this it is a label or
/// a unit, and a label is allowed to be short.
const LABEL_WORDS: usize = 8;

/// The share of a comment's vocabulary that the code below may already use before the comment reads as
/// the statement said again. Half is deliberately generous: prose reuses the code's nouns on purpose.
const SUSPECT_OVERLAP: f64 = 0.5;

/// Words that name an effect without naming a cause: the code does not say what it costs, breaks,
/// turns or forces, and a comment that does is saying something.
const CONSEQUENCE: &[&str] = &[
    "breaks",
    "buys",
    "costs",
    "counts",
    "drives",
    "fails",
    "forces",
    "frees",
    "leaves",
    "makes",
    "matters",
    "overstates",
    "owes",
    "pays",
    "produces",
    "silently",
    "stale",
    "turns",
    "understates",
];

/// A statement about an edge — what the code does at zero, at an unknown name, at a negative rate — is
/// saying something too, and usually without a marker word: the happy path needs no comment, the edge
/// does. Comparisons count, and so do the few words that only make sense at one.
const EDGE: &[&str] = &[
    "bit for bit",
    "clamps",
    "empty",
    "gives",
    "invalid",
    "unchanged",
    "unknown",
];

fn has_edge(text: &str) -> bool {
    let padded = padded(text);
    ["<=", ">=", "=="].iter().any(|op| text.contains(op))
        || EDGE.iter().any(|m| padded.contains(&format!(" {m} ")))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// It carries a rationale: a why.
    Why,
    /// Long, marker-free, and mostly the code's own words: worth a reader's eye.
    Suspect,
    /// Too short to judge: a label, a unit, a name.
    Unclear,
}

impl Verdict {
    pub fn name(self) -> &'static str {
        match self {
            Verdict::Why => "why",
            Verdict::Suspect => "suspect",
            Verdict::Unclear => "unclear",
        }
    }
}

/// What the approximation saw, with the numbers behind it, so a reader can overrule it.
pub struct Sense {
    pub verdict: Verdict,
    /// Rationale markers the comment carries.
    pub markers: usize,
    /// Share of the comment's content words that the code below also uses, rounded to two places.
    pub overlap: f64,
    /// Content words the comment has, after the function words are dropped.
    pub words: usize,
}

/// Read one comment against the code below it.
pub fn sense(text: &str, code: &str) -> Sense {
    let content: Vec<String> = words(&text.replace(['\'', '\u{2019}'], ""))
        .into_iter()
        .filter(|w| !STOPWORD.contains(&w.as_str()))
        .collect();
    let markers = RATIONALE
        .iter()
        .filter(|m| padded(text).contains(&format!(" {m} ")))
        .count()
        + CONSEQUENCE
            .iter()
            .filter(|m| padded(text).contains(&format!(" {m} ")))
            .count();
    let edge = has_edge(text);
    let code_words: Vec<String> = words(code).into_iter().map(|w| stem(&w)).collect();
    let seen = content
        .iter()
        .map(|w| stem(w))
        .filter(|w| code_words.contains(w))
        .count();
    let overlap = if content.is_empty() {
        0.0
    } else {
        seen as f64 / content.len() as f64
    };

    let verdict = if content.len() <= LABEL_WORDS {
        Verdict::Unclear
    } else if markers > 0 || edge {
        Verdict::Why
    } else if overlap >= SUSPECT_OVERLAP {
        Verdict::Suspect
    } else {
        Verdict::Unclear
    };
    Sense {
        verdict,
        markers,
        overlap: (overlap * 100.0).round() / 100.0,
        words: content.len(),
    }
}

/// A crude stem, and deliberately crude: `updates` and `update` have to be the same word for the
/// overlap to mean anything, and `fill` and `filled` may as well not be. English is not the point here.
fn stem(word: &str) -> String {
    if let Some(base) = word.strip_suffix("ing").filter(|b| b.len() >= 3) {
        return base.to_string();
    }
    if let Some(base) = word.strip_suffix("ed").filter(|b| b.len() >= 3) {
        return format!("{base}e");
    }
    if let Some(base) = word
        .strip_suffix('s')
        .filter(|b| b.len() >= 3 && !b.ends_with('s'))
    {
        return base.to_string();
    }
    word.to_string()
}

/// The comment as a padded word stream, so a marker matches a whole word and not the middle of one.
fn padded(text: &str) -> String {
    let mut s = String::from(" ");
    for w in words(&text.replace(['\'', '\u{2019}'], "")) {
        s.push_str(&w);
        s.push(' ');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_reads_as_a_why() {
        let s = sense(
            "the bound is one no command reaches, so the machine reads as welded",
            "let u = 1e12;",
        );
        assert_eq!(
            s.verdict,
            Verdict::Why,
            "markers {} overlap {}",
            s.markers,
            s.overlap
        );
    }

    #[test]
    fn a_long_comment_made_of_the_codes_words_is_a_suspect() {
        let s = sense(
            "the metric refresh reads the configuration stamp and updates the cached shaping matrix",
            "fn refresh(metric: &mut Metric, stamp: u64, shaping: &mut Mat) { metric.refresh(stamp); shaping.update(stamp); }",
        );
        assert_eq!(
            s.verdict,
            Verdict::Suspect,
            "markers {} overlap {} words {}",
            s.markers,
            s.overlap,
            s.words
        );
    }

    #[test]
    fn a_short_label_is_left_alone() {
        let s = sense("the joint COLUMNS of a task Jacobian", "base_rows: usize,");
        assert_eq!(s.verdict, Verdict::Unclear, "words {}", s.words);
    }

    #[test]
    fn a_consequence_reads_as_a_why() {
        let s = sense(
            "A held base's rows are written by the loop all the same, and are the wrench it owes.",
            "self.base_rows = if s.base_is_a_state() { s.base_dof } else { 0 };",
        );
        assert_eq!(
            s.verdict,
            Verdict::Why,
            "markers {} overlap {}",
            s.markers,
            s.overlap
        );
    }

    #[test]
    fn an_edge_stated_without_a_marker_still_reads_as_a_why() {
        let s = sense(
            "2 percent settling-time bound wn = 4/(zeta ts); ts <= 0 or zeta <= 0 gives 0.",
            "if ts <= 0.0 || zeta <= 0.0 { return 0.0; }",
        );
        assert_eq!(
            s.verdict,
            Verdict::Why,
            "markers {} overlap {}",
            s.markers,
            s.overlap
        );
    }

    #[test]
    fn prose_that_is_not_the_codes_words_is_left_alone() {
        let s = sense(
            "A plant is free to answer something else during the hold, whatever the loop asked for.",
            "self.base_rows = if s.base_is_a_state() { s.base_dof } else { 0 };",
        );
        assert_eq!(
            s.verdict,
            Verdict::Unclear,
            "markers {} overlap {}",
            s.markers,
            s.overlap
        );
    }
}
