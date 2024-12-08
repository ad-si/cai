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


.PHONY: test-units
test-units:
	cargo test --lib --bins -- --show-output
	@echo "✅ All unit tests passed!\n\n"


.PHONY: test-cli
test-cli:
	cargo test --test integration_tests


.PHONY: test
test: test-units update-readme


.PHONY: release
release:
	@echo '1. Update ./changelog.md with `cai changelog <commit-hash>`'
	@echo '2. Run `cargo release major / minor / patch`'
	@echo '3. Announce release on https://x.com and https://bsky.app'


.PHONY: install
install: update-readme
	cargo install --path .
