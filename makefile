.PHONY: help
help: makefile
	@tail -n +4 makefile | grep ".PHONY"


demo.gif: demo.tape
	vhs $<


source_files=$(shell find src -type f)
.INTERMEDIATE: usage.txt
usage.txt: $(source_files)
	cargo run -- help > usage.txt


.PHONY: update-readme
update-readme: usage.txt
	sd --flags s \
		'cai help.+\`\`\`' \
		"cai help\n$$(cat $<)\n\`\`\`" \
		readme.md


.PHONY: format
format:
	cargo clippy --fix --allow-dirty
	cargo fmt
	# nix fmt  # TODO: Reactivate when it's faster


.PHONY: test-units
test-units:
	cargo test --lib --bins -- --show-output
	@echo "✅ All unit tests passed!\n\n"


.PHONY: build
build:
	cargo build


.PHONY: test-cli
test-cli:
	cargo test --test integration_tests


.PHONY: test
test: format test-units update-readme


.PHONY: release
release:
	@echo '1. `cai changelog <first-commit-hash>`'
	@echo '2. `git add ./changelog.md && git commit -m "Update changelog"`'
	@echo '3. `cargo release major / minor / patch`'
	@echo '4. Create a new GitHub release at https://github.com/ad-si/cai/releases/new'
	@echo -e \
		"5. Announce release on \n" \
		"   - https://x.com \n" \
		"   - https://bsky.app \n" \
		"   - https://this-week-in-rust.org \n" \
		"   - https://news.ycombinator.com \n" \
		"   - https://lobste.rs \n" \
		"   - Reddit \n" \
		"     - https://reddit.com/r/rust \n" \
		"     - https://reddit.com/r/ChatGPT \n" \
		"     - https://reddit.com/r/ArtificialInteligence \n" \
		"     - https://reddit.com/r/artificial \n"


.PHONY: install
install: update-readme
	cargo install --path .
