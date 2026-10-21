//! apollo-core: shared agent logic (LLM client, models, tools, harness manifest).
//!
//! This crate is UI-agnostic. Both `apollo-cli` (TUI) and `apollo-gui` (GPUI)
//! depend on it. Every harness capability is a Cargo feature so the future
//! `apollo-wizard` TUI can compose a binary with exactly the features the
//! user picks (mcp, subagents, plan-mode, individual tools, ...).

pub mod client;
pub mod harness;
pub mod models;
pub mod tools;
