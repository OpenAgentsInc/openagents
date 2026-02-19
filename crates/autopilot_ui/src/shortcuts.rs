use std::collections::HashMap;

use wgpui::input::{Key, Modifiers};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ShortcutChord {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl ShortcutChord {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ShortcutScope {
    Global,
    App,
    Pane,
    TextInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ShortcutCommand {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    HotbarSlot(u8),
    CycleChatFocus,
    CycleChatModel,
    CloseActivePane,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ShortcutContext {
    pub text_input_focused: bool,
}

#[derive(Clone, Debug)]
pub struct ShortcutBinding {
    pub id: &'static str,
    pub chord: ShortcutChord,
    pub scope: ShortcutScope,
    pub priority: u8,
    pub command: ShortcutCommand,
}

#[derive(Clone, Debug)]
pub struct ShortcutConflict {
    pub chord: ShortcutChord,
    pub existing: ShortcutBinding,
    pub incoming: ShortcutBinding,
}

#[derive(Clone, Debug)]
pub struct ShortcutResolution {
    pub command: ShortcutCommand,
    pub conflicts: Vec<ShortcutBinding>,
}

pub struct ShortcutRegistry {
    bindings: Vec<ShortcutBinding>,
    by_chord: HashMap<ShortcutChord, Vec<usize>>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            by_chord: HashMap::new(),
        }
    }

    pub fn register(&mut self, binding: ShortcutBinding) -> Vec<ShortcutConflict> {
        let mut conflicts = Vec::new();
        if let Some(existing) = self.by_chord.get(&binding.chord) {
            for index in existing {
                if let Some(prior) = self.bindings.get(*index).cloned() {
                    conflicts.push(ShortcutConflict {
                        chord: binding.chord.clone(),
                        existing: prior,
                        incoming: binding.clone(),
                    });
                }
            }
        }
        let index = self.bindings.len();
        let chord = binding.chord.clone();
        self.bindings.push(binding);
        self.by_chord.entry(chord).or_default().push(index);
        conflicts
    }

    pub fn resolve(
        &self,
        chord: ShortcutChord,
        context: ShortcutContext,
    ) -> Option<ShortcutResolution> {
        let indices = self.by_chord.get(&chord)?;
        let mut candidates: Vec<&ShortcutBinding> = indices
            .iter()
            .filter_map(|index| self.bindings.get(*index))
            .filter(|binding| Self::scope_matches(binding.scope, context))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
        let winner = candidates[0];
        let conflicts = candidates
            .iter()
            .skip(1)
            .cloned()
            .cloned()
            .collect::<Vec<_>>();
        Some(ShortcutResolution {
            command: winner.command,
            conflicts,
        })
    }

    fn scope_matches(scope: ShortcutScope, context: ShortcutContext) -> bool {
        match scope {
            ShortcutScope::TextInput => context.text_input_focused,
            _ => true,
        }
    }
}
