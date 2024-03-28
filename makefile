.PHONY: help
help: makefile
	@tail -n +4 makefile | grep ".PHONY"


demo.gif: demo.tape
	vhs $<


.PHONY: test
test:
	cargo test -- --show-output


.PHONY: install
install:
	cargo install --path .
