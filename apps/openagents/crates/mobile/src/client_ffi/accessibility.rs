//! Account-fenced native semantics. These objects cannot execute application intents.
use super::{CoreAccount, CoreError};
use rust_native::semantics::{
    Direction, FocusBookmark, FocusTracker, LifecycleEvent, SemanticTree,
};
use std::sync::{Arc, Mutex};

fn invalid(message: impl ToString) -> CoreError {
    CoreError::InvalidArgument {
        message: message.to_string(),
    }
}
fn json(value: &impl serde::Serialize) -> Result<String, CoreError> {
    serde_json::to_string(value).map_err(invalid)
}

#[derive(uniffi::Object)]
pub struct CoreAccessibilitySurface {
    account: Arc<CoreAccount>,
    generation: u64,
    scope: String,
    focus: Mutex<FocusTracker>,
}
impl CoreAccessibilitySurface {
    fn current(&self) -> Result<(), CoreError> {
        if self.account.controller().snapshot().generation != self.generation {
            self.focus.lock().unwrap().close();
            return Err(CoreError::Closed);
        }
        Ok(())
    }
}
#[uniffi::export]
impl CoreAccessibilitySurface {
    /// Use the same account and screen scope as the Rust view owner.
    #[uniffi::constructor]
    pub fn new(account: Arc<CoreAccount>, scope: String) -> Result<Arc<Self>, CoreError> {
        if scope.is_empty() || scope.len() > 256 {
            return Err(invalid("Invalid screen."));
        }
        let snapshot = account.controller().snapshot();
        let generation = snapshot.generation;
        let scope = match snapshot.phase {
            openagents_account::Phase::SignedIn { account } => format!("{}:{scope}", account.id),
            _ => format!("anonymous:{generation}:{scope}"),
        };
        Ok(Arc::new(Self {
            account,
            generation,
            scope,
            focus: Mutex::new(FocusTracker::default()),
        }))
    }
    /// Accept only local Rust view bytes; validation adds no intent authority.
    pub fn prepare(&self, bytes: Vec<u8>) -> Result<String, CoreError> {
        self.current()?;
        let view = rust_native::View::<serde_json::Value>::from_json(&bytes).map_err(invalid)?;
        let tree = SemanticTree::of(&view);
        let out = json(&tree)?;
        if out.len() > rust_native::semantics::MAX_SEMANTIC_TREE_BYTES {
            return Err(invalid("Accessibility labels exceed their size limit."));
        }
        self.focus.lock().unwrap().prepare(tree).map_err(invalid)?;
        self.current()?;
        Ok(out)
    }
    pub fn acknowledge(
        &self,
        instance: String,
        revision: u64,
    ) -> Result<Option<String>, CoreError> {
        self.current()?;
        let out = self
            .focus
            .lock()
            .unwrap()
            .acknowledge(&instance, revision)
            .map_err(invalid)?;
        self.current()?;
        Ok(out)
    }
    pub fn focused(&self, instance: String, revision: u64, key: String) -> Result<bool, CoreError> {
        self.current()?;
        let out = self.focus.lock().unwrap().focus(&instance, revision, &key);
        self.current()?;
        Ok(out)
    }
    /// Only a scope and node key may survive process recreation. Never save field values.
    pub fn bookmark(&self) -> Result<Option<String>, CoreError> {
        self.current()?;
        let bookmark = self.focus.lock().unwrap().bookmark(&self.scope);
        bookmark.as_ref().map(json).transpose()
    }
    pub fn restore(&self, bytes: String) -> Result<bool, CoreError> {
        self.current()?;
        if bytes.len() > 1024 {
            return Err(invalid("Invalid focus."));
        }
        let bookmark: FocusBookmark = serde_json::from_str(&bytes).map_err(invalid)?;
        Ok(self.focus.lock().unwrap().restore(&self.scope, bookmark))
    }
    /// Return requests to existing account/session/media owners; do not change their state here.
    pub fn lifecycle(&self, event: String) -> Result<String, CoreError> {
        self.current()?;
        if event.len() > 64 {
            return Err(invalid("Invalid event."));
        }
        let event: LifecycleEvent =
            serde_json::from_value(serde_json::Value::String(event)).map_err(invalid)?;
        let recovery = event.recovery();
        if recovery.retire_callbacks {
            self.focus.lock().unwrap().invalidate();
        }
        json(&recovery)
    }
    pub fn environment(
        &self,
        font_scale: f32,
        density: f32,
        reduced_motion: bool,
    ) -> Result<String, CoreError> {
        self.current()?;
        let environment = rust_native::semantics::Environment {
            font_scale,
            density,
            reduced_motion,
        };
        if !environment.valid() {
            return Err(invalid("Invalid display settings."));
        }
        json(&environment)
    }
    pub fn close(&self) {
        self.focus.lock().unwrap().close();
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum AccessibilityDirection {
    Earlier,
    Later,
}
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AccessibilityBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct AccessibilityRow {
    pub key: u64,
    /// Bounds are measured by Rust in the current transcript frame, in layout points.
    pub bounds: AccessibilityBounds,
    pub text: String,
    pub next_byte: Option<u32>,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct AccessibilityPage {
    pub revision: u64,
    pub rows: Vec<AccessibilityRow>,
    pub earlier: bool,
    pub later: bool,
    pub earlier_label: String,
    pub later_label: String,
}

/// Reads the current immutable frame, with no viewport or session mutation.
#[derive(uniffi::Object)]
pub struct CoreTranscriptAccessibility {
    account: Arc<CoreAccount>,
    generation: u64,
    transcript: Arc<crate::layout::TranscriptView>,
}
#[uniffi::export]
impl CoreTranscriptAccessibility {
    #[uniffi::constructor]
    pub fn new(
        account: Arc<CoreAccount>,
        transcript: Arc<crate::layout::TranscriptView>,
    ) -> Arc<Self> {
        let generation = account.controller().snapshot().generation;
        Arc::new(Self {
            account,
            generation,
            transcript,
        })
    }
    pub fn page(
        &self,
        revision: u64,
        anchor: u64,
        direction: AccessibilityDirection,
        count: u32,
    ) -> Result<AccessibilityPage, CoreError> {
        self.current()?;
        let frame = self.transcript.frame();
        if frame.revision() != revision {
            return Err(CoreError::Closed);
        }
        let index = frame
            .index_of(anchor)
            .ok_or_else(|| invalid("The message is no longer available."))?;
        let p = rust_native::semantics::page(
            frame.row_count(),
            index,
            match direction {
                AccessibilityDirection::Earlier => Direction::Earlier,
                AccessibilityDirection::Later => Direction::Later,
            },
            count,
        );
        let mut rows = Vec::new();
        for index in p.start..p.end {
            let row = frame.placement(index).ok_or(CoreError::Closed)?;
            let display = frame.display(index).ok_or(CoreError::Closed)?;
            let (text, next_byte) = part(&display.copy_text, 0)?;
            rows.push(AccessibilityRow {
                key: row.key,
                bounds: row_bounds(frame.width(), row.y, row.height)?,
                text,
                next_byte,
            });
        }
        self.current()?;
        if self.transcript.frame().revision() != revision {
            return Err(CoreError::Closed);
        }
        Ok(AccessibilityPage {
            revision,
            rows,
            earlier: p.earlier,
            later: p.later,
            earlier_label: "Earlier messages".into(),
            later_label: "Later messages".into(),
        })
    }
    pub fn row_part(
        &self,
        revision: u64,
        key: u64,
        byte: u32,
    ) -> Result<AccessibilityRow, CoreError> {
        self.current()?;
        let frame = self.transcript.frame();
        if frame.revision() != revision {
            return Err(CoreError::Closed);
        }
        let index = frame.index_of(key).ok_or(CoreError::Closed)?;
        let row = frame.placement(index).ok_or(CoreError::Closed)?;
        let display = frame.display(index).ok_or(CoreError::Closed)?;
        let (text, next_byte) = part(&display.copy_text, byte)?;
        self.current()?;
        if self.transcript.frame().revision() != revision {
            return Err(CoreError::Closed);
        }
        Ok(AccessibilityRow {
            key,
            bounds: row_bounds(frame.width(), row.y, row.height)?,
            text,
            next_byte,
        })
    }
}
fn row_bounds(width: f32, y: f32, height: f32) -> Result<AccessibilityBounds, CoreError> {
    let bottom = y + height;
    if !width.is_finite()
        || width <= 0.0
        || !y.is_finite()
        || y < 0.0
        || !height.is_finite()
        || height <= 0.0
        || !bottom.is_finite()
    {
        return Err(invalid("The message layout is unavailable."));
    }
    Ok(AccessibilityBounds {
        x: 0.0,
        y,
        width,
        height,
    })
}
impl CoreTranscriptAccessibility {
    fn current(&self) -> Result<(), CoreError> {
        if self.account.controller().snapshot().generation == self.generation {
            Ok(())
        } else {
            Err(CoreError::Closed)
        }
    }
}
fn part(text: &str, start: u32) -> Result<(String, Option<u32>), CoreError> {
    let start = start as usize;
    if start > text.len() || !text.is_char_boundary(start) {
        return Err(invalid("Invalid text position."));
    }
    let mut end = start.saturating_add(2048).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Ok((
        text[start..end].into(),
        if end < text.len() {
            Some(end as u32)
        } else {
            None
        },
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_transcript_bounds_reject_invalid_geometry() {
        assert_eq!(
            row_bounds(320.0, 48.0, 24.0).unwrap(),
            AccessibilityBounds {
                x: 0.0,
                y: 48.0,
                width: 320.0,
                height: 24.0
            }
        );
        assert!(row_bounds(f32::NAN, 0.0, 24.0).is_err());
        assert!(row_bounds(320.0, -1.0, 24.0).is_err());
        assert!(row_bounds(320.0, 48.0, 0.0).is_err());
        assert!(row_bounds(320.0, f32::MAX, f32::MAX).is_err());
    }
    #[test]
    fn long_text_is_bounded_and_continuations_preserve_unicode() {
        let text = "🦀é".repeat(2000);
        let mut byte = 0;
        let mut out = String::new();
        loop {
            let (s, next) = part(&text, byte).unwrap();
            assert!(s.len() <= 2048);
            out.push_str(&s);
            match next {
                Some(n) => byte = n,
                None => break,
            }
        }
        assert_eq!(out, text);
        assert!(part(&text, 1).is_err());
    }
}
