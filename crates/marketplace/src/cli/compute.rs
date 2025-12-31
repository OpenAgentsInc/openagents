//! Compute CLI commands

use crate::core::discovery::{ProviderDiscovery, ProviderQuery, SortBy};
use clap::Subcommand;

/// Parameters for job submission
struct SubmitParams<'a> {
    job_type: &'a str,
    prompt: Option<&'a str>,
    file: Option<&'a str>,
    model: Option<&'a str>,
    budget: Option<u64>,
    stream: bool,
    json: bool,
    target_language: Option<&'a str>,
    local_first: bool,
}

#[derive(Debug, Subcommand)]
pub enum ComputeCommands {
    /// List available compute providers
    Providers {
        /// Filter by model
        #[arg(long)]
        model: Option<String>,

        /// Filter by region
        #[arg(long)]
        region: Option<String>,

        /// Filter by maximum price (in millisats)
        #[arg(long)]
        max_price: Option<u64>,

        /// Filter by minimum trust score (0.0-1.0)
        #[arg(long)]
        min_trust: Option<f32>,

        /// Sort by (trust, price, recommendations)
        #[arg(long, default_value = "trust")]
        sort: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Submit a compute job
    Submit {
        /// Job type (text-generation, summarization, translation, etc.)
        #[arg(long)]
        job_type: String,

        /// Input prompt or text
        #[arg(long)]
        prompt: Option<String>,

        /// Input from file
        #[arg(long)]
        file: Option<String>,

        /// Model to use
        #[arg(long)]
        model: Option<String>,

        /// Maximum bid in sats
        #[arg(long)]
        budget: Option<u64>,

        /// Stream results
        #[arg(long)]
        stream: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Target language for translation jobs (e.g., "en", "es", "fr")
        #[arg(long)]
        target_language: Option<String>,

        /// Try local inference first, fallback to swarm if unavailable
        #[arg(long)]
        local_first: bool,
    },

    /// Check job status
    Status {
        /// Job ID
        job_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Cancel a job
    Cancel {
        /// Job ID
        job_id: String,
    },

    /// View job history
    History {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Limit number of results
        #[arg(long)]
        limit: Option<usize>,
    },
}

/// Format a unix timestamp as a human-readable string
fn format_timestamp(ts: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let timestamp = UNIX_EPOCH + Duration::from_secs(ts);
    let now = SystemTime::now();

    if let Ok(duration) = now.duration_since(timestamp) {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    } else {
        "in future".to_string()
    }
}

impl ComputeCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        match self {
            ComputeCommands::Providers {
                model,
                region,
                max_price,
                min_trust,
                sort,
                json,
            } => self.providers(
                model.as_deref(),
                region.as_deref(),
                *max_price,
                *min_trust,
                sort,
                *json,
            ),

            ComputeCommands::Submit {
                job_type,
                prompt,
                file,
                model,
                budget,
                stream,
                json,
                target_language,
                local_first,
            } => self.submit(SubmitParams {
                job_type,
                prompt: prompt.as_deref(),
                file: file.as_deref(),
                model: model.as_deref(),
                budget: *budget,
                stream: *stream,
                json: *json,
                target_language: target_language.as_deref(),
                local_first: *local_first,
            }),

            ComputeCommands::Status { job_id, json } => self.status(job_id, *json),

            ComputeCommands::Cancel { job_id } => self.cancel(job_id),

            ComputeCommands::History { json, limit } => self.history(*json, *limit),
        }
    }

