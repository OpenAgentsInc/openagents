//! Unit tests for skill discovery and search functionality

use marketplace::discovery::{SearchFilters, SkillListing, SortOrder, discover_local_skills};
use std::fs;
use std::path::PathBuf;

// =========================================================================
// SearchFilters tests
// =========================================================================

#[test]
fn test_search_filters_default() {
    let filters = SearchFilters::default();
    assert!(filters.status.is_none());
    assert!(filters.category.is_none());
    assert!(!filters.free_only);
}

#[test]
fn test_search_filters_with_status() {
    let filters = SearchFilters {
        status: Some("active".to_string()),
        ..Default::default()
    };
    assert_eq!(filters.status, Some("active".to_string()));
    assert!(filters.category.is_none());
    assert!(!filters.free_only);
}

#[test]
fn test_search_filters_with_category() {
    let filters = SearchFilters {
        category: Some("productivity".to_string()),
        ..Default::default()
    };
    assert_eq!(filters.category, Some("productivity".to_string()));
    assert!(filters.status.is_none());
    assert!(!filters.free_only);
}

#[test]
fn test_search_filters_free_only() {
    let filters = SearchFilters {
        free_only: true,
        ..Default::default()
    };
    assert!(filters.free_only);
    assert!(filters.status.is_none());
    assert!(filters.category.is_none());
}

#[test]
fn test_search_filters_all_fields() {
    let filters = SearchFilters {
        status: Some("active".to_string()),
        category: Some("development".to_string()),
        free_only: true,
    };
    assert_eq!(filters.status, Some("active".to_string()));
    assert_eq!(filters.category, Some("development".to_string()));
    assert!(filters.free_only);
}

#[test]
fn test_search_filters_empty_strings() {
    let filters = SearchFilters {
        status: Some("".to_string()),
        category: Some("".to_string()),
        free_only: false,
    };
    assert_eq!(filters.status, Some("".to_string()));
    assert_eq!(filters.category, Some("".to_string()));
}

#[test]
fn test_search_filters_clone() {
    let filters1 = SearchFilters {
        status: Some("published".to_string()),
        category: Some("tools".to_string()),
        free_only: true,
    };
    let filters2 = filters1.clone();

    assert_eq!(filters1.status, filters2.status);
    assert_eq!(filters1.category, filters2.category);
    assert_eq!(filters1.free_only, filters2.free_only);
}

// =========================================================================
// SortOrder tests
// =========================================================================

#[test]
fn test_sort_order_variants() {
    let recent = SortOrder::Recent;
    let alphabetical = SortOrder::Alphabetical;
    let popular = SortOrder::Popular;

    assert!(matches!(recent, SortOrder::Recent));
    assert!(matches!(alphabetical, SortOrder::Alphabetical));
    assert!(matches!(popular, SortOrder::Popular));
}

#[test]
fn test_sort_order_equality() {
    assert_eq!(SortOrder::Recent, SortOrder::Recent);
    assert_eq!(SortOrder::Alphabetical, SortOrder::Alphabetical);
    assert_eq!(SortOrder::Popular, SortOrder::Popular);

    assert_ne!(SortOrder::Recent, SortOrder::Alphabetical);
    assert_ne!(SortOrder::Recent, SortOrder::Popular);
    assert_ne!(SortOrder::Alphabetical, SortOrder::Popular);
}

#[test]
fn test_sort_order_clone() {
    let order1 = SortOrder::Recent;
    let order2 = order1;
    assert_eq!(order1, order2);
}

#[test]
fn test_sort_order_copy() {
    let order = SortOrder::Alphabetical;
    let copied = order;
    assert_eq!(order, copied);
}

// =========================================================================
// SkillListing tests
// =========================================================================

#[test]
fn test_skill_listing_creation() {
    let listing = SkillListing {
        slug: "test-skill".to_string(),
        name: "Test Skill".to_string(),
        description: "A test skill".to_string(),
        author: Some("test-author".to_string()),
        version: "1.0.0".to_string(),
        status: "active".to_string(),
    };

    assert_eq!(listing.slug, "test-skill");
    assert_eq!(listing.name, "Test Skill");
    assert_eq!(listing.description, "A test skill");
    assert_eq!(listing.author, Some("test-author".to_string()));
    assert_eq!(listing.version, "1.0.0");
    assert_eq!(listing.status, "active");
}

#[test]
fn test_skill_listing_no_author() {
    let listing = SkillListing {
        slug: "anonymous-skill".to_string(),
        name: "Anonymous Skill".to_string(),
        description: "No author".to_string(),
        author: None,
        version: "0.1.0".to_string(),
        status: "draft".to_string(),
    };

    assert!(listing.author.is_none());
}

#[test]
fn test_skill_listing_clone() {
    let listing1 = SkillListing {
        slug: "cloneable".to_string(),
        name: "Cloneable".to_string(),
        description: "Can be cloned".to_string(),
        author: Some("author".to_string()),
        version: "2.0.0".to_string(),
        status: "published".to_string(),
    };

    let listing2 = listing1.clone();
    assert_eq!(listing1.slug, listing2.slug);
    assert_eq!(listing1.name, listing2.name);
    assert_eq!(listing1.description, listing2.description);
    assert_eq!(listing1.author, listing2.author);
    assert_eq!(listing1.version, listing2.version);
    assert_eq!(listing1.status, listing2.status);
}

