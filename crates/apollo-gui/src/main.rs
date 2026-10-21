//! apollo-gui: native GPUI client (skeleton).
//!
//! The monorepo split lands first; the GPUI app (chat list, streaming
//! renderer, tool-call cards, Editor/DiffView via gpui-kit) is built on top
//! of `apollo-core` next. This stub keeps `cargo check --workspace` green.

use apollo_core::harness::HarnessConfig;

fn main() {
    let cfg = HarnessConfig::default();
    println!("apollo-gui: skeleton — GPUI app not yet implemented.");
    println!("harness: {}", cfg.summary());
    println!("cargo features: {}", cfg.cargo_features().join(", "));
}
