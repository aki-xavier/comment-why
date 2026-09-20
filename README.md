# comment-why

A comment has to say something the code under it does not. This is that rule as a library and a command
line: three shapes it can decide from the text, and a local approximation of the question it cannot.

No dependencies, and no compiler: the rules read a file, so this runs on the toolchain a project
already has, from a test, from `make`, or from `git`.

## What it decides, and what it only suspects

Decided, and a finding fails:

| shape | what it is |
| --- | --- |
| `NARRATION` | first-person and step-by-step process, and the filler that carries no claim |
| `RESTATEMENT` | a comment line whose content words are all in the code below it |
| `DEFINITION` | a short doc comment that opens by re-saying the item's own name |

Approximated, and printed only under `--review`: a comment long enough to carry a claim, carrying no
rationale marker, and mostly using the code's own vocabulary is a **suspect**. `sense.rs` states the two
signals and their dials, and neither is allowed to decide alone — a suspect never fails anything unless
`--strict-suspect` asks it to. That is the local stand-in for asking a model, and it is honest about
being one.

## Adopt it in a project

1. Depend on it, from wherever it sits beside that project:

   ```toml
   [dev-dependencies]
   comment-why = { path = "../comment-why" }
   ```

2. Lint at `make test` time — the automatic place, because it is what a project already runs:

   ```make
   COMMENT_WHY ?= ../comment-why

   test:
   	mbx test
   	$(MAKE) comments

   comments:
   	mbx run --quiet --manifest-path $(COMMENT_WHY)/Cargo.toml --bin comment-why -- --review
   ```

   `mbx` is this family's Cargo build-cache wrapper; plain `cargo` runs the same line.

3. Or as a gate inside the suite, which is what `control-ga-pid` does: a test that walks the project's
   roots and fails on a finding, with the project's own exceptions recorded beside it — see
   `../control-ga-pid/tests/comment_why.rs`.

## The command line

```
comment-why [ROOTS...] [--changed-only [--base REF]] [--review] [--review-all]
            [--strict-suspect] [--limit N] [--format json|text] [--root DIR]
```

`ROOTS` default to `src tests examples`. The exit is 1 when a comment says what the code already says;
`--review` adds the suspects (and `--review-all` every verdict), `--strict-suspect` makes a suspect
fail too, and `--changed-only --base origin/main` keeps a run to the comments a change touched.

## Reading a verdict

```
src/window.rs:12  suspect (overlap 0.62, 14 words)
      "the metric refresh reads the configuration stamp and updates the cached shaping matrix"
```

`overlap` is the share of the comment's content words that the code below already uses, after a crude
stemming so that `updates` and `update` are the same word. The dials are in `sense.rs`: eight words
before a comment is long enough to judge, and half its vocabulary before it reads as the code's own.
They are dials, not truths — turn them, or ignore the verdict.

## What it cannot do

It does not read. A paraphrase in different words is not a restatement to a word set; a comment that
describes behaviour the code no longer has is invisible to both signals; and a comment that QUOTES a
banned phrase is read as using it. The decided shapes are the part a gate can honestly enforce, and the
approximation is the part that admits it is one.

Run over one real codebase (the GA-PID crate: 297 comment blocks, no findings), the approximation
printed two suspects — the mathematics a solver implements, which is a true reading of "the statement
again", and a property stated as "bounded by … monotone, no overshoot", which is not. That is the
honest shape of a lexical approximation: a place to look, occasionally a place to look away.

## This project's own targets

```
make test    # the tests, then the rules over this project's own sources
make lint    # rustfmt, clippy, then the same
make comments# the rules alone, with the approximation
```
