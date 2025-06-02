# Guide for Working with CAI Codebase

## Build and Test Commands

```bash
# Build the project
make build

# Run all tests
make test

# Run unit tests only
cargo test --lib --bins -- --show-output

# Run integration tests
cargo test --test integration_tests

# Run a specific test
cargo test test_name -- --show-output

# Install locally
make install
```


## Code Style Guidelines

- **Formatting**: Use `cargo fmt` for formatting, `cargo clippy` for linting
- **Documentation**: Add comments for public API, CLI commands need detailed help text
- **CLI**: Use clap for argument parsing with well-formatted help messages
- **APIs**: Use async/await with Tokio runtime for API interactions
