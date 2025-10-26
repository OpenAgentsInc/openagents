use leptos::prelude::*;

#[cfg(feature = "jsonl_components")]
use crate::jsonl::{
    CommandExecutionCard, ExecBeginPayload, ExecBeginRow, FileChangeCard, FileChangeEntry, McpToolCallRow,
    MarkdownBlock, ReasoningCard, ReasoningHeadline, ThreadStartedRow, TodoItemPayload, TodoListCard,
    TurnEventRow, WebSearchRow, ErrorRow,
};

#[derive(Clone, Debug, PartialEq)]
pub enum LibraryPage {
    Markdown,
    UserMessage,
    ReasoningHeadline,
    ReasoningCard,
    Exec,
    FileChange,
    Command,
    SearchMcp,
    Todo,
    TurnError,
    Drawer,
    Unused,
}

#[derive(Clone, Debug)]
struct LibraryItem {
    title: &'static str,
    subtitle: &'static str,
    page: LibraryPage,
}

fn items() -> Vec<LibraryItem> {
    vec![
        LibraryItem { title: "MarkdownBlock", subtitle: "Fenced/inline code, lists, headers", page: LibraryPage::Markdown },
        LibraryItem { title: "UserMessageRow", subtitle: "User-authored content in the feed", page: LibraryPage::UserMessage },
        LibraryItem { title: "ReasoningHeadline", subtitle: "Top-line reasoning extraction + markdown", page: LibraryPage::ReasoningHeadline },
        LibraryItem { title: "ReasoningCard", subtitle: "Card with markdown + code", page: LibraryPage::ReasoningCard },
        LibraryItem { title: "ExecBeginRow", subtitle: "Parsed and raw command rows", page: LibraryPage::Exec },
        LibraryItem { title: "FileChangeCard", subtitle: "Summary (+/~/-) and list of changed files", page: LibraryPage::FileChange },
        LibraryItem { title: "CommandExecutionCard", subtitle: "Command output preview with collapsible body", page: LibraryPage::Command },
        LibraryItem { title: "WebSearch & MCP Call", subtitle: "Rows for web search queries and MCP tool calls", page: LibraryPage::SearchMcp },
        LibraryItem { title: "TodoListCard", subtitle: "Agent plan checklist with completion state", page: LibraryPage::Todo },
        LibraryItem { title: "Turn & Error Rows", subtitle: "Turn lifecycle events and surfaced errors", page: LibraryPage::TurnError },
        LibraryItem { title: "Drawer Components", subtitle: "Thread history row with count badge", page: LibraryPage::Drawer },
        LibraryItem { title: "Unused Samples", subtitle: "Hidden in feed; for reference", page: LibraryPage::Unused },
    ]
}

#[component]
pub fn LibrarySidebar(selected: ReadSignal<LibraryPage>, on_select: WriteSignal<LibraryPage>) -> impl IntoView {
    view! {
        <div class="threads">
            <div class="threads-title">"Component Library"</div>
            <div class="thread-list">
                { items().into_iter().map(|it| {
                    let page = it.page.clone();
                    let title = it.title;
                    let subtitle = it.subtitle;
                    view! {
                        <div class={ {
                            let cmp = page.clone();
                            move || if selected.get() == cmp { "thread-item selected" } else { "thread-item" }
                        } }
                             on:click={
                                let page = page.clone();
                                let on_select = on_select.clone();
                                move |_| on_select.set(page.clone())
                             }>
                            <div class="thread-title">{ title }</div>
                            <div class="muted" style="font-size:12px;">{ subtitle }</div>
                        </div>
                    }
                }).collect::<Vec<_>>() }
            </div>
        </div>
    }
}

