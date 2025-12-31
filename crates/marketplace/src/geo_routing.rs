//! Geographic routing policies and region-based data residency

use serde::{Deserialize, Serialize};

/// Geographic region
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    /// US West
    UsWest,
    /// US East
    UsEast,
    /// US Central
    UsCentral,
    /// EU West
    EuWest,
    /// EU Central
    EuCentral,
    /// EU East
    EuEast,
    /// Asia Pacific
    AsiaPacific,
    /// Asia South
    AsiaSouth,
    /// Asia East
    AsiaEast,
    /// Latin America
    Latam,
    /// Africa
    Africa,
    /// Oceania
    Oceania,
}

impl Region {
    /// Get region as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::UsWest => "us_west",
            Region::UsEast => "us_east",
            Region::UsCentral => "us_central",
            Region::EuWest => "eu_west",
            Region::EuCentral => "eu_central",
            Region::EuEast => "eu_east",
            Region::AsiaPacific => "asia_pacific",
            Region::AsiaSouth => "asia_south",
            Region::AsiaEast => "asia_east",
            Region::Latam => "latam",
            Region::Africa => "africa",
            Region::Oceania => "oceania",
        }
    }

    /// Estimate base latency between regions in milliseconds
    pub fn estimate_latency_to(&self, other: &Region) -> u32 {
        if self == other {
            return 5; // Same region
        }

        match (self, other) {
            // US internal
            (Region::UsWest, Region::UsEast) | (Region::UsEast, Region::UsWest) => 70,
            (Region::UsWest, Region::UsCentral) | (Region::UsCentral, Region::UsWest) => 30,
            (Region::UsEast, Region::UsCentral) | (Region::UsCentral, Region::UsEast) => 40,

            // EU internal
            (Region::EuWest, Region::EuCentral) | (Region::EuCentral, Region::EuWest) => 20,
            (Region::EuWest, Region::EuEast) | (Region::EuEast, Region::EuWest) => 40,
            (Region::EuCentral, Region::EuEast) | (Region::EuEast, Region::EuCentral) => 30,

            // Asia internal
            (Region::AsiaEast, Region::AsiaPacific) | (Region::AsiaPacific, Region::AsiaEast) => 60,
            (Region::AsiaEast, Region::AsiaSouth) | (Region::AsiaSouth, Region::AsiaEast) => 80,
            (Region::AsiaPacific, Region::AsiaSouth) | (Region::AsiaSouth, Region::AsiaPacific) => {
                100
            }

            // US to EU
            (Region::UsEast, Region::EuWest) | (Region::EuWest, Region::UsEast) => 80,
            (Region::UsEast, Region::EuCentral) | (Region::EuCentral, Region::UsEast) => 100,
            (Region::UsWest, Region::EuWest) | (Region::EuWest, Region::UsWest) => 150,

            // US to Asia
            (Region::UsWest, Region::AsiaEast) | (Region::AsiaEast, Region::UsWest) => 120,
            (Region::UsEast, Region::AsiaEast) | (Region::AsiaEast, Region::UsEast) => 200,

            // EU to Asia
            (Region::EuWest, Region::AsiaEast) | (Region::AsiaEast, Region::EuWest) => 180,
            (Region::EuCentral, Region::AsiaEast) | (Region::AsiaEast, Region::EuCentral) => 170,

            // Oceania connections
            (Region::Oceania, Region::AsiaPacific) | (Region::AsiaPacific, Region::Oceania) => 40,
            (Region::Oceania, Region::UsWest) | (Region::UsWest, Region::Oceania) => 140,

            // Latam connections
            (Region::Latam, Region::UsEast) | (Region::UsEast, Region::Latam) => 120,
            (Region::Latam, Region::EuWest) | (Region::EuWest, Region::Latam) => 180,

            // Africa connections
            (Region::Africa, Region::EuWest) | (Region::EuWest, Region::Africa) => 100,
            (Region::Africa, Region::AsiaSouth) | (Region::AsiaSouth, Region::Africa) => 150,

            // Default for unconfigured pairs
            _ => 250,
        }
    }
}

/// Geographic location information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoLocation {
    /// Region
    pub region: Region,
    /// Optional ISO country code
    pub country_code: Option<String>,
    /// Optional measured latency from user in milliseconds
    pub latency_ms_from_user: Option<u32>,
}

impl GeoLocation {
    /// Create a new geo location
    pub fn new(region: Region) -> Self {
        Self {
            region,
            country_code: None,
            latency_ms_from_user: None,
        }
    }

    /// Set country code
    pub fn with_country(mut self, code: impl Into<String>) -> Self {
        self.country_code = Some(code.into());
        self
    }

    /// Set latency from user
    pub fn with_latency(mut self, ms: u32) -> Self {
        self.latency_ms_from_user = Some(ms);
        self
    }
}

