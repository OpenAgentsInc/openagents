//! Export routes for RLM dashboard.

use worker::*;

use crate::db::rlm;
use crate::AuthenticatedUser;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportFormat {
    Json,
    Csv,
}

impl ExportFormat {
    pub fn from_param(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("csv") => ExportFormat::Csv,
            _ => ExportFormat::Json,
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            ExportFormat::Json => "application/json; charset=utf-8",
            ExportFormat::Csv => "text/csv; charset=utf-8",
        }
    }
}

#[derive(serde::Serialize)]
struct ExperimentRunExport {
    run: rlm::RlmExperimentRunRecord,
    trace_events: Vec<rlm::RlmTraceEventRecord>,
}

#[derive(serde::Serialize)]
struct ExperimentExportPayload {
    experiment: rlm::RlmExperimentRecord,
    runs: Vec<ExperimentRunExport>,
}

/// GET /api/rlm/experiments/:id/export
pub async fn export_experiment(
    user: AuthenticatedUser,
    env: Env,
    experiment_id: String,
    format: ExportFormat,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let experiment = rlm::get_experiment(&db, &user.user_id, &experiment_id).await?;
    let Some(experiment) = experiment else {
        return Response::error("Experiment not found", 404);
    };

    match format {
        ExportFormat::Json => {
            let runs = rlm::list_experiment_runs(&db, &user.user_id, &experiment_id).await?;
            let mut export_runs = Vec::with_capacity(runs.len());
            for run in runs.iter() {
                let trace_events = rlm::list_trace_events(&db, &run.run_id).await?;
                export_runs.push(ExperimentRunExport {
                    run: run.clone(),
                    trace_events,
                });
            }

            let payload = ExperimentExportPayload {
                experiment: experiment.clone(),
                runs: export_runs,
            };
            let mut resp = Response::from_json(&payload)?;
            let headers = resp.headers_mut();
            headers.set("Content-Type", format.content_type())?;
            headers.set(
                "Content-Disposition",
                &format!("attachment; filename=rlm-experiment-{}.json", experiment_id),
            )?;
            Ok(resp)
        }
        ExportFormat::Csv => {
            let runs = rlm::list_experiment_runs(&db, &user.user_id, &experiment_id).await?;
            let csv = build_experiment_csv(&experiment, &runs);
            let mut resp = Response::ok(csv)?;
            let headers = resp.headers_mut();
            headers.set("Content-Type", format.content_type())?;
            headers.set(
                "Content-Disposition",
                &format!("attachment; filename=rlm-experiment-{}.csv", experiment_id),
            )?;
            Ok(resp)
        }
    }
}

fn build_experiment_csv(
    experiment: &rlm::RlmExperimentRecord,
    runs: &[rlm::RlmExperimentRunRecord],
) -> String {
    let mut out = String::new();
    out.push_str("experiment_id,experiment_name,run_id,label,status,query,fragment_count,budget_sats,total_cost_sats,total_duration_ms,created_at,completed_at,error_message\n");

    for run in runs {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            escape_csv(&experiment.id),
            escape_csv(&experiment.name),
            escape_csv(&run.run_id),
            escape_csv(run.label.as_deref().unwrap_or("")),
            escape_csv(&run.status),
            escape_csv(&run.query),
            run.fragment_count,
            run.budget_sats,
            run.total_cost_sats,
            run.total_duration_ms,
            run.created_at,
            run.completed_at.unwrap_or_default(),
            escape_csv(run.error_message.as_deref().unwrap_or("")),
        ));
    }

    out
}

fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}
