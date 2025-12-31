//! Skills CLI commands

use crate::skills::{SearchFilters, SkillBrowser, SkillCategory, SortBy};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum SkillsCommands {
    /// Browse available skills
    Browse {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Filter by capability
        #[arg(long)]
        capability: Option<String>,

        /// Maximum price in sats
        #[arg(long)]
        max_price: Option<u64>,

        /// Only show free skills
        #[arg(long)]
        free: bool,

        /// Sort order (recent, name, popular, price)
        #[arg(long, default_value = "name")]
        sort: String,
    },

    /// Search for skills by query
    Search {
        /// Search query
        query: String,

        /// Sort order (recent, name, popular, price)
        #[arg(long, default_value = "popular")]
        sort: String,
    },

    /// Show details for a specific skill
    Show {
        /// Skill ID (handler pubkey)
        id: String,
    },

    /// List installed skills
    List,

    /// Install a skill
    Install {
        /// Skill ID to install
        id: String,

        /// Specific version (default: latest)
        #[arg(long)]
        version: Option<String>,
    },

    /// Uninstall a skill
    Uninstall {
        /// Skill ID to uninstall
        id: String,
    },

    /// Update installed skills
    Update {
        /// Update all skills
        #[arg(long)]
        all: bool,

        /// Specific skill ID to update
        id: Option<String>,
    },
}

impl SkillsCommands {
    pub fn execute(self) -> anyhow::Result<()> {
        match self {
            SkillsCommands::Browse {
                category,
                capability,
                max_price,
                free,
                sort,
            } => {
                browse_skills(category, capability, max_price, free, &sort)?;
            }
            SkillsCommands::Search { query, sort } => {
                search_skills(&query, &sort)?;
            }
            SkillsCommands::Show { id } => {
                show_skill(&id)?;
            }
            SkillsCommands::List => {
                list_installed_skills()?;
            }
            SkillsCommands::Install { id, version } => {
                install_skill(&id, version.as_deref())?;
            }
            SkillsCommands::Uninstall { id } => {
                uninstall_skill(&id)?;
            }
            SkillsCommands::Update { all, id } => {
                update_skills(all, id.as_deref())?;
            }
        }
        Ok(())
    }
}

fn parse_sort_order(sort: &str) -> SortBy {
    match sort.to_lowercase().as_str() {
        "recent" => SortBy::Recent,
        "name" => SortBy::Name,
        "popular" => SortBy::Popular,
        "price" => SortBy::Price,
        _ => SortBy::Name,
    }
}

fn browse_skills(
    category: Option<String>,
    capability: Option<String>,
    max_price: Option<u64>,
    free: bool,
    sort: &str,
) -> anyhow::Result<()> {
    let mut filters = SearchFilters::new();

    if let Some(cat) = category {
        filters = filters.with_category(cat.parse().unwrap_or(SkillCategory::Other(cat)));
    }

    if let Some(cap) = capability {
        filters = filters.with_capability(cap);
    }

    if let Some(price) = max_price {
        filters = filters.with_max_price(price);
    }

    if free {
        filters = filters.free_only();
    }

    let sort_by = parse_sort_order(sort);

    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = rt.block_on(SkillBrowser::new())?;
    let skills = rt.block_on(browser.browse(filters, sort_by))?;

    if skills.is_empty() {
        println!("No skills found matching criteria");
        return Ok(());
    }

    println!("Available Skills ({} found):\n", skills.len());
    for skill in skills {
        print_skill_listing(&skill);
    }

    Ok(())
}

fn search_skills(query: &str, sort: &str) -> anyhow::Result<()> {
    let sort_by = parse_sort_order(sort);

    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = rt.block_on(SkillBrowser::new())?;
    let skills = rt.block_on(browser.search(query, sort_by))?;

    if skills.is_empty() {
        println!("No skills found matching '{}'", query);
        return Ok(());
    }

    println!("Search Results for '{}' ({} found):\n", query, skills.len());
    for skill in skills {
        print_skill_listing(&skill);
    }

    Ok(())
}

fn show_skill(id: &str) -> anyhow::Result<()> {
    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = rt.block_on(SkillBrowser::new())?;

    match rt.block_on(browser.get_skill(id)) {
        Ok(skill) => {
            print_skill_details(&skill);
            Ok(())
        }
        Err(e) => {
            println!("Skill not found: {}", e);
            Ok(())
        }
    }
}

fn print_skill_listing(skill: &crate::skills::SkillListing) {
    println!("ID: {}", skill.id);
    println!("Name: {}", skill.name);
    println!("Description: {}", skill.description);
    println!("Version: {}", skill.version);

    if let Some(price) = skill.price_sats {
        if let Some(model) = &skill.price_model {
            println!("Price: {} sats ({})", price, model);
        } else {
            println!("Price: {} sats", price);
        }
    } else {
        println!("Price: Free");
    }

    if !skill.capabilities.is_empty() {
        println!("Capabilities: {}", skill.capabilities.join(", "));
    }

    if skill.recommendation_count > 0 {
        println!("Recommendations: {}", skill.recommendation_count);
    }

    println!();
}

fn print_skill_details(skill: &crate::skills::SkillListing) {
    println!("=== Skill Details ===\n");
    println!("ID: {}", skill.id);
    println!("Name: {}", skill.name);
    println!("Description: {}", skill.description);
    println!("Version: {}", skill.version);
    println!("Creator: {}", skill.creator_pubkey);

    if let Some(price) = skill.price_sats {
        if let Some(model) = &skill.price_model {
            println!("Price: {} sats ({})", price, model);
        } else {
            println!("Price: {} sats", price);
        }
    } else {
        println!("Price: Free");
    }

    if !skill.capabilities.is_empty() {
        println!("\nCapabilities:");
        for cap in &skill.capabilities {
            println!("  - {}", cap);
        }
    }

    if let Some(website) = &skill.website {
        println!("\nWebsite: {}", website);
    }

    if skill.recommendation_count > 0 {
        println!("\nRecommendations: {}", skill.recommendation_count);
    }

    println!();
}

fn list_installed_skills() -> anyhow::Result<()> {
    println!("Installed skills:");
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn install_skill(id: &str, version: Option<&str>) -> anyhow::Result<()> {
    if let Some(ver) = version {
        println!("Installing skill {} version {}...", id, ver);
    } else {
        println!("Installing skill {} (latest version)...", id);
    }
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn uninstall_skill(id: &str) -> anyhow::Result<()> {
    println!("Uninstalling skill {}...", id);
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn update_skills(all: bool, id: Option<&str>) -> anyhow::Result<()> {
    if all {
        println!("Updating all installed skills...");
    } else if let Some(skill_id) = id {
        println!("Updating skill {}...", skill_id);
    } else {
        println!("Please specify --all or provide a skill ID");
        return Ok(());
    }
    println!("(Not yet implemented - coming soon)");
    Ok(())
}
