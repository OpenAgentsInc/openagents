//! Shared redraw invalidation. Idle observation does not present another frame.
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use crate::Overlay;

pub const ACTIVE_POLL: Duration = Duration::from_millis(16);
pub const IDLE_POLL: Duration = Duration::from_millis(100);
const BLINK: Duration = Duration::from_millis(530);
const BLINK_LIFE: Duration = Duration::from_millis(3180);
const FLASH: Duration = Duration::from_millis(180);

#[derive(Default)]
pub(crate) struct Invalidation {
    presented: Option<u64>,
    queued: bool,
    forced: bool,
    retry_at: Option<Instant>,
}
impl Invalidation {
    fn request(&mut self, key: u64) -> bool {
        if !self.queued && (self.forced || self.presented != Some(key)) {
            self.queued = true;
            true
        } else {
            false
        }
    }
    fn presented(&mut self, key: u64) {
        self.presented = Some(key);
        self.queued = false;
        self.forced = false;
    }
}

/// A finite typing animation settles to a steady caret.
pub(crate) fn caret_visible(now: Instant, typed: Instant) -> bool {
    let elapsed = now.saturating_duration_since(typed);
    elapsed >= BLINK_LIFE || (elapsed.as_millis() / BLINK.as_millis()) % 2 == 0
}

impl Overlay {
    /// A mount changed its layout, backing scale, or exposed surface.
    pub fn invalidate(&mut self) {
        self.presentation.forced = true;
        self.presentation.queued = false;
        self.presentation.retry_at = None;
    }

    /// Defers a failed surface acquisition until a finite retry deadline.
    pub fn defer_presentation(&mut self, until: Instant) {
        self.presentation.forced = true;
        self.presentation.queued = false;
        self.presentation.retry_at = Some(until);
    }

    /// Whether the mount must wait before acquiring its surface again.
    pub fn presentation_deferred(&self, now: Instant) -> bool {
        self.presentation.retry_at.is_some_and(|until| now < until)
    }

    /// Whether changed content or a due animation needs one frame.
    pub fn redraw_needed(&mut self, now: Instant) -> bool {
        if self.presentation_deferred(now) {
            return false;
        }
        self.presentation.retry_at = None;
        let key = self.presentation_key(now);
        self.presentation.request(key)
    }

    /// Retains the content actually presented, including changes made while drawing.
    pub fn presented(&mut self, now: Instant) {
        self.presentation.retry_at = None;
        let key = self.presentation_key(now);
        self.presentation.presented(key);
    }

    /// Next animation or surface retry boundary; the sheet's caret never blinks.
    pub fn animation_deadline(&self, now: Instant) -> Option<Instant> {
        if let Some(until) = self.presentation.retry_at.filter(|until| *until > now) {
            return Some(until);
        }
        let flash = self
            .core
            .panes
            .values()
            .filter_map(|pane| pane.flash)
            .map(|at| at + FLASH)
            .filter(|at| *at > now)
            .min();
        let blink = if !self.core.paper.on
            && self.core.open
            && self.core.focused
            && now < self.core.typed + BLINK_LIFE
            && self
                .core
                .focus_id()
                .and_then(|id| self.core.panes.get(&id))
                .is_some_and(|pane| pane.session.vt.cursor_style().blink)
        {
            let elapsed = now.saturating_duration_since(self.core.typed).as_millis();
            Some(self.core.typed + BLINK * ((elapsed / BLINK.as_millis()) as u32 + 1))
        } else {
            None
        };
        flash.into_iter().chain(blink).min()
    }

