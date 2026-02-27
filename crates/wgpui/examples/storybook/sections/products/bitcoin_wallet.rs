use super::*;

impl Storybook {
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
}