#[component]
pub fn LibraryContent(page: ReadSignal<LibraryPage>) -> impl IntoView {
    view! {
        <div class="messages">
            <Show when=move || page.get() == LibraryPage::Markdown>
                {view! { <MarkdownSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::UserMessage>
                {view! { <UserMessageSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::ReasoningHeadline>
                {view! { <ReasoningHeadlineSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::ReasoningCard>
                {view! { <ReasoningCardSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::Exec>
                {view! { <ExecSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::FileChange>
                {view! { <FileChangeSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::Command>
                {view! { <CommandSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::SearchMcp>
                {view! { <SearchMcpSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::Todo>
                {view! { <TodoSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::TurnError>
                {view! { <TurnErrorSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::Drawer>
                {view! { <DrawerSample/> }}
            </Show>
            <Show when=move || page.get() == LibraryPage::Unused>
                {view! { <UnusedSample/> }}
            </Show>
        </div>
    }
}

#[component]
fn MarkdownSample() -> impl IntoView {
    let md = [
        "# Heading",
        "",
        "Some inline `code` and a list:",
        "",
        "- One",
        "- Two",
        "",
        "```ts",
        "const x: number = 42",
        "console.log(x)",
        "```",
    ].join("\n");
    view! { <div class="jsonl-card"><MarkdownBlock markdown=md/></div> }
}

#[component]
fn UserMessageSample() -> impl IntoView {
    let text = "Find all TODOs in the repo and suggest a plan to address them. Then propose a migration strategy.".to_string();
    view! { <div class="jsonl-card"><crate::jsonl::UserMessageRow text=text/></div> }
}

#[component]
fn ReasoningHeadlineSample() -> impl IntoView {
    let reasoning = "**Plan**\n\n- Parse lines\n- Render JSONL rows\n- Highlight code".to_string();
    view! { <div class="jsonl-card"><ReasoningHeadline text=reasoning/></div> }
}

#[component]
fn ReasoningCardSample() -> impl IntoView {
    let reasoning = "**Plan**\n\n- Parse lines\n- Render JSONL rows\n- Highlight code".to_string();
    view! { <ReasoningCard text=reasoning/> }
}

#[component]
fn ExecSample() -> impl IntoView {
    let payload1 = ExecBeginPayload { command: crate::jsonl::ExecCommandField::List(vec!["rg".into(), "-n".into(), "openagents".into()]), cwd: Some("/Users/me/code/openagents".into()), parsed: Some(serde_json::json!([{ "ListFiles": { "path": "docs" } }])) };
    let payload2 = ExecBeginPayload { command: crate::jsonl::ExecCommandField::One("git status -sb".into()), cwd: Some("/Users/me/code/openagents".into()), parsed: None };
    view! {
        <div class="jsonl-row"><ExecBeginRow payload=payload1.clone() full=true/></div>
        <div class="jsonl-row"><ExecBeginRow payload=payload2.clone() full=true/></div>
    }
}

#[component]
fn CommandSample() -> impl IntoView {
    view! {
        <CommandExecutionCard
            command="rg -n prism-react-renderer".to_string()
            status=Some("completed".into())
            exit_code=Some(0)
            sample=Some("README.md:12:prism-react-renderer".into())
            output_len=Some(24)
            collapsed=true
            max_body_height=120
        />
        <CommandExecutionCard
            command="rg -n prism-react-renderer".to_string()
            status=Some("completed".into())
            exit_code=Some(0)
            sample=Some("README.md:12:prism-react-renderer".into())
            output_len=Some(24)
            show_exit_code=true
            show_output_len=true
        />
    }
}

#[component]
fn FileChangeSample() -> impl IntoView {
    let changes = vec![
        FileChangeEntry { path: Some("expo/app/session/index.tsx".into()), kind: Some("update".into()) },
        FileChangeEntry { path: Some("expo/components/code-block.tsx".into()), kind: Some("add".into()) },
        FileChangeEntry { path: Some("docs/syntax-highlighting.md".into()), kind: Some("add".into()) },
        FileChangeEntry { path: Some("expo/components/jsonl/CommandExecutionCard.tsx".into()), kind: Some("update".into()) },
    ];
    view! { <FileChangeCard status=Some("completed".into()) changes=changes limit=Some(8) /> }
}

#[component]
fn SearchMcpSample() -> impl IntoView {
    view! {
        <div class="jsonl-row"><WebSearchRow query="prism-react-renderer themes".to_string()/></div>
        <div class="jsonl-row"><McpToolCallRow server="github".to_string() tool="search".to_string() status=Some("completed".into())/></div>
    }
}

#[component]
fn TodoSample() -> impl IntoView {
    let items = vec![
        TodoItemPayload { text: Some("Wire up Prism in Markdown".into()), completed: Some(true) },
        TodoItemPayload { text: Some("Show raw JSON in detail".into()), completed: Some(true) },
        TodoItemPayload { text: Some("Add more samples".into()), completed: Some(false) },
    ];
    view! { <TodoListCard status=Some("updated".into()) items=items /> }
}

#[component]
fn TurnErrorSample() -> impl IntoView {
    view! {
        <div class="jsonl-row"><TurnEventRow phase=Some("completed".into()) usage=Some(crate::jsonl::TurnUsagePayload { input_tokens: Some(1200), cached_input_tokens: Some(300), output_tokens: Some(420) }) message=None show_usage=true duration_ms=None/></div>
        <div class="jsonl-row"><ErrorRow message="Something went wrong while fetching.".to_string()/></div>
    }
}

#[component]
fn DrawerSample() -> impl IntoView {
    // The Tauri app doesn't reuse the Expo drawer components; approximate using existing styles.
    let now = js_sys::Date::new_0().get_time();
    let _ = now; // not displayed, purely demonstrative
    view! {
        <div class="thread-list">
            <div class="thread-item"><div class="thread-title">{"New Thread"}</div></div>
            <div class="thread-item"><div class="thread-title">{"Bug bash"}</div></div>
        </div>
    }
}

#[component]
fn UnusedSample() -> impl IntoView {
    view! {
        <div class="jsonl-row"><ThreadStartedRow thread_id="abcd1234".to_string()/></div>
        <div class="jsonl-row"><TurnEventRow phase=Some("started".into()) usage=None message=None show_usage=false duration_ms=None/></div>
    }
}
//! Component library showcase for desktop JSONL components.
//!
//! Provides a simple sidebar + content view that demonstrates how each JSONL
//! row type renders with example data, mirroring the mobile Library screen.
