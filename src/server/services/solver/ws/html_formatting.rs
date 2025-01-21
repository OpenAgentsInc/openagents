use super::types::SolverStage;
use serde_json::Value;

pub(crate) fn format_solution_html(
    files_reasoning: &str,
    files: &[String],
    solution_reasoning: &str,
    solution_text: &str,
) -> String {
    format!(
        r#"<div class='space-y-4'>
        <div class='bg-gray-800 rounded-lg p-4 mb-4'>
            <div class='text-sm text-yellow-400 mb-2'>File Selection Reasoning:</div>
            <div class='text-xs text-gray-300 whitespace-pre-wrap'>{}</div>
        </div>
        <div class='text-sm text-gray-400'>Relevant files:</div>
        <div class='max-w-4xl overflow-x-auto'>
            <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>
        </div>
        <div class='bg-gray-800 rounded-lg p-4 mb-4'>
            <div class='text-sm text-yellow-400 mb-2'>Solution Reasoning:</div>
            <div class='text-xs text-gray-300 whitespace-pre-wrap'>{}</div>
        </div>
        <div class='text-sm text-gray-400'>Proposed solution:</div>
        <div class='max-w-4xl overflow-x-auto'>
            <pre class='text-xs whitespace-pre-wrap break-words overflow-hidden'><code>{}</code></pre>
        </div>
        </div>"#,
        html_escape::encode_text(files_reasoning),
        html_escape::encode_text(&files.join("\n")),
        html_escape::encode_text(solution_reasoning),
        html_escape::encode_text(solution_text)
    )
}

pub(crate) fn render_progress_bar(stage: &SolverStage, message: &str) -> String {
    format!(
        r#"<div id="solver-progress" hx-swap-oob="true">
            <div class="progress-bar" style="width: {}%">
                {}
            </div>
        </div>
        <div id="solver-stage" hx-swap-oob="true">
            Stage {}: {}
        </div>"#,
        match stage {
            SolverStage::Init => 0,
            SolverStage::Repomap => 25,
            SolverStage::Analysis => 50,
            SolverStage::Solution => 75,
            SolverStage::PR => 90,
        },
        message,
        match stage {
            SolverStage::Init => "1/5",
            SolverStage::Repomap => "2/5",
            SolverStage::Analysis => "3/5",
            SolverStage::Solution => "4/5",
            SolverStage::PR => "5/5",
        },
        message
    )
}

pub(crate) fn render_files_list(files_list: &Value) -> String {
    format!(
        r#"<div id="solver-files-list" hx-swap-oob="innerHtml">
            <pre class="content-text">{}</pre>
        </div>"#,
        files_list
    )
}

pub(crate) fn render_files_reasoning(reasoning: &Value) -> String {
    format!(
        r#"<div id="solver-files-reasoning" hx-swap-oob="innerHtml">
            <pre class="content-text">{}</pre>
        </div>"#,
        reasoning
    )
}

pub(crate) fn render_solution(solution: &Value) -> String {
    format!(
        r#"<div id="solver-solution" hx-swap-oob="innerHtml">
            <pre class="content-text">{}</pre>
        </div>"#,
        solution
    )
}

pub(crate) fn render_solution_reasoning(reasoning: &Value) -> String {
    format!(
        r#"<div id="solver-solution-reasoning" hx-swap-oob="innerHtml">
            <pre class="content-text">{}</pre>
        </div>"#,
        reasoning
    )
}

pub(crate) fn render_complete(result: &Value) -> String {
    format!(
        r#"<div id="solver-progress" hx-swap-oob="true">
            <div class="progress-bar" style="width: 100%">
                Complete
            </div>
        </div>
        <div id="solver-status" hx-swap-oob="true">
            Solution complete
        </div>
        <div id="solver-final-result" hx-swap-oob="true">
            <pre class="solution-text">{}</pre>
        </div>"#,
        result["solution"]
    )
}

pub(crate) fn render_error(message: &str, details: &Option<String>) -> String {
    format!(
        r#"<div id="solver-error" hx-swap-oob="true">
            <div class="error">
                Error: {message}
                {}</div>
        </div>"#,
        details
            .as_ref()
            .map(|d| format!("<pre>{d}</pre>"))
            .unwrap_or_default()
    )
}

pub(crate) fn render_final_solution(solution: &str) -> String {
    format!(
        r#"<div id="solver-result" hx-swap-oob="true">
            <pre class="solution-text" style="white-space: pre-wrap; word-wrap: break-word; max-width: 100%; padding: 1em; background: #1a1a1a; border-radius: 4px;">
                {}
            </pre>
        </div>"#,
        solution
    )
}