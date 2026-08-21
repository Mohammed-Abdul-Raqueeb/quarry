# Contributing

## Getting started

```bash
git clone https://github.com/Mohammed-Abdul-Raqueeb/quarry.git
cd quarry
cargo test
```

Nothing to install and nothing to download — there are no dependencies, which is also the first
constraint below.

Before opening a pull request:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

That is exactly what CI runs, so a clean local check means a green run.
