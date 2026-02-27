use super::*;

impl Storybook {
    pub(crate) fn paint_wallet_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mnemonic_height = panel_height(260.0);
        let address_height = panel_height(180.0);
        let tx_height = panel_height(280.0);
        let send_height = panel_height(360.0);
        let receive_height = panel_height(420.0);

        let panels = panel_stack(
            bounds,
            &[
                mnemonic_height,
                address_height,
                tx_height,
                send_height,
                receive_height,
            ],
        );

        // ========== Panel 1: Mnemonic Display ==========
        let mnemonic_bounds = panels[0];
        draw_panel("Mnemonic Display", mnemonic_bounds, cx, |inner, cx| {
            // Sample 12-word mnemonic
            let words = vec![
                "abandon".to_string(),
                "ability".to_string(),
                "able".to_string(),
                "about".to_string(),
                "above".to_string(),
                "absent".to_string(),
                "absorb".to_string(),
                "abstract".to_string(),
                "absurd".to_string(),
                "abuse".to_string(),
                "access".to_string(),
                "accident".to_string(),
            ];

            let mut mnemonic = MnemonicDisplay::new(words).revealed(true);
            mnemonic.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    200.0,
                ),
                cx,
            );
        });

        // ========== Panel 2: Address Cards ==========
        let address_bounds = panels[1];
        draw_panel("Address Cards", address_bounds, cx, |inner, cx| {
            // Bitcoin address
            let mut btc_card = AddressCard::new(
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                AddressType::Bitcoin,
            )
            .label("Primary Wallet");
            btc_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );

            // Lightning address
            let mut ln_card =
                AddressCard::new("lnbc1500n1pj9nr6mpp5argz38...", AddressType::Lightning)
                    .label("Lightning Invoice");
            ln_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 80.0,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );
        });

        // ========== Panel 3: Transaction History ==========
        let tx_bounds = panels[2];
        draw_panel("Transaction History", tx_bounds, cx, |inner, cx| {
            let transactions = [
                TransactionInfo::new("tx-1", 150000, TransactionDirection::Incoming)
                    .timestamp("2 hours ago")
                    .description("Payment from Alice"),
                TransactionInfo::new("tx-2", 50000, TransactionDirection::Outgoing)
                    .timestamp("Yesterday")
                    .description("Coffee shop")
                    .fee(500),
                TransactionInfo::new("tx-3", 1000000, TransactionDirection::Incoming)
                    .timestamp("3 days ago")
                    .description("Freelance payment"),
                TransactionInfo::new("tx-4", 25000, TransactionDirection::Outgoing)
                    .timestamp("1 week ago")
                    .description("Subscription")
                    .fee(250),
            ];

            for (i, tx) in transactions.iter().enumerate() {
                let mut row = TransactionRow::new(tx.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Send Flow ==========
        let send_bounds = panels[3];
        draw_panel("Send Flow Wizard", send_bounds, cx, |inner, cx| {
            let mut send_flow = SendFlow::new()
                .step(SendStep::Review)
                .address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq")
                .amount(50000)
                .fee(500);
            send_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    320.0,
                ),
                cx,
            );
        });

        // ========== Panel 5: Receive Flow ==========
        let receive_bounds = panels[4];
        draw_panel("Receive Flow Wizard", receive_bounds, cx, |inner, cx| {
            let mut receive_flow = ReceiveFlow::new()
                .step(ReceiveStep::ShowInvoice)
                .receive_type(ReceiveType::Lightning)
                .amount(25000)
                .invoice("lnbc250u1pjxxx...")
                .expires_in(3600);
            receive_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    380.0,
                ),
                cx,
            );
        });
    }
}
