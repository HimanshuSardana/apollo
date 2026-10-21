# Apollo monorepo
#
#   make build      — debug build of everything
#   make cli        — run the TUI  (cargo run -p apollo-cli)
#   make gui        — run the GUI stub (cargo run -p apollo-gui)
#   make check      — cargo check --workspace --all-targets
#   make test       — cargo test --workspace
#   make release    — stripped release binaries (see .sizes)
#   make sizes      — du -h release binaries
#   make minimal    — example minimal harness build (read+bash only, no extras)
#   make full       — full harness build (all tools + mcp + subagents + plan-mode)
#   make wizard     — placeholder for the future `apollo init` TUI wizard
#   make help       — this list

CLI_FEATURES ?= full
GUI_FEATURES ?= full

.PHONY: build cli gui check test release sizes minimal full wizard help

build:
	cargo build --workspace

cli:
	cargo run -p apollo-cli --features $(CLI_FEATURES)

gui:
	cargo run -p apollo-gui --features $(GUI_FEATURES)

check:
	cargo check --workspace --all-targets

test:
	cargo test --workspace

release:
	cargo build --release --workspace

sizes: release
	ls -lh target/release/apollo target/release/apollo-gui
	du -h target/release/apollo target/release/apollo-gui

# Example: minimal harness binary — only read+bash, none of mcp/subagents/plan-mode.
# The wizard will generate exactly this kind of flag set from HarnessConfig::cargo_features().
minimal:
	cargo build --release -p apollo-cli --no-default-features --features apollo-core/tool-read,apollo-core/tool-bash

full:
	cargo build --release -p apollo-cli --features full
	cargo build --release -p apollo-gui --features full

# Future: TUI wizard (`apollo init`) — pick mcp/subagents/plan-mode/tools,
# get a binary with exactly that. See crates/apollo-core/src/harness.rs.
wizard:
	@echo "wizard not yet implemented — see apollo-core::harness::HarnessConfig"
	@echo "preview default harness:"
	@cargo run -q -p apollo-gui --features full

help:
	@grep -E '^#   make' Makefile
