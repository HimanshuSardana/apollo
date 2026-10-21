pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Vendor grouping, used by pickers (CLI + GUI) to section the catalog.
    pub provider: &'static str,
    pub efforts: &'static [&'static str],
}

pub const DEFAULT_MODEL: &str = "deepseek-v4.1-flash";

pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "deepseek-v4.1-flash",
        name: "DeepSeek V4.1 Flash",
        provider: "DeepSeek",
        efforts: &["off", "low", "high", "max"],
    },
    ModelSpec {
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        provider: "DeepSeek",
        efforts: &["off", "high", "max"],
    },
    ModelSpec {
        id: "glm-5.3",
        name: "GLM-5.3",
        provider: "Z.ai",
        efforts: &["off", "low", "high", "max"],
    },
    ModelSpec {
        id: "glm-5.3-flash",
        name: "GLM-5.3-Flash",
        provider: "Z.ai",
        efforts: &["off", "low", "high", "max"],
    },
    ModelSpec {
        id: "glm-5.2",
        name: "GLM-5.2",
        provider: "Z.ai",
        efforts: &["off", "high", "max"],
    },
    ModelSpec {
        id: "kimi-k3",
        name: "Kimi K3",
        provider: "Moonshot",
        efforts: &["off", "max"],
    },
    ModelSpec {
        id: "kimi-k2.7-code",
        name: "Kimi K2.7 Code",
        provider: "Moonshot",
        efforts: &["off"],
    },
    ModelSpec {
        id: "hy3",
        name: "Hy3",
        provider: "Tencent",
        efforts: &["off", "low", "high"],
    },
    ModelSpec {
        id: "hy4-preview",
        name: "Hy4 Preview",
        provider: "Tencent",
        efforts: &["off", "high"],
    },
    ModelSpec {
        id: "cat-jam-free",
        name: "Cat Jam Free",
        provider: "OpenCode",
        efforts: &["off", "low", "medium", "high", "max"],
    },
    ModelSpec {
        id: "space-bunny",
        name: "Space Bunny",
        provider: "OpenCode",
        efforts: &["off", "low", "medium", "high", "max"],
    },
    ModelSpec {
        id: "step-5-preview-free",
        name: "Step 5 Preview Free",
        provider: "StepFun",
        efforts: &["off", "low", "medium", "high"],
    },
    ModelSpec {
        id: "longcat-2.0",
        name: "LongCat-2.0",
        provider: "LongCat",
        efforts: &["off"],
    },
    ModelSpec {
        id: "longcat-2.5-preview-free",
        name: "LongCat 2.5 Preview Free",
        provider: "LongCat",
        efforts: &["off"],
    },
    ModelSpec {
        id: "mimo-v2.6-pro",
        name: "MiMo-V2.6-Pro",
        provider: "Xiaomi",
        efforts: &["off"],
    },
    ModelSpec {
        id: "mimo-v2.6-flash",
        name: "MiMo-V2.6-Flash",
        provider: "Xiaomi",
        efforts: &["off"],
    },
    ModelSpec {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro",
        provider: "Xiaomi",
        efforts: &["off"],
    },
    ModelSpec {
        id: "mimo-v2.5",
        name: "MiMo V2.5",
        provider: "Xiaomi",
        efforts: &["off"],
    },
];

/// Distinct providers in catalog order, for grouped pickers.
pub fn providers() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for m in CATALOG {
        if !out.contains(&m.provider) {
            out.push(m.provider);
        }
    }
    out
}

/// Models belonging to `provider`, preserving catalog order.
pub fn models_for(provider: &str) -> Vec<&'static ModelSpec> {
    CATALOG.iter().filter(|m| m.provider == provider).collect()
}

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn index_of(id: &str) -> Option<usize> {
    CATALOG.iter().position(|m| m.id == id)
}

pub fn display_name(id: &str) -> &str {
    find(id).map(|m| m.name).unwrap_or(id)
}

pub fn efforts_for(id: &str) -> &'static [&'static str] {
    find(id).map(|m| m.efforts).unwrap_or(&["off"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_in_catalog() {
        assert!(find(DEFAULT_MODEL).is_some());
        assert!(index_of(DEFAULT_MODEL).is_some());
    }

    #[test]
    fn every_model_has_off_or_levels() {
        for m in CATALOG {
            assert!(!m.efforts.is_empty(), "{} has no effort options", m.id);
            assert!(!m.name.is_empty());
        }
    }

    #[test]
    fn unknown_model_falls_back() {
        assert!(find("nope").is_none());
        assert_eq!(display_name("nope"), "nope");
        assert_eq!(efforts_for("nope"), &["off"]);
    }

    #[test]
    fn every_model_has_a_provider() {
        for m in CATALOG {
            assert!(!m.provider.is_empty(), "{} has no provider", m.id);
        }
        let grouped: usize = providers().iter().map(|p| models_for(p).len()).sum();
        assert_eq!(grouped, CATALOG.len(), "grouping must cover the catalog");
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len());
    }
}