    fn providers(
        &self,
        model: Option<&str>,
        region: Option<&str>,
        max_price: Option<u64>,
        min_trust: Option<f32>,
        sort: &str,
        json_output: bool,
    ) -> anyhow::Result<()> {
        // Create provider discovery (in real impl, this would fetch from relays)
        let discovery = ProviderDiscovery::new();

        // Build query from filters
        let mut query = ProviderQuery::new();

        if let Some(m) = model {
            query = query.with_model(m);
        }

        if let Some(r) = region {
            query = query.with_region(r);
        }

        if let Some(price) = max_price {
            query = query.with_max_price(price);
        }

        if let Some(trust) = min_trust {
            query = query.with_min_trust_score(trust);
        }

        // Parse sort option
        let sort_by = match sort.to_lowercase().as_str() {
            "price" => SortBy::Price,
            "recommendations" | "recs" => SortBy::Recommendations,
            "trust" => SortBy::TrustScore,
            _ => SortBy::TrustScore,
        };
        query = query.sort_by(sort_by);

        // Query providers
        let providers = discovery.query(&query);

        if json_output {
            // JSON output
            let json = serde_json::json!({
                "providers": providers.iter().map(|p| serde_json::json!({
                    "pubkey": p.pubkey,
                    "name": p.metadata.name,
                    "description": p.metadata.description,
                    "capabilities": p.capabilities,
                    "pricing": p.pricing.as_ref().map(|pr| serde_json::json!({
                        "amount_msats": pr.amount_msats,
                        "model": pr.model,
                    })),
                    "trust_score": p.trust_score,
                    "recommendations": p.recommendation_count,
                    "region": p.metadata.region,
                })).collect::<Vec<_>>(),
                "count": providers.len(),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            // Human-readable table output
            println!("Compute Providers");
            println!("=================\n");

            // Show active filters
            let mut filters = Vec::new();
            if let Some(m) = model {
                filters.push(format!("model={}", m));
            }
            if let Some(r) = region {
                filters.push(format!("region={}", r));
            }
            if let Some(price) = max_price {
                filters.push(format!("max_price={}", price));
            }
            if let Some(trust) = min_trust {
                filters.push(format!("min_trust={:.2}", trust));
            }

            if !filters.is_empty() {
                println!("Filters: {}", filters.join(", "));
            }
            println!("Sort by: {}\n", sort);

            if providers.is_empty() {
                println!("No providers found matching criteria");
                println!("\nNote: Provider discovery from relays not yet implemented.");
                println!("      This is a placeholder showing the CLI structure.");
            } else {
                // Print table header
                println!(
                    "{:<16} {:<30} {:<15} {:<10} {:<8}",
                    "Pubkey", "Name", "Models", "Trust", "Recs"
                );
                println!("{}", "-".repeat(85));

                // Print providers
                for p in &providers {
                    let pubkey_short = if p.pubkey.len() > 16 {
                        format!("{}...", &p.pubkey[..13])
                    } else {
                        p.pubkey.clone()
                    };

                    let name = if p.metadata.name.len() > 30 {
                        format!("{}...", &p.metadata.name[..27])
                    } else {
                        p.metadata.name.clone()
                    };

                    let models = if p.capabilities.is_empty() {
                        "-".to_string()
                    } else if p.capabilities.len() == 1 {
                        p.capabilities[0].clone()
                    } else {
                        format!("{} models", p.capabilities.len())
                    };

                    println!(
                        "{:<16} {:<30} {:<15} {:<10.2} {:<8}",
                        pubkey_short, name, models, p.trust_score, p.recommendation_count
                    );
                }

                println!("\nTotal: {} providers", providers.len());
            }
        }

        Ok(())
    }

    fn submit(&self, params: SubmitParams) -> anyhow::Result<()> {
        let SubmitParams {
            job_type,
            prompt,
            file,
            model,
            budget,
            stream,
            json,
            target_language,
            local_first,
        } = params;
        // Validate input
        if prompt.is_none() && file.is_none() {
            anyhow::bail!("Either --prompt or --file must be provided");
        }

        let input = if let Some(file_path) = file {
            std::fs::read_to_string(file_path)?
        } else {
            prompt.unwrap().to_string()
        };

        // Create job request based on job type
        use crate::compute::events::ComputeJobRequest;
        let mut request = match job_type.to_lowercase().as_str() {
            "text-generation" => ComputeJobRequest::text_generation(&input)?,
            "summarization" => ComputeJobRequest::summarization(&input)?,
            "translation" => {
                let target_lang = target_language.unwrap_or("en");
                ComputeJobRequest::translation(&input, target_lang)?
            }
            "text-extraction" | "ocr" => ComputeJobRequest::text_extraction(&input)?,
            "image-generation" => ComputeJobRequest::image_generation(&input)?,
            "speech-to-text" => ComputeJobRequest::speech_to_text(&input)?,
            _ => anyhow::bail!("Unknown job type: {}", job_type),
        };

        // Add optional parameters
        if let Some(model) = model {
            request = request.with_model(model);
        }
        if let Some(budget) = budget {
            // Convert from sats to millisats
            request = request.with_bid(budget * 1000);
        }

        // Handle local-first fallback logic
        let model_name = model.unwrap_or("default");

        if local_first {
            // Try local inference first with fallback to swarm
            use crate::compute::fallback::{FallbackConfig, FallbackManager, FallbackResult};

            let fallback_config = FallbackConfig {
                enabled: true,
                max_price_msats: budget.map(|b| b * 1000), // Convert sats to msats
                local_timeout_secs: 30,
                force_local: false,
                force_swarm: false,
            };

            let manager = FallbackManager::new(fallback_config);

            // Execute with fallback using tokio runtime
            let rt = tokio::runtime::Runtime::new()?;
            let result =
                rt.block_on(async { manager.execute_with_fallback(model_name, &input).await })?;

            match result {
                FallbackResult::Local {
                    response,
                    duration_ms,
                } => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "completed",
                                "source": "local",
                                "response": response,
                                "duration_ms": duration_ms,
                            })
                        );
                    } else {
                        println!("✓ Completed locally in {}ms\n", duration_ms);
                        println!("{}", response);
                    }
                    return Ok(());
                }
                FallbackResult::Swarm {
                    job_id,
                    provider,
                    cost_msats,
                    duration_ms,
                } => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "submitted_to_swarm",
                                "source": "swarm",
                                "job_id": job_id,
                                "provider": provider,
                                "cost_msats": cost_msats,
                                "duration_ms": duration_ms,
                            })
                        );
                    } else {
                        println!("✓ Submitted to swarm (local unavailable)");
                        println!("Job ID: {}", job_id);
                        println!("Provider: {}", provider);
                        println!("Cost: {} msats ({} sats)", cost_msats, cost_msats / 1000);
                        println!("Duration: {}ms", duration_ms);
                    }
                    return Ok(());
                }
                FallbackResult::Failed {
                    local_error,
                    swarm_error,
                } => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "failed",
                                "local_error": local_error,
                                "swarm_error": swarm_error,
                            })
                        );
                    } else {
                        println!("✗ Job failed");
                        println!("Local error: {}", local_error);
                        if let Some(err) = swarm_error {
                            println!("Swarm error: {}", err);
                        }
                    }
                    anyhow::bail!("Job execution failed");
                }
            }
        }

        // Standard marketplace submission (no local-first)
        use crate::compute::consumer::Consumer;
        use crate::compute::db::JobDatabase;

        let consumer = Consumer::new();
        let mut handle = consumer.submit_job(request)?;

        // Save job to database for tracking
        let db = JobDatabase::new(None)?;
        db.save_job(&handle.info())?;

        if stream {
            // Streaming mode - wait for updates
            if json {
                // JSON streaming output
                println!(
                    "{{\"job_id\": \"{}\", \"status\": \"pending\"}}",
                    handle.job_id
                );
            } else {
                println!("Submitting {} job...", job_type);
                println!("Job ID: {}", handle.job_id);
                println!("\nWaiting for updates...");
            }

            // Use tokio runtime to handle async updates
            use crate::compute::consumer::JobUpdate;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                while let Some(update) = handle.next_update().await {
                    match update {
                        JobUpdate::StateChange { new_state, .. } => {
                            if json {
                                println!("{{\"status\": \"{:?}\"}}", new_state);
                            } else {
                                println!("Status: {:?}", new_state);
                            }
                        }
                        JobUpdate::PaymentRequired { amount_msats, bolt11, .. } => {
                            if json {
                                println!("{{\"status\": \"payment_required\", \"amount_msats\": {}, \"bolt11\": {}}}",
                                    amount_msats,
                                    bolt11.as_ref().map(|b| format!("\"{}\"", b)).unwrap_or("null".to_string())
                                );
                            } else {
                                println!("Payment required: {} msats", amount_msats);
                                if let Some(bolt11) = bolt11 {
                                    println!("Invoice: {}", bolt11);
                                }
                            }
                        }
                        JobUpdate::Processing { provider, extra, .. } => {
                            if json {
                                println!("{{\"status\": \"processing\", \"provider\": \"{}\"}}", provider);
                            } else {
                                println!("Processing by provider: {}", provider);
                                if let Some(extra) = extra {
                                    println!("  {}", extra);
                                }
                            }
                        }
                        JobUpdate::Partial { content, .. } => {
                            if json {
                                println!("{{\"partial\": \"{}\"}}", content.replace('"', "\\\""));
                            } else {
                                print!("{}", content);
                            }
                        }
                        JobUpdate::Completed { result, .. } => {
                            if json {
                                println!("{{\"status\": \"completed\", \"result\": \"{}\"}}",
                                    result.replace('"', "\\\""));
                            } else {
                                println!("\n\nResult:\n{}", result);
                            }
                            break;
                        }
                        JobUpdate::Failed { error, .. } => {
                            if json {
                                println!("{{\"status\": \"failed\", \"error\": \"{}\"}}",
                                    error.replace('"', "\\\""));
                            } else {
                                println!("\nError: {}", error);
                            }
                            break;
                        }
                    }
                }
            });
        } else {
            // Non-streaming mode - just return job ID
            if json {
                println!(
                    "{{\"job_id\": \"{}\", \"status\": \"pending\", \"type\": \"{}\"}}",
                    handle.job_id, job_type
                );
            } else {
                println!("Job submitted successfully");
                println!("Job ID: {}", handle.job_id);
                println!("Type: {}", job_type);
                if let Some(model) = model {
                    println!("Model: {}", model);
                }
                if let Some(budget) = budget {
                    println!("Budget: {} sats", budget);
                }
                println!("\nNote: Job tracking from relays not yet implemented.");
                println!("      Use 'status' command to check job status.");
            }
        }

        Ok(())
    }

    fn status(&self, job_id: &str, json: bool) -> anyhow::Result<()> {
        use crate::compute::db::JobDatabase;

        let db = JobDatabase::new(None)?;
        let job_opt = db.get_job(job_id)?;

        if let Some(job) = job_opt {
            if json {
                let json_str = serde_json::to_string_pretty(&job)?;
                println!("{}", json_str);
            } else {
                println!("Job Status");
                println!("==========\n");
                println!("Job ID: {}", job.job_id);
                println!("State: {:?}", job.state);
                println!("Submitted: {}", format_timestamp(job.submitted_at));

                if let Some(provider) = &job.provider {
                    println!("Provider: {}", provider);
                }

                if let Some(amount) = job.payment_amount {
                    println!("Payment: {} msats", amount);
                    if let Some(ref bolt11) = job.payment_bolt11 {
                        println!("Invoice: {}", bolt11);
                    }
                }

                if let Some(completed) = job.completed_at {
                    println!("Completed: {}", format_timestamp(completed));
                }

                if let Some(ref result) = job.result {
                    println!("\nResult:");
                    println!("{}", result);
                }

                if let Some(ref error) = job.error {
                    println!("\nError:");
                    println!("{}", error);
                }
            }
        } else if json {
            println!("{{\"error\": \"Job not found\"}}");
        } else {
            println!("Job ID: {}", job_id);
            println!("Status: Not found");
            println!("\nNote: Job may not have been submitted from this machine");
            println!("      or database tracking may not have been enabled.");
        }

        Ok(())
    }

    fn cancel(&self, job_id: &str) -> anyhow::Result<()> {
        println!("Cancelling job: {}", job_id);
        println!("Cancel not yet implemented");
        Ok(())
    }

    fn history(&self, json: bool, limit: Option<usize>) -> anyhow::Result<()> {
        use crate::compute::db::JobDatabase;

        let db = JobDatabase::new(None)?;
        let jobs = db.get_jobs(None, limit)?;

        if json {
            let json_data = serde_json::json!({
                "jobs": jobs,
                "count": jobs.len(),
            });
            println!("{}", serde_json::to_string_pretty(&json_data)?);
        } else {
            println!("Job History");
            println!("===========\n");

            if let Some(limit) = limit {
                println!("Showing last {} jobs\n", limit);
            }

            if jobs.is_empty() {
                println!("No jobs found");
                println!("\nJobs are tracked when submitted via this CLI.");
            } else {
                // Print table header
                println!(
                    "{:<20} {:<15} {:<20} {:<10}",
                    "Job ID", "State", "Submitted", "Provider"
                );
                println!("{}", "-".repeat(70));

                // Print jobs
                for job in &jobs {
                    let job_id_short = if job.job_id.len() > 20 {
                        format!("{}...", &job.job_id[..17])
                    } else {
                        job.job_id.clone()
                    };

                    let provider = job
                        .provider
                        .as_ref()
                        .map(|p| {
                            if p.len() > 10 {
                                format!("{}...", &p[..7])
                            } else {
                                p.clone()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<20} {:<15} {:<20} {:<10}",
                        job_id_short,
                        format!("{:?}", job.state),
                        format_timestamp(job.submitted_at),
                        provider
                    );
                }

                println!("\nTotal: {} jobs", jobs.len());
                println!("\nUse 'status <job-id>' for full job details");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_commands_variants_exist() {
        // Just verify the command structure is defined correctly
        let _providers = ComputeCommands::Providers {
            model: None,
            region: None,
            max_price: None,
            min_trust: None,
            sort: "trust".to_string(),
            json: false,
        };

        let _submit = ComputeCommands::Submit {
            job_type: "text-generation".to_string(),
            prompt: Some("test".to_string()),
            file: None,
            model: None,
            budget: None,
            stream: false,
            json: false,
            target_language: None,
            local_first: false,
        };

        let _status = ComputeCommands::Status {
            job_id: "test-id".to_string(),
            json: false,
        };

        let _cancel = ComputeCommands::Cancel {
            job_id: "test-id".to_string(),
        };

        let _history = ComputeCommands::History {
            json: false,
            limit: None,
        };
    }

    #[test]
    fn test_submit_requires_prompt_or_file() {
        let cmd = ComputeCommands::Submit {
            job_type: "text-generation".to_string(),
            prompt: None,
            file: None,
            model: None,
            budget: None,
            stream: false,
            json: false,
            target_language: None,
            local_first: false,
        };

        let result = cmd.execute();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Either --prompt or --file must be provided")
        );
    }

    #[test]
    fn test_providers_command_with_filters() {
        let cmd = ComputeCommands::Providers {
            model: Some("llama3".to_string()),
            region: Some("us-west".to_string()),
            max_price: Some(5000),
            min_trust: Some(0.5),
            sort: "trust".to_string(),
            json: false,
        };

        // Should execute without error (even though no providers found)
        let result = cmd.execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_providers_json_output() {
        let cmd = ComputeCommands::Providers {
            model: None,
            region: None,
            max_price: None,
            min_trust: None,
            sort: "trust".to_string(),
            json: true,
        };

        let result = cmd.execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_providers_sort_options() {
        // Test different sort options
        for sort in &["trust", "price", "recommendations", "recs"] {
            let cmd = ComputeCommands::Providers {
                model: None,
                region: None,
                max_price: None,
                min_trust: None,
                sort: sort.to_string(),
                json: false,
            };

            let result = cmd.execute();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_submit_text_generation() {
        let cmd = ComputeCommands::Submit {
            job_type: "text-generation".to_string(),
            prompt: Some("Hello world".to_string()),
            file: None,
            model: Some("llama3".to_string()),
            budget: Some(100),
            stream: false,
            json: false,
            target_language: None,
            local_first: false,
        };

        let result = cmd.execute();
        // Job submission returns error when not implemented per d-012 (No Stubs)
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_json_output() {
        let cmd = ComputeCommands::Submit {
            job_type: "summarization".to_string(),
            prompt: Some("Long text to summarize".to_string()),
            file: None,
            model: None,
            budget: None,
            stream: false,
            json: true,
            target_language: None,
            local_first: false,
        };

        let result = cmd.execute();
        // Job submission returns error when not implemented per d-012 (No Stubs)
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_unsupported_job_type() {
        let cmd = ComputeCommands::Submit {
            job_type: "unsupported-type".to_string(),
            prompt: Some("test".to_string()),
            file: None,
            model: None,
            budget: None,
            stream: false,
            json: false,
            target_language: None,
            local_first: false,
        };

        let result = cmd.execute();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown job type"));
    }
}
