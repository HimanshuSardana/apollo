//! Harness manifest: the serializable description of "what's in my agent binary".
//!
//! This is the contract the future `apollo-wizard` TUI will produce. The user
//! picks capabilities (tools, mcp, subagents, plan-mode) and the wizard maps
//! the manifest to `apollo-core` Cargo features, then builds a binary with
//! exactly those features.
//!
//! Keeping this in `apollo-core` (not the CLI) means CLI, GUI and wizard all
//! agree on feature names.

use serde::{Deserialize, Serialize};

/// Full harness configuration. Serialize to JSON/TOML for `apollo init`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessConfig {
    #[serde(default = "default_tools")]
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp: bool,
    #[serde(default)]
    pub subagents: bool,
    #[serde(default)]
    pub plan_mode: bool,
}

fn default_tools() -> Vec<String> {
    vec![
        "read".to_string(),
        "write".to_string(),
        "edit".to_string(),
        "bash".to_string(),
    ]
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            tools: default_tools(),
            mcp: false,
            subagents: false,
            plan_mode: false,
        }
    }
}

impl HarnessConfig {
    /// Minimal harness: no tools, no extras. Wizard starts here.
    pub fn minimal() -> Self {
        Self {
            tools: Vec::new(),
            mcp: false,
            subagents: false,
            plan_mode: false,
        }
    }

    /// Everything currently implemented. Note: `mcp` / `subagents` /
    /// `plan-mode` are feature flags with stub modules until implemented;
    /// enabling them compiles the (currently empty) module so the wizard can
    /// already round-trip them.
    pub fn full() -> Self {
        Self {
            tools: crate::tools::ALL_TOOLS.iter().map(|s| s.to_string()).collect(),
            mcp: true,
            subagents: true,
            plan_mode: true,
        }
    }

    /// Map this manifest to `apollo-core` Cargo features for
    /// `cargo build -p apollo-cli --no-default-features --features <...>`
    /// (or the same for `apollo-gui`).
    pub fn cargo_features(&self) -> Vec<String> {
        let mut feats: Vec<String> = self.tools.iter().map(|t| format!("tool-{t}")).collect();
        if self.mcp {
            feats.push("mcp".to_string());
        }
        if self.subagents {
            feats.push("subagents".to_string());
        }
        if self.plan_mode {
            feats.push("plan-mode".to_string());
        }
        feats.sort();
        feats.dedup();
        feats
    }

    /// Parse back what `cargo_features()` produced (wizard preview round-trip).
    pub fn from_features(features: &[String]) -> Self {
        let mut cfg = Self::minimal();
        for f in features {
            if let Some(tool) = f.strip_prefix("tool-") {
                if crate::tools::ALL_TOOLS.contains(&tool) && !cfg.tools.contains(&tool.to_string())
                {
                    cfg.tools.push(tool.to_string());
                }
            } else if f == "mcp" {
                cfg.mcp = true;
            } else if f == "subagents" {
                cfg.subagents = true;
            } else if f == "plan-mode" {
                cfg.plan_mode = true;
            }
        }
        cfg.tools.sort();
        cfg
    }

    pub fn summary(&self) -> String {
        let tools = if self.tools.is_empty() {
            "no tools".to_string()
        } else {
            format!("tools: {}", self.tools.join(", "))
        };
        let mut extras = Vec::new();
        if self.mcp {
            extras.push("mcp");
        }
        if self.subagents {
            extras.push("subagents");
        }
        if self.plan_mode {
            extras.push("plan-mode");
        }
        if extras.is_empty() {
            tools
        } else {
            format!("{tools} + {}", extras.join(" + "))
        }
    }
}

#[cfg(feature = "mcp")]
pub mod mcp {
    //! Stub: Model Context Protocol servers. Not implemented yet.
    //! The feature flag exists so the wizard can already emit `--features mcp`.
}

#[cfg(feature = "subagents")]
pub mod subagents {
    //! Stub: subagent spawning. Not implemented yet.
}

#[cfg(feature = "plan-mode")]
pub mod plan_mode {
    //! Stub: plan-then-execute mode. Not implemented yet.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_roundtrip() {
        let cfg = HarnessConfig {
            tools: vec!["bash".to_string(), "read".to_string()],
            mcp: true,
            subagents: false,
            plan_mode: true,
        };
        let feats = cfg.cargo_features();
        assert_eq!(feats, vec!["mcp", "plan-mode", "tool-bash", "tool-read"]);
        assert_eq!(HarnessConfig::from_features(&feats), cfg);
    }

    #[test]
    fn full_includes_every_tool() {
        let full = HarnessConfig::full();
        for t in crate::tools::ALL_TOOLS {
            assert!(full.tools.contains(&t.to_string()), "missing {t}");
        }
    }

    #[test]
    fn unknown_features_ignored() {
        let cfg = HarnessConfig::from_features(&["nope".to_string(), "tool-read".to_string()]);
        assert_eq!(cfg.tools, vec!["read"]);
    }
}
