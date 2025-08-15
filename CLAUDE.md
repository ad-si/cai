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

# Run the current version with a specific subcommand
cargo run -- value 'age of universe'
```


## Code Style Guidelines

- **Formatting/Linting**: Run `make format` for automatic formatting and linting
- **Documentation**: Add comments for public API, CLI commands need detailed help text
- **CLI**: Use clap for argument parsing with well-formatted help messages
- **APIs**: Use async/await with Tokio runtime for API interactions