/// Data residency policy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataResidencyPolicy {
    /// Regions where sensitive data is allowed
    pub sensitive_data_regions: Vec<Region>,
    /// Whether to audit cross-region data transfers
    pub audit_cross_region: bool,
    /// Whether to require encryption for data in transit
    pub require_encryption_in_transit: bool,
}

impl DataResidencyPolicy {
    /// Create a new data residency policy
    pub fn new() -> Self {
        Self {
            sensitive_data_regions: Vec::new(),
            audit_cross_region: false,
            require_encryption_in_transit: true,
        }
    }

    /// Add allowed region for sensitive data
    pub fn allow_region(mut self, region: Region) -> Self {
        if !self.sensitive_data_regions.contains(&region) {
            self.sensitive_data_regions.push(region);
        }
        self
    }

    /// Enable cross-region auditing
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_cross_region = enabled;
        self
    }

    /// Set encryption requirement
    pub fn require_encryption(mut self, required: bool) -> Self {
        self.require_encryption_in_transit = required;
        self
    }

    /// Check if region is allowed for sensitive data
    pub fn is_region_allowed(&self, region: &Region) -> bool {
        self.sensitive_data_regions.is_empty() || self.sensitive_data_regions.contains(region)
    }
}

impl Default for DataResidencyPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Geographic routing policy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoRoutingPolicy {
    /// Preferred regions (in order of preference)
    pub preferred_regions: Vec<Region>,
    /// Required regions (data must stay within these)
    pub required_regions: Option<Vec<Region>>,
    /// Blocked regions (never route here)
    pub blocked_regions: Vec<Region>,
    /// Allow fallback to global pool if preferred/required unavailable
    pub allow_global_fallback: bool,
}

impl GeoRoutingPolicy {
    /// Create a new routing policy
    pub fn new() -> Self {
        Self {
            preferred_regions: Vec::new(),
            required_regions: None,
            blocked_regions: Vec::new(),
            allow_global_fallback: true,
        }
    }

    /// Add a preferred region
    pub fn prefer_region(mut self, region: Region) -> Self {
        if !self.preferred_regions.contains(&region) {
            self.preferred_regions.push(region);
        }
        self
    }

    /// Set required regions
    pub fn require_regions(mut self, regions: Vec<Region>) -> Self {
        self.required_regions = Some(regions);
        self
    }

    /// Block a region
    pub fn block_region(mut self, region: Region) -> Self {
        if !self.blocked_regions.contains(&region) {
            self.blocked_regions.push(region);
        }
        self
    }

    /// Set global fallback
    pub fn with_fallback(mut self, allow: bool) -> Self {
        self.allow_global_fallback = allow;
        self
    }

    /// Check if a region is allowed by this policy
    pub fn is_region_allowed(&self, region: &Region) -> bool {
        // Check if blocked
        if self.blocked_regions.contains(region) {
            return false;
        }

        // If required regions specified, must be in that list
        if let Some(ref required) = self.required_regions {
            return required.contains(region);
        }

        true
    }

    /// Get preference score for a region (higher is better)
    pub fn preference_score(&self, region: &Region) -> i32 {
        if self.blocked_regions.contains(region) {
            return -1000;
        }

        if let Some(ref required) = self.required_regions {
            if !required.contains(region) {
                return if self.allow_global_fallback {
                    -100
                } else {
                    -1000
                };
            }
        }

        // Higher score for earlier in preferred list
        if let Some(idx) = self.preferred_regions.iter().position(|r| r == region) {
            return 1000 - (idx as i32);
        }

        0 // Neutral for regions not in preferred list
    }
}

impl Default for GeoRoutingPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Organization geographic policy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrgGeoPolicy {
    /// Organization ID
    pub org_id: String,
    /// Default routing policy
    pub default_policy: GeoRoutingPolicy,
    /// Data residency policy
    pub data_residency: DataResidencyPolicy,
}

impl OrgGeoPolicy {
    /// Create a new organization policy
    pub fn new(org_id: impl Into<String>) -> Self {
        Self {
            org_id: org_id.into(),
            default_policy: GeoRoutingPolicy::default(),
            data_residency: DataResidencyPolicy::default(),
        }
    }

    /// Set routing policy
    pub fn with_routing(mut self, policy: GeoRoutingPolicy) -> Self {
        self.default_policy = policy;
        self
    }