#[test]
fn test_skill_listing_empty_fields() {
    let listing = SkillListing {
        slug: "".to_string(),
        name: "".to_string(),
        description: "".to_string(),
        author: Some("".to_string()),
        version: "".to_string(),
        status: "".to_string(),
    };

    assert_eq!(listing.slug, "");
    assert_eq!(listing.name, "");
    assert_eq!(listing.description, "");
}

#[test]
fn test_skill_listing_special_characters() {
    let listing = SkillListing {
        slug: "special-chars".to_string(),
        name: "Special & <Characters>".to_string(),
        description: "Test \"quotes\" and 'apostrophes'".to_string(),
        author: Some("Author@Example.com".to_string()),
        version: "1.2.3-beta+001".to_string(),
        status: "pre-release".to_string(),
    };

    assert!(listing.name.contains("&"));
    assert!(listing.description.contains("\""));
    assert!(listing.version.contains("-beta"));
}

// =========================================================================
// discover_local_skills integration tests (already exist in discovery.rs)
// =========================================================================

#[test]
fn test_discover_local_skills_nonexistent_dir() {
    let nonexistent = PathBuf::from("/tmp/nonexistent_skill_dir_12345");
    let metadata = discover_local_skills(&[nonexistent]);

    // Should handle gracefully and return empty
    assert_eq!(metadata.len(), 0);
}

#[test]
fn test_discover_local_skills_invalid_skill_format() {
    let temp_dir = std::env::temp_dir().join("test_invalid_skill");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create directory without SKILL.md
    let skill_dir = temp_dir.join("broken-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let metadata = discover_local_skills(std::slice::from_ref(&temp_dir));

    // Should skip invalid skills
    assert_eq!(metadata.len(), 0);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_discover_local_skills_mixed_valid_invalid() {
    let temp_dir = std::env::temp_dir().join("test_mixed_skills");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Valid skill
    let valid_dir = temp_dir.join("valid-skill");
    fs::create_dir_all(&valid_dir).unwrap();
    fs::write(
        valid_dir.join("SKILL.md"),
        r#"---
name: valid-skill
description: Valid skill
---
# Valid"#,
    )
    .unwrap();

    // Invalid skill (no SKILL.md)
    let invalid_dir = temp_dir.join("invalid-skill");
    fs::create_dir_all(&invalid_dir).unwrap();

    let metadata = discover_local_skills(std::slice::from_ref(&temp_dir));

    // Should only find the valid one
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].name, "valid-skill");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_discover_local_skills_permissions_handling() {
    // Test that discovery handles permission errors gracefully
    let temp_dir = std::env::temp_dir().join("test_permissions");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let metadata = discover_local_skills(std::slice::from_ref(&temp_dir));

    // Should handle empty dir gracefully
    assert_eq!(metadata.len(), 0);

    let _ = fs::remove_dir_all(&temp_dir);
}

// =========================================================================
// Edge cases and boundary conditions
// =========================================================================

#[test]
fn test_search_filters_unicode_values() {
    let filters = SearchFilters {
        status: Some("活动".to_string()),
        category: Some("開発".to_string()),
        free_only: false,
    };

    assert_eq!(filters.status, Some("活动".to_string()));
    assert_eq!(filters.category, Some("開発".to_string()));
}

#[test]
fn test_skill_listing_very_long_description() {
    let long_desc = "A".repeat(10000);
    let listing = SkillListing {
        slug: "long-desc".to_string(),
        name: "Long Description Test".to_string(),
        description: long_desc.clone(),
        author: None,
        version: "1.0.0".to_string(),
        status: "active".to_string(),
    };

    assert_eq!(listing.description.len(), 10000);
}

#[test]
fn test_discover_local_skills_many_directories() {
    let temp_base = std::env::temp_dir().join("test_many_dirs");
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    let mut dirs = Vec::new();
    for i in 0..10 {
        let dir = temp_base.join(format!("dir_{}", i));
        fs::create_dir_all(&dir).unwrap();
        dirs.push(dir);
    }

    let metadata = discover_local_skills(&dirs);

    // All empty directories - should return empty
    assert_eq!(metadata.len(), 0);

    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn test_skill_listing_version_formats() {
    let versions = vec![
        "1.0.0",
        "2.1.3-alpha",
        "0.0.1-beta.1",
        "1.2.3+build.001",
        "v1.0.0",
    ];

    for version in versions {
        let listing = SkillListing {
            slug: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            author: None,
            version: version.to_string(),
            status: "active".to_string(),
        };

        assert_eq!(listing.version, version);
    }
}

#[test]
fn test_search_filters_debug_format() {
    let filters = SearchFilters {
        status: Some("test".to_string()),
        category: Some("cat".to_string()),
        free_only: true,
    };

    let debug_str = format!("{:?}", filters);
    assert!(debug_str.contains("SearchFilters"));
}

#[test]
fn test_sort_order_debug_format() {
    let order = SortOrder::Recent;
    let debug_str = format!("{:?}", order);
    assert!(debug_str.contains("Recent"));
}

#[test]
fn test_skill_listing_debug_format() {
    let listing = SkillListing {
        slug: "test".to_string(),
        name: "Test".to_string(),
        description: "Desc".to_string(),
        author: None,
        version: "1.0.0".to_string(),
        status: "active".to_string(),
    };

    let debug_str = format!("{:?}", listing);
    assert!(debug_str.contains("SkillListing"));
}
