use wgpui::components::atoms::{
    AgentScheduleBadge, AgentStatus, AgentStatusBadge, AgentType, AmountDirection, ApmGauge,
    Bech32Entity, Bech32Type, BitcoinAmount, BitcoinNetwork, BitcoinUnit, BountyBadge,
    BountyStatus, ContributionStatus, DaemonStatus, DaemonStatusBadge, EarningsBadge, EarningsType,
    EventKind, EventKindBadge, GoalPriority, GoalProgressBadge, GoalStatus, IssueStatus,
    IssueStatusBadge, JobStatus, JobStatusBadge, LicenseStatus, MarketType, MarketTypeBadge, Model,
    NetworkBadge, ParallelAgentBadge, ParallelAgentStatus, PaymentMethod, PaymentMethodIcon,
    PaymentStatus, PaymentStatusBadge, PrStatus, PrStatusBadge, RelayStatus, RelayStatusBadge,
    RelayStatusDot, ReputationBadge, ResourceType, ResourceUsageBar, SessionStatus,
    SessionStatusBadge, SkillLicenseBadge, SkillType, StackLayerBadge, StackLayerStatus,
    ThresholdKeyBadge, TickEventBadge, TickOutcome, ToolStatus, ToolType, TrajectorySource,
    TrajectorySourceBadge, TrajectoryStatus, TrajectoryStatusBadge, TriggerType, TrustTier,
};
use wgpui::components::molecules::{
    BalanceCard, DiffType, InvoiceDisplay, InvoiceInfo, InvoiceType, PaymentDirection, PaymentInfo,
    PaymentRow, RelayInfo, RelayRow, WalletBalance,
};
use wgpui::components::organisms::{
    AssistantMessage, DiffLine, DiffLineKind, DiffToolCall, SearchMatch, SearchToolCall,
    TerminalToolCall, ToolCallCard, UserMessage,
};
use wgpui::{Bounds, Component, PaintContext, Point, Quad, theme};