    /// Set data residency policy
    pub fn with_residency(mut self, policy: DataResidencyPolicy) -> Self {
        self.data_residency = policy;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_as_str() {
        assert_eq!(Region::UsWest.as_str(), "us_west");
        assert_eq!(Region::EuCentral.as_str(), "eu_central");
        assert_eq!(Region::AsiaEast.as_str(), "asia_east");
    }

    #[test]
    fn test_region_estimate_latency() {
        // Same region
        assert_eq!(Region::UsWest.estimate_latency_to(&Region::UsWest), 5);

        // US internal
        assert_eq!(Region::UsWest.estimate_latency_to(&Region::UsEast), 70);
        assert_eq!(Region::UsEast.estimate_latency_to(&Region::UsWest), 70);

        // Cross-continent
        assert!(Region::UsWest.estimate_latency_to(&Region::EuWest) > 100);
        assert!(Region::UsEast.estimate_latency_to(&Region::AsiaEast) > 100);
    }

    #[test]
    fn test_geo_location_builder() {
        let loc = GeoLocation::new(Region::UsWest)
            .with_country("US")
            .with_latency(25);

        assert_eq!(loc.region, Region::UsWest);
        assert_eq!(loc.country_code, Some("US".to_string()));
        assert_eq!(loc.latency_ms_from_user, Some(25));
    }

    #[test]
    fn test_data_residency_policy() {
        let policy = DataResidencyPolicy::new()
            .allow_region(Region::UsWest)
            .allow_region(Region::UsEast)
            .with_audit(true)
            .require_encryption(true);

        assert!(policy.is_region_allowed(&Region::UsWest));
        assert!(policy.is_region_allowed(&Region::UsEast));
        assert!(!policy.is_region_allowed(&Region::EuWest));
        assert!(policy.audit_cross_region);
        assert!(policy.require_encryption_in_transit);
    }

    #[test]
    fn test_data_residency_empty_allows_all() {
        let policy = DataResidencyPolicy::new();
        assert!(policy.is_region_allowed(&Region::UsWest));
        assert!(policy.is_region_allowed(&Region::EuWest));
    }

    #[test]
    fn test_geo_routing_policy_builder() {
        let policy = GeoRoutingPolicy::new()
            .prefer_region(Region::UsWest)
            .prefer_region(Region::UsEast)
            .block_region(Region::Africa)
            .with_fallback(false);

        assert_eq!(policy.preferred_regions.len(), 2);
        assert_eq!(policy.blocked_regions.len(), 1);
        assert!(!policy.allow_global_fallback);
    }

    #[test]
    fn test_geo_routing_is_region_allowed() {
        let policy = GeoRoutingPolicy::new()
            .block_region(Region::Africa)
            .require_regions(vec![Region::UsWest, Region::UsEast]);

        assert!(!policy.is_region_allowed(&Region::Africa));
        assert!(policy.is_region_allowed(&Region::UsWest));
        assert!(policy.is_region_allowed(&Region::UsEast));
        assert!(!policy.is_region_allowed(&Region::EuWest));
    }

    #[test]
    fn test_geo_routing_preference_score() {
        let policy = GeoRoutingPolicy::new()
            .prefer_region(Region::UsWest)
            .prefer_region(Region::UsEast)
            .block_region(Region::Africa);

        // Blocked region
        assert!(policy.preference_score(&Region::Africa) < 0);

        // Preferred regions (higher score for earlier in list)
        let us_west_score = policy.preference_score(&Region::UsWest);
        let us_east_score = policy.preference_score(&Region::UsEast);
        assert!(us_west_score > us_east_score);

        // Neutral region
        assert_eq!(policy.preference_score(&Region::EuWest), 0);
    }

    #[test]
    fn test_geo_routing_required_regions_strict() {
        let policy = GeoRoutingPolicy::new()
            .require_regions(vec![Region::EuWest, Region::EuCentral])
            .with_fallback(false);

        assert!(policy.is_region_allowed(&Region::EuWest));
        assert!(!policy.is_region_allowed(&Region::UsWest));

        // With fallback disabled, non-required regions get very negative score
        assert!(policy.preference_score(&Region::UsWest) < -500);
    }

    #[test]
    fn test_geo_routing_required_regions_with_fallback() {
        let policy = GeoRoutingPolicy::new()
            .require_regions(vec![Region::EuWest])
            .with_fallback(true);

        // Still allowed, but low score
        let score = policy.preference_score(&Region::UsWest);
        assert!(score < 0);
        assert!(score > -500); // Not as negative as strict mode
    }

    #[test]
    fn test_org_geo_policy() {
        let routing = GeoRoutingPolicy::new().prefer_region(Region::UsWest);
        let residency = DataResidencyPolicy::new().allow_region(Region::UsWest);

        let org_policy = OrgGeoPolicy::new("org123")
            .with_routing(routing)
            .with_residency(residency);

        assert_eq!(org_policy.org_id, "org123");
        assert_eq!(org_policy.default_policy.preferred_regions.len(), 1);
        assert_eq!(org_policy.data_residency.sensitive_data_regions.len(), 1);
    }

    #[test]
    fn test_geo_routing_serde() {
        let policy = GeoRoutingPolicy::new()
            .prefer_region(Region::UsWest)
            .block_region(Region::Africa);

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: GeoRoutingPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_region_serde() {
        let region = Region::UsWest;
        let json = serde_json::to_string(&region).unwrap();
        assert_eq!(json, "\"us_west\"");
        let deserialized: Region = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, region);
    }
}
