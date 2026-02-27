use super::*;

impl Storybook {
    pub(crate) fn paint_chat_threads(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let entry_gap = 12.0;

        // ========== Panel 1: Simple Conversation ==========
        let simple_height = panel_height(480.0);
        let multi_height = panel_height(600.0);
        let edit_height = panel_height(520.0);
        let search_height = panel_height(440.0);
        let stream_height = panel_height(320.0);
        let complex_height = panel_height(800.0);
        let error_height = panel_height(280.0);
        let panels = panel_stack(
            bounds,
            &[
                simple_height,
                multi_height,
                edit_height,
                search_height,
                stream_height,
                complex_height,
                error_height,
            ],
        );
        let simple_bounds = panels[0];
        draw_panel("Simple Conversation", simple_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User message
            let user_h = 100.0;
            let mut user_msg =
                UserMessage::new("Can you help me understand how async/await works in Rust?")
                    .timestamp("10:30 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Assistant response
            let asst_h = 180.0;
            let mut asst_msg = AssistantMessage::new(
                "Async/await in Rust allows you to write asynchronous code that looks synchronous. \
                 The key concepts are:\n\n\
                 1. `async fn` - declares a function that returns a Future\n\
                 2. `.await` - suspends execution until the Future completes\n\
                 3. An executor (like tokio) runs these Futures to completion\n\n\
                 Would you like me to show you a practical example?",
            )
            .model(Model::CodexSonnet)
            .timestamp("10:30 AM");
            asst_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, asst_h), cx);
            entry_y += asst_h + entry_gap;

            // Follow-up user message
            let user2_h = 80.0;
            let mut user_msg2 =
                UserMessage::new("Yes please! Show me a simple HTTP request example.")
                    .timestamp("10:31 AM");
            user_msg2.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user2_h), cx);
        });

        // ========== Panel 2: Multi-Tool Workflow ==========
        let multi_bounds = panels[1];
        draw_panel("Multi-Tool Workflow", multi_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 80.0;
            let mut user_msg =
                UserMessage::new("Find all TODO comments in the codebase and list them")
                    .timestamp("2:15 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Assistant thinking + tool call
            let asst_h = 60.0;
            let mut asst_msg = AssistantMessage::new("I'll search the codebase for TODO comments.")
                .model(Model::CodexSonnet)
                .timestamp("2:15 PM");
            asst_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, asst_h), cx);
            entry_y += asst_h + entry_gap;

            // Search tool call
            let search_h = 200.0;
            let mut search_tool = SearchToolCall::new("TODO")
                .status(ToolStatus::Success)
                .matches(vec![
                    SearchMatch {
                        file: "src/main.rs".into(),
                        line: 42,
                        content: "TODO: Add error handling".into(),
                    },
                    SearchMatch {
                        file: "src/lib.rs".into(),
                        line: 78,
                        content: "TODO: Implement caching".into(),
                    },
                    SearchMatch {
                        file: "src/utils.rs".into(),
                        line: 15,
                        content: "TODO: Refactor this function".into(),
                    },
                    SearchMatch {
                        file: "tests/integration.rs".into(),
                        line: 23,
                        content: "TODO: Add more test cases".into(),
                    },
                ]);
            search_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, search_h), cx);
            entry_y += search_h + entry_gap;

            // Terminal tool for file count
            let term_h = 100.0;
            let mut term_tool = TerminalToolCall::new("wc -l $(grep -rl 'TODO' src/)")
                .status(ToolStatus::Success)
                .exit_code(0)
                .output("  42 src/main.rs\n  78 src/lib.rs\n  15 src/utils.rs\n 135 total");
            term_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, term_h), cx);
            entry_y += term_h + entry_gap;

            // Final summary
            let summary_h = 80.0;
            let mut summary = AssistantMessage::new(
                "Found 4 TODO comments across 3 source files totaling 135 lines. \
                 The main areas needing attention are error handling, caching, and test coverage.",
            )
            .model(Model::CodexSonnet)
            .timestamp("2:16 PM");
            summary.paint(Bounds::new(inner.origin.x, entry_y, entry_w, summary_h), cx);
        });

        // ========== Panel 3: Code Editing Session ==========
        let edit_bounds = panels[2];
        draw_panel("Code Editing Session", edit_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Add error handling to the process_data function")
                .timestamp("3:42 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Read tool
            let read_h = 100.0;
            let mut read_tool = ToolCallCard::new(ToolType::Read, "read_file")
                .status(ToolStatus::Success)
                .input("path: src/processor.rs")
                .output("Read 156 lines (4.2 KB)");
            read_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, read_h), cx);
            entry_y += read_h + entry_gap;

            // Diff tool showing the edit
            let diff_h = 180.0;
            let mut diff_tool = DiffToolCall::new("src/processor.rs")
                .status(ToolStatus::Success)
                .diff_type(DiffType::Unified)
                .lines(vec![
                    DiffLine {
                        kind: DiffLineKind::Header,
                        content: "@@ -45,6 +45,12 @@".into(),
                        old_line: None,
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Context,
                        content: "fn process_data(input: &str) -> String {".into(),
                        old_line: Some(45),
                        new_line: Some(45),
                    },
                    DiffLine {
                        kind: DiffLineKind::Deletion,
                        content: "    input.parse().unwrap()".into(),
                        old_line: Some(46),
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    match input.parse() {".into(),
                        old_line: None,
                        new_line: Some(46),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        Ok(val) => val,".into(),
                        old_line: None,
                        new_line: Some(47),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        Err(e) => {".into(),
                        old_line: None,
                        new_line: Some(48),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "            eprintln!(\"Parse error: {}\", e);".into(),
                        old_line: None,
                        new_line: Some(49),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "            String::new()".into(),
                        old_line: None,
                        new_line: Some(50),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        }".into(),
                        old_line: None,
                        new_line: Some(51),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    }".into(),
                        old_line: None,
                        new_line: Some(52),
                    },
                    DiffLine {
                        kind: DiffLineKind::Context,
                        content: "}".into(),
                        old_line: Some(47),
                        new_line: Some(53),
                    },
                ]);
            diff_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, diff_h), cx);
            entry_y += diff_h + entry_gap;

            // Completion message
            let complete_h = 80.0;
            let mut complete = AssistantMessage::new(
                "I've added proper error handling with a match statement. The function now logs \
                 parse errors to stderr and returns an empty string instead of panicking.",
            )
            .model(Model::CodexSonnet)
            .timestamp("3:43 PM");
            complete.paint(
                Bounds::new(inner.origin.x, entry_y, entry_w, complete_h),
                cx,
            );
        });

        // ========== Panel 4: Search & Navigation ==========
        let search_bounds = panels[3];
        draw_panel("Search & Navigation", search_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User question
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Where is the authentication logic implemented?")
                .timestamp("11:05 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Glob search
            let glob_h = 100.0;
            let mut glob_tool = ToolCallCard::new(ToolType::Search, "glob")
                .status(ToolStatus::Success)
                .input("pattern: **/auth*.rs")
                .output("Found 3 files");
            glob_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, glob_h), cx);
            entry_y += glob_h + entry_gap;

            // Grep search
            let grep_h = 180.0;
            let mut grep_tool = SearchToolCall::new("fn authenticate")
                .status(ToolStatus::Success)
                .matches(vec![
                    SearchMatch {
                        file: "src/auth/mod.rs".into(),
                        line: 12,
                        content: "pub fn authenticate(token: &str) -> Result<User, AuthError>"
                            .into(),
                    },
                    SearchMatch {
                        file: "src/auth/jwt.rs".into(),
                        line: 45,
                        content: "fn authenticate_jwt(token: &str) -> Result<Claims, JwtError>"
                            .into(),
                    },
                    SearchMatch {
                        file: "src/middleware/auth.rs".into(),
                        line: 28,
                        content: "async fn authenticate(req: Request) -> Result<Response, Error>"
                            .into(),
                    },
                ]);
            grep_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, grep_h), cx);
            entry_y += grep_h + entry_gap;

            // Answer
            let answer_h = 60.0;
            let mut answer = AssistantMessage::new(
                "Authentication is handled in `src/auth/mod.rs:12` with JWT validation in `src/auth/jwt.rs`."
            )
            .model(Model::CodexSonnet)
            .timestamp("11:05 AM");
            answer.paint(Bounds::new(inner.origin.x, entry_y, entry_w, answer_h), cx);
        });

        // ========== Panel 5: Streaming Response ==========
        let stream_bounds = panels[4];
        draw_panel("Streaming Response", stream_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User message
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Explain the visitor pattern in software design")
                .timestamp("4:20 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Streaming assistant response
            let stream_h = 200.0;
            let mut stream_msg = AssistantMessage::new(
                "The Visitor pattern is a behavioral design pattern that lets you separate algorithms \
                 from the objects they operate on. It's useful when you have a complex object structure \
                 and want to perform operations on it without modifying the classes.\n\n\
                 Key components:\n\
                 - **Element**: objects being visited\n\
                 - **Visitor**: defines operations..."
            )
            .model(Model::CodexOpus)
            .streaming(true)
            .timestamp("4:20 PM");
            stream_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, stream_h), cx);
        });

        // ========== Panel 6: Complex Agent Session ==========
        let complex_bounds = panels[5];
        draw_panel("Complex Agent Session", complex_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 100.0;
            let mut user_msg = UserMessage::new(
                "Create a new API endpoint for user preferences with GET and POST methods. \
                 Include input validation and proper error responses.",
            )
            .timestamp("9:00 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Planning response
            let plan_h = 100.0;
            let mut plan = AssistantMessage::new(
                "I'll create the preferences endpoint. Here's my plan:\n\
                 1. Create preference types and validation\n\
                 2. Add route handlers for GET/POST\n\
                 3. Write integration tests\n\
                 4. Update API documentation",
            )
            .model(Model::CodexSonnet)
            .timestamp("9:00 AM");
            plan.paint(Bounds::new(inner.origin.x, entry_y, entry_w, plan_h), cx);
            entry_y += plan_h + entry_gap;

            // Write tool - creating new file
            let write_h = 100.0;
            let mut write_tool = ToolCallCard::new(ToolType::Write, "write_file")
                .status(ToolStatus::Success)
                .input("path: src/api/preferences.rs")
                .output("Created file (78 lines)");
            write_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, write_h), cx);
            entry_y += write_h + entry_gap;

            // Edit existing file
            let edit_h = 100.0;
            let mut edit_tool = ToolCallCard::new(ToolType::Edit, "edit_file")
                .status(ToolStatus::Success)
                .input("path: src/api/mod.rs")
                .output("Added route registration (+3 lines)");
            edit_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, edit_h), cx);
            entry_y += edit_h + entry_gap;

            // Run tests
            let test_h = 120.0;
            let mut test_tool = TerminalToolCall::new("cargo test preferences")
                .status(ToolStatus::Success)
                .exit_code(0)
                .output(
                    "running 5 tests\ntest api::preferences::tests::test_get_prefs ... ok\n\
                         test api::preferences::tests::test_post_prefs ... ok\n\
                         test api::preferences::tests::test_validation ... ok\n\n\
                         test result: ok. 5 passed; 0 failed",
                );
            test_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, test_h), cx);
            entry_y += test_h + entry_gap;

            // Documentation update
            let doc_h = 100.0;
            let mut doc_tool = ToolCallCard::new(ToolType::Edit, "edit_file")
                .status(ToolStatus::Success)
                .input("path: docs/api.md")
                .output("Updated API docs (+45 lines)");
            doc_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, doc_h), cx);
            entry_y += doc_h + entry_gap;

            // Summary
            let summary_h = 100.0;
            let mut summary = AssistantMessage::new(
                "The preferences API is now complete:\n\
                 - `GET /api/preferences` returns user preferences\n\
                 - `POST /api/preferences` updates them with validation\n\
                 - All 5 tests pass and documentation is updated",
            )
            .model(Model::CodexSonnet)
            .timestamp("9:03 AM");
            summary.paint(Bounds::new(inner.origin.x, entry_y, entry_w, summary_h), cx);
        });

        // ========== Panel 7: Error Handling ==========
        let error_bounds = panels[6];
        draw_panel("Error Handling", error_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 60.0;
            let mut user_msg =
                UserMessage::new("Run the database migration script").timestamp("5:30 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Failed terminal command
            let term_h = 100.0;
            let mut term_tool = TerminalToolCall::new("./scripts/migrate.sh")
                .status(ToolStatus::Error)
                .exit_code(1)
                .output("Error: Connection refused\nDatabase server not running at localhost:5432");
            term_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, term_h), cx);
            entry_y += term_h + entry_gap;

            // Error response
            let error_h = 80.0;
            let mut error_msg = AssistantMessage::new(
                "The migration failed because the database server isn't running. \
                 Please start PostgreSQL with `sudo systemctl start postgresql` and try again.",
            )
            .model(Model::CodexSonnet)
            .timestamp("5:30 PM");
            error_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, error_h), cx);
        });
    }
}
