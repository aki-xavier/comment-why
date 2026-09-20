# comment-why — the WHY-ONLY comment rules: a library and a command line.
#
#   make test    # the rules' own tests, then the rules over this project's own sources
#   make lint    # rustfmt and clippy over every target, then the same
#   make comments# the rules alone, with the local approximation for the comments they cannot decide
#
# `lint` is rustfmt, then clippy, then the same rules. Nothing here needs a toolchain beyond stable and
# nothing needs a dependency — the rules read text. Another project adopts it in two lines; see the
# README.

.PHONY: test lint comments

test:
	cargo test
	$(MAKE) comments

lint:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	$(MAKE) comments

comments:
	cargo run --quiet --bin comment-why -- --review
