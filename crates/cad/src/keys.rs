/// Typed metadata/parameter key wrapper for canonical CAD map keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CadMapKey(&'static str);

impl CadMapKey {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn owned(self) -> String {
        self.0.to_string()
    }
}

/// Canonical document metadata keys produced by STEP import.
pub mod import_metadata {
    use super::CadMapKey;

    pub const FORMAT: CadMapKey = CadMapKey::new("import.format");
    pub const HASH: CadMapKey = CadMapKey::new("import.hash");
    pub const SOLID_COUNT: CadMapKey = CadMapKey::new("import.solid_count");
    pub const SHELL_COUNT: CadMapKey = CadMapKey::new("import.shell_count");
    pub const FACE_COUNT: CadMapKey = CadMapKey::new("import.face_count");
}

/// Canonical warning metadata keys shared across validity/feature flows.
pub mod warning_metadata {
    use super::CadMapKey;

    pub const ENTITY_ID: CadMapKey = CadMapKey::new("entity_id");
    pub const FEATURE_ID: CadMapKey = CadMapKey::new("feature_id");
    pub const SOURCE_FEATURE_ID: CadMapKey = CadMapKey::new("source_feature_id");
    pub const DEEP_LINK: CadMapKey = CadMapKey::new("deep_link");
    pub const OPERATION_KEY: CadMapKey = CadMapKey::new("operation_key");
    pub const CLASSIFICATION: CadMapKey = CadMapKey::new("classification");
    pub const WARNING_DOMAIN: CadMapKey = CadMapKey::new("warning_domain");
}

/// Canonical reusable feature node parameter keys.
pub mod feature_params {
    use super::CadMapKey;

    pub const KIND: CadMapKey = CadMapKey::new("kind");
    pub const RADIUS_PARAM: CadMapKey = CadMapKey::new("radius_param");
}

#[cfg(test)]
mod tests {
    use super::{feature_params, import_metadata, warning_metadata};

    #[test]
    fn canonical_key_values_are_stable() {
        assert_eq!(import_metadata::FORMAT.as_str(), "import.format");
        assert_eq!(import_metadata::HASH.as_str(), "import.hash");
        assert_eq!(import_metadata::SOLID_COUNT.as_str(), "import.solid_count");
        assert_eq!(import_metadata::SHELL_COUNT.as_str(), "import.shell_count");
        assert_eq!(import_metadata::FACE_COUNT.as_str(), "import.face_count");
        assert_eq!(warning_metadata::FEATURE_ID.as_str(), "feature_id");
        assert_eq!(warning_metadata::DEEP_LINK.as_str(), "deep_link");
        assert_eq!(feature_params::KIND.as_str(), "kind");
        assert_eq!(feature_params::RADIUS_PARAM.as_str(), "radius_param");
    }
}
