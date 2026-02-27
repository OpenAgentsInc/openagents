use super::*;

impl Storybook {
    pub(crate) fn paint_nostr_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let contacts_height = panel_height(320.0);
        let dm_height = panel_height(380.0);
        let zaps_height = panel_height(280.0);
        let relay_mgr_height = panel_height(420.0);
        let dm_thread_height = panel_height(450.0);
        let zap_flow_height = panel_height(420.0);
        let event_inspector_height = panel_height(400.0);
        let ref_height = panel_height(180.0);

        let panels = panel_stack(
            bounds,
            &[
                contacts_height,
                dm_height,
                zaps_height,
                relay_mgr_height,
                dm_thread_height,
                zap_flow_height,
                event_inspector_height,
                ref_height,
            ],
        );

        // ========== Panel 1: Contact Cards ==========
        let contacts_bounds = panels[0];
        draw_panel("Contact Management", contacts_bounds, cx, |inner, cx| {
            let contacts = [
                ContactInfo::new("npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq")
                    .display_name("Alice Developer")
                    .nip05("alice@openagents.com")
                    .about("Building the future of decentralized AI")
                    .verification(ContactVerification::Verified)
                    .following(true)
                    .mutual(true),
                ContactInfo::new("npub1zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
                    .display_name("Bob Builder")
                    .nip05("bob@nostr.dev")
                    .about("Open source contributor")
                    .verification(ContactVerification::WebOfTrust)
                    .following(true)
                    .mutual(false),
                ContactInfo::new("npub1yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy")
                    .display_name("Anonymous")
                    .verification(ContactVerification::Unknown)
                    .following(false)
                    .mutual(false),
            ];

            for (i, contact) in contacts.iter().enumerate() {
                let mut card = ContactCard::new(contact.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 95.0,
                        inner.size.width.min(500.0),
                        90.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: DM Conversations ==========
        let dm_bounds = panels[1];
        draw_panel("Direct Messages", dm_bounds, cx, |inner, cx| {
            let messages = [
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates. I'll review it tonight.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
                DmMessage::new(
                    "m4",
                    "[Encrypted message - decryption failed]",
                    DmDirection::Incoming,
                )
                .sender("Unknown")
                .timestamp("5 min ago")
                .encryption(EncryptionStatus::Failed)
                .read(false),
            ];

            for (i, msg) in messages.iter().enumerate() {
                let mut bubble = DmBubble::new(msg.clone());
                bubble.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 80.0,
                        inner.size.width.min(500.0),
                        75.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Zaps & Lightning ==========
        let zaps_bounds = panels[2];
        draw_panel("Zaps & Lightning", zaps_bounds, cx, |inner, cx| {
            let zaps = [
                ZapInfo::new("z1", 21000, "npub1alice...")
                    .sender_name("Alice")
                    .message("Great thread!")
                    .timestamp("5 min ago"),
                ZapInfo::new("z2", 1000000, "npub1bob...")
                    .sender_name("Bob")
                    .message("Thanks for the amazing tutorial!")
                    .timestamp("1 hour ago"),
                ZapInfo::new("z3", 500, "npub1anon...").timestamp("2 hours ago"),
            ];

            for (i, zap) in zaps.iter().enumerate() {
                let mut card = ZapCard::new(zap.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 85.0,
                        inner.size.width.min(450.0),
                        80.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Relay Manager Organism ==========
        let relay_mgr_bounds = panels[3];
        draw_panel(
            "Relay Manager (Organism)",
            relay_mgr_bounds,
            cx,
            |inner, cx| {
                let relays = vec![
                    RelayInfo::new("wss://relay.damus.io").status(RelayStatus::Connected),
                    RelayInfo::new("wss://nos.lol").status(RelayStatus::Connecting),
                    RelayInfo::new("wss://relay.nostr.band").status(RelayStatus::Connected),
                    RelayInfo::new("wss://relay.snort.social").status(RelayStatus::Disconnected),
                ];
                let mut manager = RelayManager::new(relays);
                manager.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 5: DM Thread Organism ==========
        let dm_thread_bounds = panels[4];
        draw_panel("DM Thread (Organism)", dm_thread_bounds, cx, |inner, cx| {
            let messages = vec![
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
            ];
            let mut thread =
                DmThread::new("Alice Developer", "npub1abc123xyz789").messages(messages);
            thread.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    400.0,
                ),
                cx,
            );
        });

        // ========== Panel 6: Zap Flow Organism ==========
        let zap_flow_bounds = panels[5];
        draw_panel(
            "Zap Flow Wizard (Organism)",
            zap_flow_bounds,
            cx,
            |inner, cx| {
                let mut flow = ZapFlow::new("Alice Developer", "npub1abc123xyz789...");
                flow.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(400.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 7: Event Inspector Organism ==========
        let event_inspector_bounds = panels[6];
        draw_panel(
            "Event Inspector (Organism)",
            event_inspector_bounds,
            cx,
            |inner, cx| {
                let event_data = EventData::new(
                    "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd",
                    "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                    1,
                )
                .content("GM! Building the future of decentralized AI. #OpenAgents #Nostr")
                .created_at(1700000000)
                .tags(vec![
                    TagData::new("t", vec!["OpenAgents".to_string()]),
                    TagData::new("t", vec!["Nostr".to_string()]),
                    TagData::new("p", vec!["npub1alice...".to_string()]),
                ])
                .sig("abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234")
                .verified(true);

                let mut inspector = EventInspector::new(event_data);
                inspector.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(450.0),
                        350.0,
                    ),
                    cx,
                );
            },
        );

        // ========== Panel 8: Status Reference ==========
        let nostr_ref_bounds = panels[7];
        draw_panel(
            "Nostr Status Reference",
            nostr_ref_bounds,
            cx,
            |inner, cx| {
                // Verification statuses
                let mut ver_x = inner.origin.x;
                let verifications = [
                    ContactVerification::Verified,
                    ContactVerification::WebOfTrust,
                    ContactVerification::Unknown,
                ];

                for ver in &verifications {
                    let ver_w = (ver.label().len() as f32 * 7.0) + 16.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(ver_x, inner.origin.y, ver_w, 20.0))
                            .with_background(ver.color().with_alpha(0.2))
                            .with_border(ver.color(), 1.0),
                    );
                    let text = cx.text.layout(
                        ver.label(),
                        Point::new(ver_x + 6.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        ver.color(),
                    );
                    cx.scene.draw_text(text);
                    ver_x += ver_w + 12.0;
                }

                // Encryption statuses
                let mut enc_x = inner.origin.x;
                let encryptions = [
                    EncryptionStatus::Encrypted,
                    EncryptionStatus::Decrypted,
                    EncryptionStatus::Failed,
                ];

                for enc in &encryptions {
                    let enc_w = 80.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(enc_x, inner.origin.y + 35.0, enc_w, 20.0))
                            .with_background(enc.color().with_alpha(0.2))
                            .with_border(enc.color(), 1.0),
                    );
                    let label = format!(
                        "{} {}",
                        enc.icon(),
                        match enc {
                            EncryptionStatus::Encrypted => "Encrypted",
                            EncryptionStatus::Decrypted => "Decrypted",
                            EncryptionStatus::Failed => "Failed",
                        }
                    );
                    let text = cx.text.layout(
                        &label,
                        Point::new(enc_x + 6.0, inner.origin.y + 39.0),
                        theme::font_size::XS,
                        enc.color(),
                    );
                    cx.scene.draw_text(text);
                    enc_x += enc_w + 12.0;
                }

                // DM directions
                let mut dir_x = inner.origin.x;
                let directions = [
                    ("Incoming", Hsla::new(200.0, 0.6, 0.5, 1.0)),
                    ("Outgoing", theme::accent::PRIMARY),
                ];

                for (label, color) in &directions {
                    let dir_w = 80.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(dir_x, inner.origin.y + 70.0, dir_w, 20.0))
                            .with_background(color.with_alpha(0.2))
                            .with_border(*color, 1.0),
                    );
                    let text = cx.text.layout(
                        label,
                        Point::new(dir_x + 6.0, inner.origin.y + 74.0),
                        theme::font_size::XS,
                        *color,
                    );
                    cx.scene.draw_text(text);
                    dir_x += dir_w + 12.0;
                }
            },
        );
    }
}