    fn presentation_key(&self, now: Instant) -> u64 {
        let app = &self.core;
        let paper = &app.paper;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            app.open,
            app.focused,
            app.active,
            app.prefix,
            paper.on,
            paper.help,
            paper.cursor,
            paper.scroll,
            paper.revision(),
        )
            .hash(&mut h);
        paper.input.hash(&mut h);
        app.focus_id().hash(&mut h);
        for tab in &app.tabs {
            (tab.layout.focus(), tab.zoomed).hash(&mut h);
        }
        (paper.attach, &paper.warned, paper.grid, paper.queue.len()).hash(&mut h);
        (app.smart.workers.len(), app.smart.execution.is_some()).hash(&mut h);
        app.copy
            .as_ref()
            .map(|copy| (copy.pane, copy.cursor, copy.state()))
            .hash(&mut h);
        (app.products.focus, app.products.scroll).hash(&mut h);
        (app.sharing.open, app.sharing.pane, app.sharing.scroll).hash(&mut h);
        if app.sharing.open {
            app.sharing.lines().hash(&mut h);
        }
        if app.sharing.active() {
            app.sharing.marker().hash(&mut h);
            app.sharing.agent_badge().hash(&mut h);
        }
        app.button
            .map(|r| [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()])
            .hash(&mut h);
        app.button
            .is_some_and(|r| r.contains(app.pointer))
            .hash(&mut h);
        (
            app.tabs.len(),
            app.smart.selected,
            app.smart.proposal_scroll,
        )
            .hash(&mut h);
        app.smart.pending.hash(&mut h);
        if let Some((_, key)) = &app.smart.pending {
            if let Some(entry) = app.smart.book.entries.get(key) {
                matches!(entry.phase, terminal_core::proposals::Phase::Warned { .. }).hash(&mut h);
                if let Some(terminal_core::proposals::Effect::Destructive(warning)) =
                    app.smart.policy.0.get(&entry.proposal.command)
                {
                    warning.hash(&mut h);
                }
            }
        }
        if let Some(draft) = &app.smart.draft {
            draft.context.preview().hash(&mut h);
        }
        app.smart
            .correction
            .as_ref()
            .map(|c| (&c.choices.lines, c.next, &c.typed))
            .hash(&mut h);
        app.smart
            .draft
            .as_ref()
            .map(|draft| (&draft.text, draft.scroll))
            .hash(&mut h);
        app.notice.hash(&mut h);
        paper.git.hash(&mut h);
        paper.door.hash(&mut h);
        app.stats.shown.hash(&mut h);
        if app.stats.shown {
            app.stats.line().hash(&mut h);
        }
        app.find_prompt().hash(&mut h);
        app.paste_prompt().hash(&mut h);
        for (id, rect) in app.shown() {
            (
                id,
                rect.x.to_bits(),
                rect.y.to_bits(),
                rect.w.to_bits(),
                rect.h.to_bits(),
            )
                .hash(&mut h);
        }
        for (id, pane) in &app.panes {
            (
                id,
                pane.session.vt.generation(),
                pane.render_revision,
                pane.scroll,
                pane.ended,
            )
                .hash(&mut h);
            pane.session.status.hash(&mut h);
            pane.session.cwd.hash(&mut h);
            pane.session.vt.title().hash(&mut h);
            pane.session.blocks.buffer.hash(&mut h);
            pane.selection.hash(&mut h);
            pane.label.hash(&mut h);
            pane.session.blocks.prompt_revision.hash(&mut h);
            pane.session.blocks.cwd.hash(&mut h);
            pane.session.blocks.word.hash(&mut h);
            pane.session
                .blocks
                .records
                .back()
                .map(|b| (b.id, b.status, b.end.is_some(), b.collapsed))
                .hash(&mut h);
            pane.typist.hash(&mut h);
            pane.flash
                .is_some_and(|at| now.saturating_duration_since(at) < FLASH)
                .hash(&mut h);
        }
        if !paper.on
            && app.focused
            && app
                .focus_id()
                .and_then(|id| app.panes.get(&id))
                .is_some_and(|p| p.session.vt.cursor_style().blink)
        {
            caret_visible(now, app.typed).hash(&mut h);
        }
        // Async read-only pages have independently owned versions. Hash only the open page.
        if paper.studio.open {
            terminal_core::studio::lines(&paper.studio).hash(&mut h);
        }
        if paper.thread.open {
            terminal_core::thread::lines(&paper.thread).hash(&mut h);
        }
        if paper.run.open {
            terminal_core::run::lines(&paper.run).hash(&mut h);
        }
        if paper.files.open {
            terminal_core::files::lines(&paper.files).hash(&mut h);
        }
        if paper.rules.open || paper.gym.open {
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if paper.rules.open {
                terminal_core::rules::lines(&paper.rules, epoch).hash(&mut h);
            }
            if paper.gym.open {
                terminal_core::gym::lines(&paper.gym, epoch).hash(&mut h);
            }
        }
        if app.products.focus.is_some() {
            app.products.status().to_string().hash(&mut h);
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_content_never_queues_another_frame_and_requests_coalesce() {
        let mut state = Invalidation::default();
        assert!(state.request(1));
        assert!(!state.request(2));
        state.presented(2);
        for _ in 0..10_000 {
            assert!(!state.request(2));
        }
        assert!(state.request(3));
        state.presented(3);
        state.forced = true;
        assert!(state.request(3));
    }
    #[test]
    fn caret_animation_has_a_deadline_and_settles_visible() {
        let typed = Instant::now();
        assert!(caret_visible(typed, typed));
        assert!(!caret_visible(typed + BLINK, typed));
        assert!(caret_visible(typed + BLINK_LIFE, typed));
        assert!(caret_visible(typed + Duration::from_secs(3600), typed));
    }
    #[cfg(unix)]
    #[test]
    fn overlay_output_input_selection_copy_resize_and_idle_invalidate() {
        let scratch = std::env::var_os("OPENAGENTS_SCRATCH").expect("Run through the build lease");
        let root = tempfile::tempdir_in(scratch).unwrap();
        let mut overlay = Overlay::with(
            root.path(),
            "/bin/sh".into(),
            crate::pty::Program::Command {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "exec cat".into()],
                label: "fixture".into(),
            },
        );
        overlay.mount = crate::Mount::Window;
        overlay.open = true;
        overlay.focused = true;
        overlay.ensure_started();
        let atlas = crate::ui::Atlas::new(16.0);
        overlay.fit(&atlas, [1200.0, 800.0]);
        let now = Instant::now();
        assert!(overlay.redraw_needed(now));
        overlay.presented(now);
        for _ in 0..1000 {
            assert!(!overlay.redraw_needed(now));
        }
        assert!(
            overlay.animation_deadline(now).is_none(),
            "The default sheet's caret is steady"
        );
        let id = overlay.focus_id().unwrap();
        overlay
            .panes
            .get_mut(&id)
            .unwrap()
            .session
            .vt
            .feed(b"output changed\r\n");
        assert!(overlay.redraw_needed(now));
        assert!(
            !overlay.redraw_needed(now),
            "Output requests coalesce until presented"
        );
        overlay.presented(now);
        overlay.panes.get_mut(&id).unwrap().session.blocks.at_prompt = true;
        overlay.key(&crate::KeyIn {
            code: winit::keyboard::KeyCode::KeyA,
            logical: winit::keyboard::Key::Character("a".into()),
            text: Some("a".into()),
            plain: Some("a".into()),
            pressed: true,
            repeat: false,
            synthetic: false,
        });
        assert_eq!(overlay.paper.input, "a");
        assert!(overlay.redraw_needed(now));
        overlay.presented(now);
        overlay.paper.on = false;
        overlay.typed = now - Duration::from_secs(10);
        overlay.presented(now);
        let rect = overlay.rect_of(id).unwrap();
        let inner = terminal_core::layout::inner(rect, overlay.cell);
        let point = [
            inner.x + overlay.cell[0] * 0.5,
            inner.y + overlay.cell[1] * 0.5,
        ];
        overlay.press(point);
        overlay.pointer([point[0] + overlay.cell[0] * 4.0, point[1]]);
        overlay.release([point[0] + overlay.cell[0] * 4.0, point[1]]);
        assert!(overlay.panes[&id].selection.is_some_and(|s| !s.empty()));
        assert!(
            overlay.redraw_needed(now),
            "Mouse selection redraws without a VT change"
        );
        overlay.presented(now);
        overlay.enter_copy(false);
        assert!(overlay.redraw_needed(now));
        overlay.presented(now);
        let copy_at = overlay.copy.as_ref().unwrap().cursor;
        overlay.key(&crate::KeyIn {
            code: winit::keyboard::KeyCode::ArrowRight,
            logical: winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight),
            text: None,
            plain: None,
            pressed: true,
            repeat: false,
            synthetic: false,
        });
        assert_ne!(overlay.copy.as_ref().unwrap().cursor, copy_at);
        assert!(
            overlay.redraw_needed(now),
            "Copy navigation redraws its cursor"
        );
        overlay.presented(now);
        overlay.fit(&atlas, [1000.0, 700.0]);
        assert!(overlay.redraw_needed(now), "Layout changes redraw");
        overlay.presented(now);
        assert!(!overlay.redraw_needed(now + Duration::from_secs(3600)));
        assert!(overlay.animation_deadline(now).is_none());
        overlay.copy = None;
        overlay.presented(now);
        let shown = overlay.shown();
        overlay
            .apply(&terminal_core::control::Request::Zoom)
            .unwrap();
        assert_eq!(overlay.shown(), shown);
        assert!(
            overlay.redraw_needed(now),
            "A single pane's zoom label redraws"
        );
        overlay.presented(now);
        overlay
            .apply(&terminal_core::control::Request::Zoom)
            .unwrap();
        overlay.split(
            terminal_core::layout::Axis::Columns,
            &crate::pty::Program::Command {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "exec cat".into()],
                label: "second fixture".into(),
            },
        );
        overlay.fit(&atlas, [1000.0, 700.0]);
        assert_ne!(overlay.focus_id(), Some(id));
        overlay.presented(now);
        let shown = overlay.shown();
        overlay
            .apply(&terminal_core::control::Request::Focus {
                direction: None,
                pane: Some(id),
            })
            .unwrap();
        assert_eq!(overlay.shown(), shown);
        assert!(
            overlay.redraw_needed(now),
            "Control focus changes redraw the active pane"
        );
        overlay.presented(now);
        assert!(!overlay.redraw_needed(now));
        overlay.shutdown();
    }
    #[cfg(unix)]
    #[test]
    fn failed_acquisitions_wait_for_their_deadline_and_recover() {
        // No surface or session is opened; the actual overlay scheduling methods run.
        let root = tempfile::tempdir_in(
            std::env::var_os("OPENAGENTS_SCRATCH").expect("Run through the build lease"),
        )
        .unwrap();
        let mut overlay = Overlay::with(root.path(), "/bin/sh".into(), crate::pty::Program::Shell);
        let start = Instant::now();
        let mut now = start;
        for _ in 0..10 {
            assert!(overlay.redraw_needed(now));
            let retry = now + IDLE_POLL;
            overlay.defer_presentation(retry);
            assert_eq!(overlay.animation_deadline(now), Some(retry));
            for _ in 0..1000 {
                assert!(overlay.presentation_deferred(now));
                assert!(!overlay.redraw_needed(now));
            }
            assert!(!overlay.redraw_needed(retry - Duration::from_nanos(1)));
            now = retry;
        }
        assert!(overlay.redraw_needed(now));
        overlay.presented(now);
        assert!(!overlay.redraw_needed(now + IDLE_POLL));
        overlay.defer_presentation(now + IDLE_POLL);
        overlay.invalidate();
        assert!(
            !overlay.presentation_deferred(now),
            "Reconfiguration permits recovery immediately"
        );
        assert!(overlay.redraw_needed(now));
        overlay.presented(now);
        assert!(!overlay.redraw_needed(now));
    }
}
