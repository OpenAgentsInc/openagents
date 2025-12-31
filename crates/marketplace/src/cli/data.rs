//! Data CLI commands

use crate::data::{DatasetBrowser, DatasetCategory, SearchFilters, SortBy};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DataCommands {
    /// Browse available datasets
    Browse {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Filter by MIME type
        #[arg(long)]
        mime_type: Option<String>,

        /// Maximum price in sats
        #[arg(long)]
        max_price: Option<u64>,

        /// Minimum size in bytes
        #[arg(long)]
        min_size: Option<u64>,

        /// Maximum size in bytes
        #[arg(long)]
        max_size: Option<u64>,

        /// Only show free datasets
        #[arg(long)]
        free: bool,

        /// Sort order (recent, name, size, price)
        #[arg(long, default_value = "name")]
        sort: String,
    },

    /// Search for datasets by query
    Search {
        /// Search query
        query: String,

        /// Sort order (recent, name, size, price)
        #[arg(long, default_value = "size")]
        sort: String,
    },

    /// Show details for a specific dataset
    Show {
        /// Dataset ID
        id: String,
    },

    /// List purchased datasets
    List,

    /// Purchase a dataset
    Purchase {
        /// Dataset ID to purchase
        id: String,
    },

    /// Download a purchased dataset
    Download {
        /// Dataset ID to download
        id: String,

        /// Output directory
        #[arg(long, default_value = ".")]
        output: String,
    },

    /// Publish a dataset
    Publish {
        /// Path to dataset
        path: String,

        /// Dataset name
        #[arg(long)]
        name: String,

        /// Price in sats (omit for free)
        #[arg(long)]
        price: Option<u64>,
    },
}

impl DataCommands {
    pub fn execute(self) -> anyhow::Result<()> {
        match self {
            DataCommands::Browse {
                category,
                mime_type,
                max_price,
                min_size,
                max_size,
                free,
                sort,
            } => {
                browse_datasets(
                    category, mime_type, max_price, min_size, max_size, free, &sort,
                )?;
            }
            DataCommands::Search { query, sort } => {
                search_datasets(&query, &sort)?;
            }
            DataCommands::Show { id } => {
                show_dataset(&id)?;
            }
            DataCommands::List => {
                list_purchased_datasets()?;
            }
            DataCommands::Purchase { id } => {
                purchase_dataset(&id)?;
            }
            DataCommands::Download { id, output } => {
                download_dataset(&id, &output)?;
            }
            DataCommands::Publish { path, name, price } => {
                publish_dataset(&path, &name, price)?;
            }
        }
        Ok(())
    }
}

fn parse_sort_order(sort: &str) -> SortBy {
    match sort.to_lowercase().as_str() {
        "recent" => SortBy::Recent,
        "name" => SortBy::Name,
        "size" => SortBy::Size,
        "price" => SortBy::Price,
        _ => SortBy::Name,
    }
}

fn browse_datasets(
    category: Option<String>,
    mime_type: Option<String>,
    max_price: Option<u64>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    free: bool,
    sort: &str,
) -> anyhow::Result<()> {
    let mut filters = SearchFilters::new();

    if let Some(cat) = category {
        filters = filters.with_category(cat.parse().unwrap_or(DatasetCategory::Other(cat)));
    }

    if let Some(mime) = mime_type {
        filters = filters.with_mime_type(mime);
    }

    if let Some(price) = max_price {
        filters = filters.with_max_price(price);
    }

    if let Some(min) = min_size {
        filters = filters.with_min_size(min);
    }

    if let Some(max) = max_size {
        filters = filters.with_max_size(max);
    }

    if free {
        filters = filters.free_only();
    }

    let sort_by = parse_sort_order(sort);

    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = DatasetBrowser::new();
    let datasets = rt.block_on(browser.browse(filters, sort_by))?;

    if datasets.is_empty() {
        println!("No datasets found matching criteria");
        return Ok(());
    }

    println!("Available Datasets ({} found):\n", datasets.len());
    for dataset in datasets {
        print_dataset_listing(&dataset);
    }

    Ok(())
}

fn search_datasets(query: &str, sort: &str) -> anyhow::Result<()> {
    let sort_by = parse_sort_order(sort);

    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = DatasetBrowser::new();
    let datasets = rt.block_on(browser.search(query, sort_by))?;

    if datasets.is_empty() {
        println!("No datasets found matching '{}'", query);
        return Ok(());
    }

    println!(
        "Search Results for '{}' ({} found):\n",
        query,
        datasets.len()
    );
    for dataset in datasets {
        print_dataset_listing(&dataset);
    }

    Ok(())
}

fn show_dataset(id: &str) -> anyhow::Result<()> {
    // Create runtime for async operation
    let rt = tokio::runtime::Runtime::new()?;
    let browser = DatasetBrowser::new();

    match rt.block_on(browser.get_dataset(id)) {
        Ok(dataset) => {
            print_dataset_details(&dataset);
            Ok(())
        }
        Err(e) => {
            println!("Dataset not found: {}", e);
            Ok(())
        }
    }
}

fn print_dataset_listing(dataset: &crate::data::DatasetListing) {
    println!("ID: {}", dataset.id);
    println!("Name: {}", dataset.name);
    println!("Description: {}", dataset.description);
    println!("Size: {}", dataset.format_size());
    println!("MIME Type: {}", dataset.mime_type);

    if let Some(price) = dataset.price_sats {
        println!("Price: {} sats", price);
    } else {
        println!("Price: Free");
    }

    println!();
}

fn print_dataset_details(dataset: &crate::data::DatasetListing) {
    println!("=== Dataset Details ===\n");
    println!("ID: {}", dataset.id);
    println!("Name: {}", dataset.name);
    println!("Description: {}", dataset.description);
    println!("Creator: {}", dataset.creator_pubkey);
    println!("URL: {}", dataset.url);
    println!("MIME Type: {}", dataset.mime_type);
    println!("Hash: {}", dataset.hash);
    println!("Size: {}", dataset.format_size());

    if let Some(price) = dataset.price_sats {
        println!("Price: {} sats", price);
    } else {
        println!("Price: Free");
    }

    if let Some(preview) = &dataset.preview_url {
        println!("\nPreview: {}", preview);
    }

    if let Some(summary) = &dataset.summary {
        println!("\nSummary: {}", summary);
    }

    println!();
}

fn list_purchased_datasets() -> anyhow::Result<()> {
    println!("Purchased datasets:");
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn purchase_dataset(id: &str) -> anyhow::Result<()> {
    println!("Purchasing dataset {}...", id);
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn download_dataset(id: &str, output: &str) -> anyhow::Result<()> {
    println!("Downloading dataset {} to {}...", id, output);
    println!("(Not yet implemented - coming soon)");
    Ok(())
}

fn publish_dataset(path: &str, name: &str, price: Option<u64>) -> anyhow::Result<()> {
    if let Some(p) = price {
        println!(
            "Publishing dataset {} from {} for {} sats...",
            name, path, p
        );
    } else {
        println!("Publishing free dataset {} from {}...", name, path);
    }
    println!("(Not yet implemented - coming soon)");
    Ok(())
}
