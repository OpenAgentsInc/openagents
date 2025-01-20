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