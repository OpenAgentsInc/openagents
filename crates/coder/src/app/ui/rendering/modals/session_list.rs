        ModalState::SessionList { selected } => {
            let sessions = &state.session.session_index;
            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let selected = (*selected).min(sessions.len().saturating_sub(1));
            let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                0.0
            } else {
                state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
            };
            let layout = session_list_layout(
                logical_width,
                logical_height,
                sessions.len(),
                selected,
                checkpoint_height,
            );
            let modal_bounds = layout.modal_bounds;
            let modal_x = modal_bounds.origin.x;
            let modal_y = modal_bounds.origin.y;
            let _modal_width = modal_bounds.size.width;
            let modal_height = modal_bounds.size.height;

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + SESSION_MODAL_PADDING;
            let title_run = state.text_system.layout_styled_mono(
                "Sessions",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let desc_run = state.text_system.layout_styled_mono(
                "Click a card to resume, or fork from a previous session.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);

            if sessions.is_empty() {
                y += 26.0;
                let empty_run = state.text_system.layout_styled_mono(
                    "No sessions recorded yet.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                for (index, bounds) in &layout.card_bounds {
                    if let Some(card) = state.session.session_cards.get_mut(*index) {
                        card.paint(*bounds, &mut paint_cx);
                    }
                    if *index == selected {
                        let outline =
                            Quad::new(*bounds).with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                        paint_cx.scene.draw_quad(outline);
                    }
                }
            }

            if let Some(bounds) = layout.checkpoint_bounds {
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                state.session.checkpoint_restore.paint(bounds, &mut paint_cx);
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to resume · Esc to exit · Fork with button",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        },
