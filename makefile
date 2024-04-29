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


.PHONY: install
install: update-readme
	cargo install --path .