use crate::constants::SECTION_GAP;
use crate::helpers::{draw_panel, panel_height, panel_stack};
use crate::state::Storybook;

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

    pub(crate) fn paint_bitcoin_wallet(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // ========== Panel 1: Payment Method Icons ==========
        let methods_height = panel_height(200.0);
        let status_height = panel_height(180.0);
        let network_height = panel_height(160.0);
        let amounts_height = panel_height(200.0);
        let balance_height = panel_height(220.0);
        let txn_height = panel_height(300.0);
        let invoice_height = panel_height(320.0);
        let dashboard_height = panel_height(400.0);
        let panels = panel_stack(
            bounds,
            &[
                methods_height,
                status_height,
                network_height,
                amounts_height,
                balance_height,
                txn_height,
                invoice_height,
                dashboard_height,
            ],
        );
        let methods_bounds = panels[0];
        draw_panel("Payment Method Icons", methods_bounds, cx, |inner, cx| {
            let methods = [
                PaymentMethod::Lightning,
                PaymentMethod::Spark,
                PaymentMethod::OnChain,
                PaymentMethod::Token,
                PaymentMethod::Deposit,
                PaymentMethod::Withdraw,
            ];

            let tile_w = 140.0;
            let tile_h = 50.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, method) in methods.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Draw tile background
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::MUTED)
                        .with_border(method.color(), 1.0),
                );

                // Draw icon with label
                let mut icon = PaymentMethodIcon::new(*method).size(24.0).show_label(true);
                icon.paint(
                    Bounds::new(tile_x + 12.0, tile_y + 14.0, tile_w - 24.0, 24.0),
                    cx,
                );
            }
        });

        // ========== Panel 2: Payment Status Badges ==========
        let status_bounds = panels[1];
        draw_panel("Payment Status Badges", status_bounds, cx, |inner, cx| {
            let statuses = [
                (PaymentStatus::Pending, "Awaiting confirmation..."),
                (PaymentStatus::Completed, "Successfully sent!"),
                (PaymentStatus::Failed, "Transaction rejected"),
                (PaymentStatus::Expired, "Invoice expired"),
            ];

            let tile_w = 200.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, desc)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Status badge
                let mut badge = PaymentStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x + 8.0, tile_y + 8.0, 72.0, 20.0), cx);

                // Description
                let desc_run = cx.text.layout(
                    *desc,
                    Point::new(tile_x + 8.0, tile_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_run);
            }
        });

        // ========== Panel 3: Network Badges ==========
        let network_bounds = panels[2];
        draw_panel("Bitcoin Networks", network_bounds, cx, |inner, cx| {
            let networks = [
                (BitcoinNetwork::Mainnet, "Production - Real money!"),
                (BitcoinNetwork::Testnet, "Testing - Free test sats"),
                (BitcoinNetwork::Signet, "Staging - Controlled testnet"),
                (BitcoinNetwork::Regtest, "Local dev - Private network"),
            ];

            let tile_w = 220.0;
            let tile_h = 48.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (network, desc)) in networks.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let mut badge = NetworkBadge::new(*network);
                badge.paint(Bounds::new(tile_x + 8.0, tile_y + 14.0, 64.0, 20.0), cx);

                let desc_run = cx.text.layout(
                    *desc,
                    Point::new(tile_x + 80.0, tile_y + 16.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_run);
            }
        });

        // ========== Panel 4: Bitcoin Amounts ==========
        let amounts_bounds = panels[3];
        draw_panel(
            "Bitcoin Amount Formatting",
            amounts_bounds,
            cx,
            |inner, cx| {
                let amounts_data = [
                    (
                        1000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Sats,
                        "Small amount",
                    ),
                    (
                        50000,
                        AmountDirection::Incoming,
                        BitcoinUnit::Sats,
                        "Incoming payment",
                    ),
                    (
                        25000,
                        AmountDirection::Outgoing,
                        BitcoinUnit::Sats,
                        "Outgoing payment",
                    ),
                    (
                        100_000_000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Btc,
                        "One Bitcoin",
                    ),
                    (
                        2_100_000_000_000_000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Btc,
                        "Max supply",
                    ),
                ];

                let row_h = 32.0;
                let gap = 8.0;

                for (idx, (sats, direction, unit, label)) in amounts_data.iter().enumerate() {
                    let row_y = inner.origin.y + idx as f32 * (row_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(inner.origin.x, row_y + 8.0),
                        theme::font_size::SM,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Amount
                    let mut amount = BitcoinAmount::new(*sats)
                        .direction(*direction)
                        .unit(*unit)
                        .font_size(theme::font_size::LG);
                    amount.paint(Bounds::new(inner.origin.x + 180.0, row_y, 200.0, row_h), cx);
                }
            },
        );

        // ========== Panel 5: Balance Cards ==========
        let balance_bounds = panels[4];
        draw_panel("Wallet Balance Cards", balance_bounds, cx, |inner, cx| {
            // Mainnet balance
            let mainnet_balance = WalletBalance::new(150000, 75000, 25000);
            let mut mainnet_card = BalanceCard::new(mainnet_balance)
                .network(BitcoinNetwork::Mainnet)
                .show_breakdown(true);
            mainnet_card.paint(
                Bounds::new(inner.origin.x, inner.origin.y, 300.0, 180.0),
                cx,
            );

            // Testnet balance
            let testnet_balance = WalletBalance::new(1_000_000, 500_000, 0);
            let mut testnet_card = BalanceCard::new(testnet_balance)
                .network(BitcoinNetwork::Testnet)
                .show_breakdown(true);
            let card_x = inner.origin.x + 320.0;
            if card_x + 300.0 <= inner.origin.x + inner.size.width {
                testnet_card.paint(Bounds::new(card_x, inner.origin.y, 300.0, 180.0), cx);
            }
        });

        // ========== Panel 6: Payment Rows (Transaction History) ==========
        let txn_bounds = panels[5];
        draw_panel("Transaction History", txn_bounds, cx, |inner, cx| {
            let transactions = [
                PaymentInfo::new("tx1", 50000, PaymentDirection::Receive)
                    .method(PaymentMethod::Lightning)
                    .status(PaymentStatus::Completed)
                    .timestamp("Dec 25, 10:30 AM")
                    .description("Zap from @alice"),
                PaymentInfo::new("tx2", 25000, PaymentDirection::Send)
                    .method(PaymentMethod::Spark)
                    .status(PaymentStatus::Completed)
                    .fee(10)
                    .timestamp("Dec 24, 3:15 PM")
                    .description("Coffee payment"),
                PaymentInfo::new("tx3", 100000, PaymentDirection::Receive)
                    .method(PaymentMethod::OnChain)
                    .status(PaymentStatus::Pending)
                    .timestamp("Dec 24, 1:00 PM")
                    .description("On-chain deposit"),
                PaymentInfo::new("tx4", 15000, PaymentDirection::Send)
                    .method(PaymentMethod::Lightning)
                    .status(PaymentStatus::Failed)
                    .timestamp("Dec 23, 8:45 PM")
                    .description("Invoice expired"),
            ];

            let row_h = 60.0;
            let gap = 8.0;

            for (idx, payment) in transactions.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                let mut row = PaymentRow::new(payment.clone());
                row.paint(
                    Bounds::new(inner.origin.x, row_y, inner.size.width, row_h),
                    cx,
                );
            }
        });

        // ========== Panel 7: Invoice Displays ==========
        let invoice_bounds = panels[6];
        draw_panel(
            "Invoice & Address Displays",
            invoice_bounds,
            cx,
            |inner, cx| {
                // Lightning invoice
                let ln_invoice = InvoiceInfo::new(
                    InvoiceType::Bolt11,
                    "lnbc500u1pn9xnxhpp5e5wfyknkdxqmz9f0vs4j8kqz3h5qf7c4xhp2s5ngrqj6u4m8qz",
                )
                .amount(50000)
                .description("Payment for services")
                .expiry("10 minutes")
                .status(PaymentStatus::Pending);
                let mut ln_display = InvoiceDisplay::new(ln_invoice).show_qr(true);
                ln_display.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, 320.0, 280.0),
                    cx,
                );

                // Spark address (compact)
                let spark_addr = InvoiceInfo::new(
                    InvoiceType::SparkAddress,
                    "sp1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
                )
                .status(PaymentStatus::Pending);
                let mut spark_display =
                    InvoiceDisplay::new(spark_addr).show_qr(false).compact(true);
                let spark_x = inner.origin.x + 340.0;
                if spark_x + 320.0 <= inner.origin.x + inner.size.width {
                    spark_display.paint(Bounds::new(spark_x, inner.origin.y, 320.0, 120.0), cx);
                }

                // Bitcoin address
                let btc_addr = InvoiceInfo::new(
                    InvoiceType::OnChainAddress,
                    "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                )
                .status(PaymentStatus::Pending);
                let mut btc_display = InvoiceDisplay::new(btc_addr).show_qr(false).compact(true);
                if spark_x + 320.0 <= inner.origin.x + inner.size.width {
                    btc_display.paint(
                        Bounds::new(spark_x, inner.origin.y + 140.0, 320.0, 120.0),
                        cx,
                    );
                }
            },
        );

        // ========== Panel 8: Complete Wallet Dashboard ==========
        let dashboard_bounds = panels[7];
        draw_panel(
            "Complete Wallet Dashboard",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Left column: Balance card
                let col_w = (inner.size.width - 20.0) / 2.0;

                let balance = WalletBalance::new(250000, 100000, 50000);
                let mut balance_card = BalanceCard::new(balance)
                    .network(BitcoinNetwork::Mainnet)
                    .show_breakdown(true);
                balance_card.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, col_w.min(320.0), 180.0),
                    cx,
                );

                // Below balance: Quick actions hints
                let actions_y = inner.origin.y + 200.0;
                let actions = ["Send Payment", "Receive", "History", "Settings"];
                let btn_w = 100.0;
                let btn_h = 32.0;
                let btn_gap = 12.0;

                for (idx, action) in actions.iter().enumerate() {
                    let btn_x = inner.origin.x + idx as f32 * (btn_w + btn_gap);
                    if btn_x + btn_w > inner.origin.x + col_w {
                        break;
                    }

                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(btn_x, actions_y, btn_w, btn_h))
                            .with_background(theme::bg::MUTED)
                            .with_border(theme::border::DEFAULT, 1.0),
                    );

                    let btn_text = cx.text.layout(
                        *action,
                        Point::new(btn_x + 8.0, actions_y + 8.0),
                        theme::font_size::XS,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(btn_text);
                }

                // Right column: Recent transactions
                let right_x = inner.origin.x + col_w + 20.0;
                if right_x + col_w <= inner.origin.x + inner.size.width {
                    let header_run = cx.text.layout(
                        "Recent Transactions",
                        Point::new(right_x, inner.origin.y),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(header_run);

                    let recent = [
                        PaymentInfo::new("r1", 10000, PaymentDirection::Receive)
                            .method(PaymentMethod::Lightning)
                            .status(PaymentStatus::Completed)
                            .timestamp("Just now"),
                        PaymentInfo::new("r2", 5000, PaymentDirection::Send)
                            .method(PaymentMethod::Spark)
                            .status(PaymentStatus::Completed)
                            .timestamp("5 min ago"),
                        PaymentInfo::new("r3", 75000, PaymentDirection::Receive)
                            .method(PaymentMethod::OnChain)
                            .status(PaymentStatus::Pending)
                            .timestamp("1 hour ago"),
                    ];

                    let row_h = 56.0;
                    let gap = 4.0;
                    let txn_y = inner.origin.y + 28.0;

                    for (idx, payment) in recent.iter().enumerate() {
                        let row_y = txn_y + idx as f32 * (row_h + gap);
                        let mut row = PaymentRow::new(payment.clone()).show_fee(false);
                        row.paint(Bounds::new(right_x, row_y, col_w.min(400.0), row_h), cx);
                    }
                }
            },
        );
    }

    pub(crate) fn paint_nostr_protocol(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Relay Status Indicators ==========
        let status_height = panel_height(160.0);
        let status_bounds = Bounds::new(bounds.origin.x, y, width, status_height);
        draw_panel("Relay Status Indicators", status_bounds, cx, |inner, cx| {
            let statuses = [
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
                RelayStatus::Authenticating,
            ];

            // Row 1: Status dots
            let mut dot_x = inner.origin.x;
            let dot_y = inner.origin.y;
            let dot_run = cx.text.layout(
                "Status Dots:",
                Point::new(dot_x, dot_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(dot_run);

            dot_x = inner.origin.x;
            for status in &statuses {
                let mut dot = RelayStatusDot::new(*status).size(12.0).show_label(true);
                dot.paint(Bounds::new(dot_x, dot_y + 20.0, 60.0, 16.0), cx);
                dot_x += 80.0;
            }

            // Row 2: Status badges
            let badge_y = dot_y + 56.0;
            let badge_run = cx.text.layout(
                "Status Badges:",
                Point::new(inner.origin.x, badge_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(badge_run);

            let mut badge_x = inner.origin.x;
            for status in &statuses {
                let mut badge = RelayStatusBadge::new(*status);
                badge.paint(Bounds::new(badge_x, badge_y + 20.0, 90.0, 22.0), cx);
                badge_x += 100.0;
            }
        });
        y += status_height + SECTION_GAP;

        // ========== Panel 2: Event Kind Badges ==========
        let kinds_height = panel_height(280.0);
        let kinds_bounds = Bounds::new(bounds.origin.x, y, width, kinds_height);
        draw_panel("Event Kind Badges", kinds_bounds, cx, |inner, cx| {
            let kinds = [
                (EventKind::TextNote, "Social"),
                (EventKind::Metadata, "Identity"),
                (EventKind::Contacts, "Identity"),
                (EventKind::EncryptedDm, "Messaging"),
                (EventKind::Reaction, "Social"),
                (EventKind::ZapReceipt, "Payments"),
                (EventKind::RepoAnnounce, "Git"),
                (EventKind::Issue, "Git"),
                (EventKind::Patch, "Git"),
                (EventKind::PullRequest, "Git"),
                (EventKind::AgentProfile, "Agents"),
                (EventKind::TrajectorySession, "Agents"),
                (EventKind::DvmTextRequest, "DVM"),
                (EventKind::DvmTextResult, "DVM"),
                (EventKind::LongFormContent, "Content"),
                (EventKind::Custom(99999), "Custom"),
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (kind, category)) in kinds.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Category label
                let cat_run = cx.text.layout(
                    *category,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(cat_run);

                // Event kind badge
                let mut badge = EventKindBadge::new(kind.clone()).show_number(true);
                badge.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w - 4.0, 22.0), cx);
            }
        });
        y += kinds_height + SECTION_GAP;

        // ========== Panel 3: Bech32 Entities ==========
        let entities_height = panel_height(200.0);
        let entities_bounds = Bounds::new(bounds.origin.x, y, width, entities_height);
        draw_panel(
            "Bech32 Entities (NIP-19)",
            entities_bounds,
            cx,
            |inner, cx| {
                let entities = [
                    (
                        Bech32Type::Npub,
                        "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsutj2v5",
                    ),
                    (
                        Bech32Type::Note,
                        "note1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5nkr4f",
                    ),
                    (
                        Bech32Type::Nevent,
                        "nevent1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqnmupg",
                    ),
                    (
                        Bech32Type::Nprofile,
                        "nprofile1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsdp8el",
                    ),
                    (
                        Bech32Type::Nsec,
                        "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq9wlz3w",
                    ),
                    (Bech32Type::Nrelay, "nrelay1qqxrgvfex9j3n8qerc94kk"),
                ];

                let row_h = 40.0;
                let gap = 8.0;

                for (idx, (entity_type, value)) in entities.iter().enumerate() {
                    let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                    let mut entity = Bech32Entity::new(*entity_type, *value)
                        .show_prefix_badge(true)
                        .truncate(true);
                    entity.paint(
                        Bounds::new(
                            inner.origin.x,
                            row_y,
                            inner.size.width.min(400.0),
                            row_h - 4.0,
                        ),
                        cx,
                    );
                }
            },
        );
        y += entities_height + SECTION_GAP;

        // ========== Panel 4: Relay Connection List ==========
        let relays_height = panel_height(300.0);
        let relays_bounds = Bounds::new(bounds.origin.x, y, width, relays_height);
        draw_panel("Relay Connection List", relays_bounds, cx, |inner, cx| {
            let relays = [
                RelayInfo::new("wss://relay.damus.io")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(15420, 342)
                    .latency(45),
                RelayInfo::new("wss://nos.lol")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(8934, 156)
                    .latency(78),
                RelayInfo::new("wss://relay.nostr.band")
                    .status(RelayStatus::Connecting)
                    .read(true)
                    .write(false)
                    .events(0, 0),
                RelayInfo::new("wss://purplepag.es")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(false)
                    .events(2341, 0)
                    .latency(120),
                RelayInfo::new("wss://relay.snort.social")
                    .status(RelayStatus::Disconnected)
                    .read(true)
                    .write(true)
                    .events(0, 0),
                RelayInfo::new("wss://offchain.pub")
                    .status(RelayStatus::Error)
                    .read(true)
                    .write(true)
                    .events(0, 0),
            ];

            let row_h = 44.0;
            let gap = 4.0;

            for (idx, relay) in relays.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                let mut row = RelayRow::new(relay.clone());
                row.paint(
                    Bounds::new(inner.origin.x, row_y, inner.size.width.min(500.0), row_h),
                    cx,
                );
            }
        });
        y += relays_height + SECTION_GAP;

        // ========== Panel 5: Complete Relay Dashboard ==========
        let dashboard_height = panel_height(320.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Complete Relay Dashboard",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Dashboard header
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let header_run = cx.text.layout(
                    "Nostr Relay Pool",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(header_run);

                // Stats summary
                let stats_run = cx.text.layout(
                    "4 Connected | 1 Connecting | 1 Error",
                    Point::new(
                        inner.origin.x + inner.size.width - 220.0,
                        inner.origin.y + 14.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(stats_run);

                // Split into two columns
                let col_gap = 24.0;
                let col_w = (inner.size.width - col_gap) / 2.0;
                let content_y = inner.origin.y + 52.0;

                // Left column: Active relays
                let left_x = inner.origin.x;
                let active_label = cx.text.layout(
                    "Active Relays",
                    Point::new(left_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(active_label);

                let active_relays = [
                    RelayInfo::new("wss://relay.damus.io")
                        .status(RelayStatus::Connected)
                        .events(15420, 342)
                        .latency(45),
                    RelayInfo::new("wss://nos.lol")
                        .status(RelayStatus::Connected)
                        .events(8934, 156)
                        .latency(78),
                ];

                let row_h = 36.0;
                let row_gap = 4.0;
                let relay_y = content_y + 24.0;

                for (idx, relay) in active_relays.iter().enumerate() {
                    let row_y = relay_y + idx as f32 * (row_h + row_gap);
                    let mut row = RelayRow::new(relay.clone()).compact(true);
                    row.paint(Bounds::new(left_x, row_y, col_w.min(320.0), row_h), cx);
                }

                // Right column: Event statistics
                let right_x = inner.origin.x + col_w + col_gap;
                let stats_label = cx.text.layout(
                    "Event Statistics",
                    Point::new(right_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(stats_label);

                // Event kind summary
                let event_summary = [
                    ("Notes (kind:1)", "12,453"),
                    ("Reactions (kind:7)", "8,921"),
                    ("Profiles (kind:0)", "1,234"),
                    ("Zaps (kind:9735)", "456"),
                    ("DMs (kind:4)", "89"),
                ];

                let stat_y = content_y + 24.0;
                for (idx, (label, count)) in event_summary.iter().enumerate() {
                    let y_pos = stat_y + idx as f32 * 28.0;

                    let label_run = cx.text.layout(
                        *label,
                        Point::new(right_x, y_pos),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    let count_run = cx.text.layout(
                        *count,
                        Point::new(right_x + 140.0, y_pos),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(count_run);
                }
            },
        );
    }

    pub(crate) fn paint_gitafter(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Issue Status Badges ==========
        let issue_height = panel_height(160.0);
        let issue_bounds = Bounds::new(bounds.origin.x, y, width, issue_height);
        draw_panel("Issue Status Badges", issue_bounds, cx, |inner, cx| {
            let statuses = [
                IssueStatus::Open,
                IssueStatus::Claimed,
                IssueStatus::InProgress,
                IssueStatus::Closed,
                IssueStatus::Draft,
            ];

            let tile_w = 100.0;
            let gap = 12.0;

            for (idx, status) in statuses.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = IssueStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 80.0, 22.0), cx);

                // Compact version
                let mut compact = IssueStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 48.0, 24.0, 22.0), cx);
            }
        });
        y += issue_height + SECTION_GAP;

        // ========== Panel 2: PR Status Badges ==========
        let pr_height = panel_height(180.0);
        let pr_bounds = Bounds::new(bounds.origin.x, y, width, pr_height);
        draw_panel("PR Status Badges", pr_bounds, cx, |inner, cx| {
            let statuses = [
                PrStatus::Draft,
                PrStatus::Open,
                PrStatus::NeedsReview,
                PrStatus::Approved,
                PrStatus::ChangesRequested,
                PrStatus::Merged,
                PrStatus::Closed,
            ];

            let tile_w = 80.0;
            let tile_h = 60.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = PrStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 70.0, 22.0), cx);
            }
        });
        y += pr_height + SECTION_GAP;

        // ========== Panel 3: Bounty Badges ==========
        let bounty_height = panel_height(140.0);
        let bounty_bounds = Bounds::new(bounds.origin.x, y, width, bounty_height);
        draw_panel("Bounty Badges", bounty_bounds, cx, |inner, cx| {
            let bounties = [
                (500, BountyStatus::Active),
                (5000, BountyStatus::Active),
                (50000, BountyStatus::Claimed),
                (100000, BountyStatus::Paid),
                (25000, BountyStatus::Expired),
            ];

            let tile_w = 110.0;
            let gap = 12.0;

            for (idx, (amount, status)) in bounties.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Status label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Bounty badge
                let mut badge = BountyBadge::new(*amount).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 24.0), cx);
            }
        });
        y += bounty_height + SECTION_GAP;

        // ========== Panel 4: Stack Layer Indicators ==========
        let stack_height = panel_height(160.0);
        let stack_bounds = Bounds::new(bounds.origin.x, y, width, stack_height);
        draw_panel("Stack Layer Indicators", stack_bounds, cx, |inner, cx| {
            let stacks = [
                (1, 4, StackLayerStatus::Merged),
                (2, 4, StackLayerStatus::Ready),
                (3, 4, StackLayerStatus::Pending),
                (4, 4, StackLayerStatus::Blocked),
            ];

            let tile_w = 120.0;
            let gap = 12.0;

            for (idx, (layer, total, status)) in stacks.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Full badge
                let mut badge = StackLayerBadge::new(*layer, *total).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 80.0, 24.0), cx);

                // Compact badge
                let mut compact = StackLayerBadge::new(*layer, *total)
                    .status(*status)
                    .compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 32.0, 36.0, 22.0), cx);
            }
        });
        y += stack_height + SECTION_GAP;

        // ========== Panel 5: Agent Status Badges ==========
        let agent_height = panel_height(180.0);
        let agent_bounds = Bounds::new(bounds.origin.x, y, width, agent_height);
        draw_panel(
            "Agent Status & Type Badges",
            agent_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    AgentStatus::Online,
                    AgentStatus::Busy,
                    AgentStatus::Idle,
                    AgentStatus::Offline,
                    AgentStatus::Error,
                ];

                // Row 1: Agent statuses
                let mut x = inner.origin.x;
                for status in &statuses {
                    let mut badge = AgentStatusBadge::new(*status).show_dot(true);
                    badge.paint(Bounds::new(x, inner.origin.y, 80.0, 24.0), cx);
                    x += 90.0;
                }

                // Row 2: Agent types
                let types = [AgentType::Human, AgentType::Sovereign, AgentType::Custodial];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 40.0;
                for agent_type in &types {
                    let mut badge =
                        AgentStatusBadge::new(AgentStatus::Online).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);
                    x += 110.0;
                }

                // Row 3: Combined status + type
                let combined = [
                    (AgentType::Sovereign, AgentStatus::Busy, "Working on issue"),
                    (AgentType::Human, AgentStatus::Online, "Reviewing PRs"),
                    (AgentType::Sovereign, AgentStatus::Idle, "Waiting for work"),
                ];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 80.0;
                for (agent_type, status, desc) in &combined {
                    // Badge
                    let mut badge = AgentStatusBadge::new(*status).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);

                    // Description
                    let desc_run = cx.text.layout(
                        *desc,
                        Point::new(x, row_y + 28.0),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(desc_run);
                    x += 140.0;
                }
            },
        );
        y += agent_height + SECTION_GAP;

        // ========== Panel 6: Trajectory Status Badges ==========
        let traj_height = panel_height(160.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Status Badges", traj_bounds, cx, |inner, cx| {
            let statuses = [
                TrajectoryStatus::Verified,
                TrajectoryStatus::Partial,
                TrajectoryStatus::HasGaps,
                TrajectoryStatus::Suspicious,
                TrajectoryStatus::Mismatch,
                TrajectoryStatus::Unknown,
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Full badge
                let mut badge = TrajectoryStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 95.0, 22.0), cx);

                // Compact
                let mut compact = TrajectoryStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x + tile_w - 28.0, tile_y, 24.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 7: Complete GitAfter Dashboard ==========
        let dashboard_height = panel_height(360.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "GitAfter Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let title_run = cx.text.layout(
                    "openagents/openagents",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                // Issue row example
                let issue_y = inner.origin.y + 52.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, issue_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Issue status
                let mut issue_status = IssueStatusBadge::new(IssueStatus::Open);
                issue_status.paint(
                    Bounds::new(inner.origin.x + 8.0, issue_y + 17.0, 60.0, 22.0),
                    cx,
                );

                // Issue title
                let issue_title = cx.text.layout(
                    "#42: Add NIP-SA trajectory publishing",
                    Point::new(inner.origin.x + 76.0, issue_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(issue_title);

                // Bounty
                let mut bounty = BountyBadge::new(50000).status(BountyStatus::Active);
                bounty.paint(
                    Bounds::new(inner.origin.x + 76.0, issue_y + 28.0, 90.0, 22.0),
                    cx,
                );

                // Agent claimant
                let claimed_run = cx.text.layout(
                    "Claimed by npub1agent...",
                    Point::new(inner.origin.x + 180.0, issue_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(claimed_run);

                // PR row example
                let pr_y = issue_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, pr_y, inner.size.width, 72.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // PR status
                let mut pr_status = PrStatusBadge::new(PrStatus::Open);
                pr_status.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 8.0, 60.0, 22.0),
                    cx,
                );

                // Stack layer
                let mut stack_layer = StackLayerBadge::new(2, 4).status(StackLayerStatus::Ready);
                stack_layer.paint(
                    Bounds::new(inner.origin.x + 76.0, pr_y + 8.0, 80.0, 24.0),
                    cx,
                );

                // PR title
                let pr_title = cx.text.layout(
                    "Layer 2: Wire trajectory events to relay pool",
                    Point::new(inner.origin.x + 164.0, pr_y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(pr_title);

                // Agent author + trajectory
                let mut agent =
                    AgentStatusBadge::new(AgentStatus::Busy).agent_type(AgentType::Sovereign);
                agent.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 38.0, 100.0, 24.0),
                    cx,
                );

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(
                    Bounds::new(inner.origin.x + 116.0, pr_y + 40.0, 80.0, 22.0),
                    cx,
                );

                // "depends on layer 1" indicator
                let depends_run = cx.text.layout(
                    "Depends on: Layer 1 (merged)",
                    Point::new(inner.origin.x + 210.0, pr_y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(depends_run);
            },
        );
    }

    pub(crate) fn paint_sovereign_agents(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Threshold Key Badges ==========
        let threshold_height = panel_height(160.0);
        let threshold_bounds = Bounds::new(bounds.origin.x, y, width, threshold_height);
        draw_panel("Threshold Key Badges", threshold_bounds, cx, |inner, cx| {
            let configs = [
                (2, 3, 2, "2-of-3 (ready)"),
                (2, 3, 1, "2-of-3 (partial)"),
                (3, 5, 3, "3-of-5 (ready)"),
                (3, 5, 2, "3-of-5 (partial)"),
                (2, 3, 0, "2-of-3 (unknown)"),
            ];

            let tile_w = 130.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (threshold, total, available, label)) in configs.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Full badge
                let mut badge =
                    ThresholdKeyBadge::new(*threshold, *total).shares_available(*available);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 24.0), cx);
            }
        });
        y += threshold_height + SECTION_GAP;

        // ========== Panel 2: Agent Schedule Badges ==========
        let schedule_height = panel_height(180.0);
        let schedule_bounds = Bounds::new(bounds.origin.x, y, width, schedule_height);
        draw_panel("Agent Schedule Badges", schedule_bounds, cx, |inner, cx| {
            let schedules = [
                (
                    900,
                    vec![TriggerType::Mention, TriggerType::DirectMessage],
                    "15m + mentions/DMs",
                ),
                (
                    3600,
                    vec![TriggerType::Zap, TriggerType::Issue],
                    "1h + zaps/issues",
                ),
                (7200, vec![TriggerType::PullRequest], "2h + PRs"),
                (300, vec![], "5m heartbeat only"),
            ];

            let tile_w = 160.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (seconds, triggers, label)) in schedules.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Full badge
                let mut badge = AgentScheduleBadge::new(*seconds).triggers(triggers.clone());
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 24.0), cx);
            }
        });
        y += schedule_height + SECTION_GAP;

        // ========== Panel 3: Goal Progress Badges ==========
        let goal_height = panel_height(160.0);
        let goal_bounds = Bounds::new(bounds.origin.x, y, width, goal_height);
        draw_panel("Goal Progress Badges", goal_bounds, cx, |inner, cx| {
            let goals = [
                (
                    0.0,
                    GoalStatus::NotStarted,
                    GoalPriority::Medium,
                    "Not started",
                ),
                (
                    0.35,
                    GoalStatus::InProgress,
                    GoalPriority::High,
                    "In progress",
                ),
                (
                    0.65,
                    GoalStatus::InProgress,
                    GoalPriority::Critical,
                    "Critical",
                ),
                (
                    1.0,
                    GoalStatus::Completed,
                    GoalPriority::Medium,
                    "Completed",
                ),
                (0.5, GoalStatus::Blocked, GoalPriority::High, "Blocked"),
                (0.8, GoalStatus::Failed, GoalPriority::Critical, "Failed"),
            ];

            let tile_w = 140.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (progress, status, priority, label)) in goals.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = GoalProgressBadge::new(*progress)
                    .status(*status)
                    .priority(*priority);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 125.0, 22.0), cx);
            }
        });
        y += goal_height + SECTION_GAP;

        // ========== Panel 4: Tick Event Badges ==========
        let tick_height = panel_height(180.0);
        let tick_bounds = Bounds::new(bounds.origin.x, y, width, tick_height);
        draw_panel("Tick Event Badges", tick_bounds, cx, |inner, cx| {
            // Row 1: Tick outcomes
            let outcomes = [
                TickOutcome::Pending,
                TickOutcome::Success,
                TickOutcome::Failure,
                TickOutcome::Timeout,
                TickOutcome::Skipped,
            ];

            let mut x = inner.origin.x;
            for outcome in &outcomes {
                let mut badge = TickEventBadge::result(*outcome).duration_ms(1500);
                badge.paint(Bounds::new(x, inner.origin.y, 110.0, 22.0), cx);
                x += 120.0;
            }

            // Row 2: Request vs Result
            let row_y = inner.origin.y + 40.0;
            let req_label = cx.text.layout(
                "Request",
                Point::new(inner.origin.x, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(req_label);

            let mut request = TickEventBadge::request();
            request.paint(Bounds::new(inner.origin.x, row_y + 18.0, 80.0, 22.0), cx);

            let res_label = cx.text.layout(
                "Result (success, 2.3s)",
                Point::new(inner.origin.x + 120.0, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(res_label);

            let mut result = TickEventBadge::result(TickOutcome::Success).duration_ms(2300);
            result.paint(
                Bounds::new(inner.origin.x + 120.0, row_y + 18.0, 130.0, 22.0),
                cx,
            );

            // Compact versions
            let compact_y = row_y + 50.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, compact_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for outcome in &outcomes {
                let mut compact = TickEventBadge::result(*outcome).compact(true);
                compact.paint(Bounds::new(x, compact_y, 28.0, 22.0), cx);
                x += 36.0;
            }
        });
        y += tick_height + SECTION_GAP;

        // ========== Panel 5: Skill License Badges ==========
        let skill_height = panel_height(180.0);
        let skill_bounds = Bounds::new(bounds.origin.x, y, width, skill_height);
        draw_panel("Skill License Badges", skill_bounds, cx, |inner, cx| {
            let skills = [
                (SkillType::Code, LicenseStatus::Active, Some("git-rebase")),
                (SkillType::Data, LicenseStatus::Active, Some("market-data")),
                (SkillType::Model, LicenseStatus::Pending, Some("sonnet-4.5")),
                (SkillType::Tool, LicenseStatus::Expired, Some("browser-use")),
                (SkillType::Code, LicenseStatus::Revoked, None),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (skill_type, status, name)) in skills.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Type label
                let type_label = match skill_type {
                    SkillType::Code => "Code Skill",
                    SkillType::Data => "Data Skill",
                    SkillType::Model => "Model Skill",
                    SkillType::Tool => "Tool Skill",
                };
                let label_run = cx.text.layout(
                    type_label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = SkillLicenseBadge::new(*skill_type, *status);
                if let Some(n) = name {
                    badge = badge.name(*n);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += skill_height + SECTION_GAP;

        // ========== Panel 6: Complete Agent Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Sovereign Agent Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar with agent identity
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Agent icon + name
                let agent_icon = cx.text.layout(
                    "🤖",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 14.0),
                    theme::font_size::LG,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_icon);

                let agent_name = cx.text.layout(
                    "code-monkey-42",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_name);

                let npub = cx.text.layout(
                    "npub1agent42xyz...",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 28.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(npub);

                // Status badges on right side of header
                let mut status =
                    AgentStatusBadge::new(AgentStatus::Online).agent_type(AgentType::Sovereign);
                status.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 200.0,
                        inner.origin.y + 13.0,
                        100.0,
                        24.0,
                    ),
                    cx,
                );

                let mut threshold = ThresholdKeyBadge::new(2, 3).shares_available(2);
                threshold.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 13.0,
                        80.0,
                        24.0,
                    ),
                    cx,
                );

                // Schedule row
                let sched_y = inner.origin.y + 60.0;
                let sched_label = cx.text.layout(
                    "Schedule:",
                    Point::new(inner.origin.x + 8.0, sched_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(sched_label);

                let mut schedule = AgentScheduleBadge::new(900).triggers(vec![
                    TriggerType::Mention,
                    TriggerType::Zap,
                    TriggerType::Issue,
                ]);
                schedule.paint(Bounds::new(inner.origin.x + 70.0, sched_y, 140.0, 24.0), cx);

                // Goals section
                let goals_y = sched_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, goals_y, inner.size.width, 90.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let goals_title = cx.text.layout(
                    "Current Goals",
                    Point::new(inner.origin.x + 8.0, goals_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goals_title);

                // Goal 1
                let mut goal1 = GoalProgressBadge::new(0.75)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::High);
                goal1.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 28.0, 125.0, 22.0),
                    cx,
                );
                let goal1_desc = cx.text.layout(
                    "Fix d-006 Phase 4 issues",
                    Point::new(inner.origin.x + 142.0, goals_y + 32.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal1_desc);

                // Goal 2
                let mut goal2 = GoalProgressBadge::new(0.3)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::Medium);
                goal2.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 56.0, 125.0, 22.0),
                    cx,
                );
                let goal2_desc = cx.text.layout(
                    "Publish trajectory events",
                    Point::new(inner.origin.x + 142.0, goals_y + 60.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal2_desc);

                // Skills section
                let skills_y = goals_y + 100.0;
                let skills_label = cx.text.layout(
                    "Licensed Skills:",
                    Point::new(inner.origin.x + 8.0, skills_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(skills_label);

                let mut skill1 =
                    SkillLicenseBadge::new(SkillType::Code, LicenseStatus::Active).name("git-ops");
                skill1.paint(
                    Bounds::new(inner.origin.x + 100.0, skills_y, 120.0, 22.0),
                    cx,
                );

                let mut skill2 = SkillLicenseBadge::new(SkillType::Model, LicenseStatus::Active)
                    .name("opus-4.5");
                skill2.paint(
                    Bounds::new(inner.origin.x + 230.0, skills_y, 130.0, 22.0),
                    cx,
                );

                // Recent ticks section
                let ticks_y = skills_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        ticks_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let ticks_title = cx.text.layout(
                    "Recent Ticks",
                    Point::new(inner.origin.x + 8.0, ticks_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(ticks_title);

                // Tick timeline
                let tick_row_y = ticks_y + 30.0;
                let times = ["2m ago", "17m ago", "32m ago", "47m ago"];
                let outcomes = [
                    TickOutcome::Success,
                    TickOutcome::Success,
                    TickOutcome::Failure,
                    TickOutcome::Success,
                ];
                let durations = [1200, 890, 0, 2300];

                for (i, ((time, outcome), dur)) in times
                    .iter()
                    .zip(outcomes.iter())
                    .zip(durations.iter())
                    .enumerate()
                {
                    let tick_x = inner.origin.x + 8.0 + i as f32 * 100.0;

                    let time_run = cx.text.layout(
                        *time,
                        Point::new(tick_x, tick_row_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(time_run);

                    let mut tick = if *dur > 0 {
                        TickEventBadge::result(*outcome).duration_ms(*dur as u64)
                    } else {
                        TickEventBadge::result(*outcome)
                    };
                    tick.paint(Bounds::new(tick_x, tick_row_y + 16.0, 90.0, 22.0), cx);
                }

                // Trajectory hash
                let traj_y = ticks_y + 72.0;
                let traj_label = cx.text.layout(
                    "Current trajectory:",
                    Point::new(inner.origin.x + 8.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(traj_label);

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(Bounds::new(inner.origin.x + 120.0, traj_y, 80.0, 22.0), cx);

                let hash = cx.text.layout(
                    "hash: 7c6267e85a...",
                    Point::new(inner.origin.x + 210.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(hash);
            },
        );
    }

    pub(crate) fn paint_marketplace(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Market Type Badges ==========
        let market_height = panel_height(140.0);
        let market_bounds = Bounds::new(bounds.origin.x, y, width, market_height);
        draw_panel("Market Type Badges", market_bounds, cx, |inner, cx| {
            let types = [
                MarketType::Compute,
                MarketType::Skills,
                MarketType::Data,
                MarketType::Trajectories,
            ];

            let mut x = inner.origin.x;
            for market_type in &types {
                // Full badge
                let mut badge = MarketTypeBadge::new(*market_type);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);

                // Compact badge
                let mut compact = MarketTypeBadge::new(*market_type).compact(true);
                compact.paint(Bounds::new(x, inner.origin.y + 30.0, 28.0, 22.0), cx);

                x += 100.0;
            }
        });
        y += market_height + SECTION_GAP;

        // ========== Panel 2: Job Status Badges ==========
        let job_height = panel_height(180.0);
        let job_bounds = Bounds::new(bounds.origin.x, y, width, job_height);
        draw_panel(
            "Job Status Badges (NIP-90 DVM)",
            job_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    (JobStatus::Pending, None, "Pending"),
                    (JobStatus::Processing, None, "Processing"),
                    (JobStatus::Streaming, None, "Streaming"),
                    (JobStatus::Completed, Some(150), "Completed"),
                    (JobStatus::Failed, None, "Failed"),
                    (JobStatus::Cancelled, None, "Cancelled"),
                ];

                let tile_w = 110.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (status, cost, label)) in statuses.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Badge
                    let mut badge = JobStatusBadge::new(*status);
                    if let Some(sats) = cost {
                        badge = badge.cost_sats(*sats);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 22.0), cx);
                }
            },
        );
        y += job_height + SECTION_GAP;

        // ========== Panel 3: Reputation Badges ==========
        let rep_height = panel_height(160.0);
        let rep_bounds = Bounds::new(bounds.origin.x, y, width, rep_height);
        draw_panel(
            "Reputation & Trust Tier Badges",
            rep_bounds,
            cx,
            |inner, cx| {
                let tiers = [
                    (TrustTier::New, None, "New provider"),
                    (TrustTier::Established, Some(0.85), "Established"),
                    (TrustTier::Trusted, Some(0.95), "Trusted"),
                    (TrustTier::Expert, Some(0.99), "Expert"),
                ];

                let tile_w = 130.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (tier, rate, label)) in tiers.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Badge
                    let mut badge = ReputationBadge::new(*tier);
                    if let Some(r) = rate {
                        badge = badge.success_rate(*r);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 22.0), cx);
                }
            },
        );
        y += rep_height + SECTION_GAP;

        // ========== Panel 4: Trajectory Source Badges ==========
        let traj_height = panel_height(180.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Source Badges", traj_bounds, cx, |inner, cx| {
            let sources = [
                (
                    TrajectorySource::Codex,
                    Some(ContributionStatus::Accepted),
                    Some(42),
                ),
                (
                    TrajectorySource::Cursor,
                    Some(ContributionStatus::Pending),
                    Some(15),
                ),
                (
                    TrajectorySource::Windsurf,
                    Some(ContributionStatus::Redacted),
                    Some(23),
                ),
                (TrajectorySource::Custom, None, None),
            ];

            let tile_w = 180.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (source, status, count)) in sources.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Source label
                let label_run = cx.text.layout(
                    source.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = TrajectorySourceBadge::new(*source);
                if let Some(s) = status {
                    badge = badge.status(*s);
                }
                if let Some(c) = count {
                    badge = badge.session_count(*c);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 170.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 5: Earnings Badges ==========
        let earn_height = panel_height(180.0);
        let earn_bounds = Bounds::new(bounds.origin.x, y, width, earn_height);
        draw_panel("Earnings Badges", earn_bounds, cx, |inner, cx| {
            let earnings = [
                (EarningsType::Total, 1_250_000),
                (EarningsType::Compute, 500_000),
                (EarningsType::Skills, 350_000),
                (EarningsType::Data, 250_000),
                (EarningsType::Trajectories, 150_000),
            ];

            // Row 1: Full earnings badges
            let mut x = inner.origin.x;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount);
                badge.paint(Bounds::new(x, inner.origin.y, 160.0, 22.0), cx);
                x += 170.0;
                if x > inner.origin.x + inner.size.width - 100.0 {
                    break;
                }
            }

            // Row 2: Compact versions
            let row_y = inner.origin.y + 40.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, row_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount).compact(true);
                badge.paint(Bounds::new(x, row_y, 70.0, 22.0), cx);
                x += 80.0;
            }
        });
        y += earn_height + SECTION_GAP;

        // ========== Panel 6: Complete Marketplace Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Marketplace Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Title
                let title = cx.text.layout(
                    "Unified Marketplace",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Market type tabs
                let mut x = inner.origin.x + 12.0;
                let tab_y = inner.origin.y + 28.0;
                for market_type in &[
                    MarketType::Compute,
                    MarketType::Skills,
                    MarketType::Data,
                    MarketType::Trajectories,
                ] {
                    let mut badge = MarketTypeBadge::new(*market_type);
                    badge.paint(Bounds::new(x, tab_y, 80.0, 20.0), cx);
                    x += 90.0;
                }

                // Earnings summary on right
                let mut total_earn =
                    EarningsBadge::new(EarningsType::Total, 1_250_000).compact(true);
                total_earn.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 14.0,
                        80.0,
                        22.0,
                    ),
                    cx,
                );

                // Provider row
                let prov_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, prov_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Provider name + reputation
                let prov_name = cx.text.layout(
                    "compute-provider-1",
                    Point::new(inner.origin.x + 8.0, prov_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(prov_name);

                let mut rep = ReputationBadge::new(TrustTier::Trusted).success_rate(0.97);
                rep.paint(
                    Bounds::new(inner.origin.x + 140.0, prov_y + 6.0, 100.0, 22.0),
                    cx,
                );

                // Job in progress
                let mut job = JobStatusBadge::new(JobStatus::Processing);
                job.paint(
                    Bounds::new(inner.origin.x + 8.0, prov_y + 32.0, 90.0, 22.0),
                    cx,
                );

                let job_info = cx.text.layout(
                    "llama3 • 1.2K tokens",
                    Point::new(inner.origin.x + 106.0, prov_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(job_info);

                // Trajectory contribution section
                let traj_y = prov_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, traj_y, inner.size.width, 100.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let traj_title = cx.text.layout(
                    "Trajectory Contributions",
                    Point::new(inner.origin.x + 8.0, traj_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(traj_title);

                // Source badges row
                let source_y = traj_y + 28.0;
                let mut x = inner.origin.x + 8.0;
                let sources = [
                    (TrajectorySource::Codex, ContributionStatus::Accepted, 42),
                    (TrajectorySource::Cursor, ContributionStatus::Pending, 15),
                ];
                for (source, status, count) in &sources {
                    let mut badge = TrajectorySourceBadge::new(*source)
                        .status(*status)
                        .session_count(*count);
                    badge.paint(Bounds::new(x, source_y, 170.0, 22.0), cx);
                    x += 180.0;
                }

                // Earnings row
                let earn_y = traj_y + 56.0;
                let earn_label = cx.text.layout(
                    "Trajectory earnings:",
                    Point::new(inner.origin.x + 8.0, earn_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(earn_label);

                let mut traj_earn = EarningsBadge::new(EarningsType::Trajectories, 150_000);
                traj_earn.paint(Bounds::new(inner.origin.x + 120.0, earn_y, 160.0, 22.0), cx);

                // Total earnings bar at bottom
                let total_y = traj_y + 80.0;
                let total_label = cx.text.layout(
                    "Total today:",
                    Point::new(inner.origin.x + 8.0, total_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(total_label);

                let mut today_earn = EarningsBadge::new(EarningsType::Total, 25_000);
                today_earn.paint(Bounds::new(inner.origin.x + 80.0, total_y, 150.0, 22.0), cx);
            },
        );
    }

    pub(crate) fn paint_autopilot(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Session Status Badges ==========
        let session_height = panel_height(180.0);
        let session_bounds = Bounds::new(bounds.origin.x, y, width, session_height);
        draw_panel("Session Status Badges", session_bounds, cx, |inner, cx| {
            let statuses = [
                (SessionStatus::Pending, None, None, "Pending"),
                (SessionStatus::Running, Some(125), Some(8), "Running"),
                (SessionStatus::Paused, Some(340), Some(12), "Paused"),
                (SessionStatus::Completed, Some(1800), Some(45), "Completed"),
                (SessionStatus::Failed, Some(65), Some(3), "Failed"),
                (SessionStatus::Aborted, Some(200), Some(5), "Aborted"),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, duration, tasks, label)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = SessionStatusBadge::new(*status);
                if let Some(secs) = duration {
                    badge = badge.duration(*secs);
                }
                if let Some(count) = tasks {
                    badge = badge.task_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += session_height + SECTION_GAP;

        // ========== Panel 2: APM Gauges ==========
        let apm_height = panel_height(160.0);
        let apm_bounds = Bounds::new(bounds.origin.x, y, width, apm_height);
        draw_panel(
            "APM (Actions Per Minute) Gauges",
            apm_bounds,
            cx,
            |inner, cx| {
                let apms = [
                    (0.0, "Idle"),
                    (5.0, "Low"),
                    (22.0, "Normal"),
                    (45.0, "High"),
                    (80.0, "Intense"),
                ];

                let tile_w = 160.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (apm, label)) in apms.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Gauge
                    let mut gauge = ApmGauge::new(*apm);
                    gauge.paint(Bounds::new(tile_x, tile_y + 18.0, 150.0, 22.0), cx);
                }
            },
        );
        y += apm_height + SECTION_GAP;

        // ========== Panel 3: Resource Usage Bars ==========
        let resource_height = panel_height(180.0);
        let resource_bounds = Bounds::new(bounds.origin.x, y, width, resource_height);
        draw_panel("Resource Usage Bars", resource_bounds, cx, |inner, cx| {
            let resources = [
                (ResourceType::Memory, 35.0, "Normal Memory (35%)"),
                (ResourceType::Memory, 65.0, "Warning Memory (65%)"),
                (ResourceType::Memory, 92.0, "Critical Memory (92%)"),
                (ResourceType::Cpu, 28.0, "Normal CPU (28%)"),
                (ResourceType::Cpu, 75.0, "Warning CPU (75%)"),
                (ResourceType::Cpu, 95.0, "Critical CPU (95%)"),
            ];

            let tile_w = 200.0;
            let tile_h = 50.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (rtype, pct, label)) in resources.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Bar
                let mut bar = ResourceUsageBar::new(*rtype, *pct);
                bar.paint(Bounds::new(tile_x, tile_y + 18.0, 180.0, 22.0), cx);
            }
        });
        y += resource_height + SECTION_GAP;

        // ========== Panel 4: Daemon Status Badges ==========
        let daemon_height = panel_height(160.0);
        let daemon_bounds = Bounds::new(bounds.origin.x, y, width, daemon_height);
        draw_panel("Daemon Status Badges", daemon_bounds, cx, |inner, cx| {
            let statuses = [
                (DaemonStatus::Offline, None, None, "Offline"),
                (DaemonStatus::Starting, None, None, "Starting"),
                (
                    DaemonStatus::Online,
                    Some(86400),
                    Some(3),
                    "Online (1d, 3 workers)",
                ),
                (DaemonStatus::Restarting, None, None, "Restarting"),
                (DaemonStatus::Error, None, None, "Error"),
                (DaemonStatus::Stopping, None, None, "Stopping"),
            ];

            let tile_w = 170.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, uptime, workers, label)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = DaemonStatusBadge::new(*status);
                if let Some(secs) = uptime {
                    badge = badge.uptime(*secs);
                }
                if let Some(count) = workers {
                    badge = badge.worker_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 160.0, 22.0), cx);
            }
        });
        y += daemon_height + SECTION_GAP;

        // ========== Panel 5: Parallel Agent Badges ==========
        let parallel_height = panel_height(180.0);
        let parallel_bounds = Bounds::new(bounds.origin.x, y, width, parallel_height);
        draw_panel("Parallel Agent Badges", parallel_bounds, cx, |inner, cx| {
            let agents = [
                (0, ParallelAgentStatus::Idle, None, "Agent 0: Idle"),
                (
                    1,
                    ParallelAgentStatus::Running,
                    Some("Building tests"),
                    "Agent 1: Running",
                ),
                (
                    2,
                    ParallelAgentStatus::Waiting,
                    Some("Awaiting input"),
                    "Agent 2: Waiting",
                ),
                (3, ParallelAgentStatus::Completed, None, "Agent 3: Done"),
                (
                    4,
                    ParallelAgentStatus::Failed,
                    Some("Build error"),
                    "Agent 4: Failed",
                ),
                (5, ParallelAgentStatus::Initializing, None, "Agent 5: Init"),
            ];

            let tile_w = 220.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (agent_idx, status, task, label)) in agents.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = ParallelAgentBadge::new(*agent_idx, *status);
                if let Some(t) = task {
                    badge = badge.current_task(*t);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 200.0, 22.0), cx);
            }
        });
        y += parallel_height + SECTION_GAP;

        // ========== Panel 6: Complete Autopilot Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Autopilot Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Title
                let title = cx.text.layout(
                    "Autopilot Control",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Daemon status on right
                let mut daemon = DaemonStatusBadge::new(DaemonStatus::Online)
                    .uptime(86400)
                    .worker_count(3);
                daemon.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 180.0,
                        inner.origin.y + 10.0,
                        170.0,
                        22.0,
                    ),
                    cx,
                );

                // APM gauge
                let mut apm = ApmGauge::new(28.5);
                apm.paint(
                    Bounds::new(inner.origin.x + 12.0, inner.origin.y + 32.0, 140.0, 22.0),
                    cx,
                );

                // Active session row
                let session_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        session_y,
                        inner.size.width,
                        56.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                // Session info
                let session_title = cx.text.layout(
                    "Active Session #1234",
                    Point::new(inner.origin.x + 8.0, session_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(session_title);

                let mut session = SessionStatusBadge::new(SessionStatus::Running)
                    .duration(325)
                    .task_count(12);
                session.paint(
                    Bounds::new(inner.origin.x + 160.0, session_y + 6.0, 200.0, 22.0),
                    cx,
                );

                // Task info
                let task_info = cx.text.layout(
                    "Current: Building component tests",
                    Point::new(inner.origin.x + 8.0, session_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(task_info);

                // Parallel agents section
                let agents_y = session_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        agents_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let agents_label = cx.text.layout(
                    "Parallel Agents",
                    Point::new(inner.origin.x + 8.0, agents_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agents_label);

                // Agent badges in a row
                let mut x = inner.origin.x + 8.0;
                for (idx, status) in [
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Waiting,
                ]
                .iter()
                .enumerate()
                {
                    let mut agent = ParallelAgentBadge::new(idx as u8, *status).compact(true);
                    agent.paint(Bounds::new(x, agents_y + 32.0, 50.0, 22.0), cx);
                    x += 60.0;
                }

                // Resource bars
                let res_y = agents_y + 60.0;
                let mut mem = ResourceUsageBar::new(ResourceType::Memory, 45.0).bar_width(80.0);
                mem.paint(Bounds::new(inner.origin.x + 8.0, res_y, 160.0, 22.0), cx);

                let mut cpu = ResourceUsageBar::new(ResourceType::Cpu, 62.0).bar_width(80.0);
                cpu.paint(Bounds::new(inner.origin.x + 180.0, res_y, 160.0, 22.0), cx);

                // Session history section
                let history_y = agents_y + 112.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        history_y,
                        inner.size.width,
                        80.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let history_label = cx.text.layout(
                    "Recent Sessions",
                    Point::new(inner.origin.x + 8.0, history_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(history_label);

                // Completed sessions
                let mut completed1 = SessionStatusBadge::new(SessionStatus::Completed)
                    .duration(1800)
                    .task_count(45)
                    .compact(true);
                completed1.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 32.0, 28.0, 22.0),
                    cx,
                );
                let c1_label = cx.text.layout(
                    "#1233 - 45 tasks",
                    Point::new(inner.origin.x + 42.0, history_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c1_label);

                let mut completed2 = SessionStatusBadge::new(SessionStatus::Failed).compact(true);
                completed2.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 56.0, 28.0, 22.0),
                    cx,
                );
                let c2_label = cx.text.layout(
                    "#1232 - Build error",
                    Point::new(inner.origin.x + 42.0, history_y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c2_label);
            },
        );
    }
}
