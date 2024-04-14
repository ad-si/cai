.PHONY: help
help: makefile
	@tail -n +4 makefile | grep ".PHONY"


demo.gif: demo.tape
	vhs $<


source_files=$(shell find src -type f)
.INTERMEDIATE: usage.txt
usage.txt: $(source_files)
	cargo run -- help > usage.txt


readme.md: usage.txt
	sd --flags s \
		'cai help.+\`\`\`' \
		"cai help\n$$(cat $<)\n\`\`\`" \
		$@


.PHONY: test-rust
test-rust:
	cargo test -- --show-output


.PHONY: test
test: test-rust readme.md


.PHONY: install
install:
	cargo install --path .
