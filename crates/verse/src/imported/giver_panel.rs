//! Authored dialogue over current, server-admitted giver interactions.
use crate::ui::{Atlas, UiBatch};
use verse_world::service::{progression::Marker, view::View};

#[derive(Default)]
pub struct Panel {
    quest: usize,
    page: usize,
}
#[derive(Debug, PartialEq)]
pub enum Action {
    Accept(u64, verse_engine::core::LifeId),
    Claim(u64),
    Close,
}
fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
fn lines(text: &str, columns: usize) -> Vec<String> {
    let mut result = Vec::new();
    let columns = columns.max(1);
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.len() + word.len() + 1 > columns {
                result.push(std::mem::take(&mut line));
            }
            for chunk in word.as_bytes().chunks(columns) {
                let chunk = std::str::from_utf8(chunk).unwrap();
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(chunk);
                if line.len() >= columns {
                    result.push(std::mem::take(&mut line));
                }
            }
        }
        result.push(line);
    }
    result
}
impl Panel {
    pub fn reset(&mut self) {
        self.quest = 0;
        self.page = 0;
    }
    pub fn contains(&self, view: &View, point: [f32; 2]) -> bool {
        view.interaction().is_some() && inside([20., 160., 500., 420.], point)
    }
    pub fn draw(&mut self, ui: &mut UiBatch, atlas: &Atlas, view: &View) {
        let Some(interaction) = view.interaction() else {
            return;
        };
        let quests = view.interaction_quests();
        if quests.is_empty() {
            return;
        }
        self.quest = self.quest.min(quests.len() - 1);
        let quest = quests[self.quest];
        let name = view
            .replica()
            .latest()
            .and_then(|s| {
                s.presentation
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == interaction.giver)
            })
            .map_or("Quest giver", |p| p.actor.name.as_str());
        ui.rect(atlas, 20., 160., 500., 420., [0.06, 0.035, 0.02, 0.98]);
        ui.frame(atlas, 20., 160., 500., 420., 2., [0.7, 0.54, 0.27, 1.]);
        let gold = [1., 0.82, 0.45, 1.];
        ui.text(atlas, 36., 174., name, gold);
        ui.text(atlas, 36., 204., &quest.name, gold);
        let text = lines(
            quest
                .dialogue_text()
                .unwrap_or("Complete the objective and return."),
            46,
        );
        let mut text = text;
        for item in &quest.items {
            text.extend(lines(
                &format!("Reward: item {} x{}", item.id, item.count),
                46,
            ));
        }
        self.page = self.page.min(text.len().saturating_sub(1) / 10);
        for (i, line) in text.iter().skip(self.page * 10).take(10).enumerate() {
            ui.text(
                atlas,
                36.,
                238. + i as f32 * 21.,
                line,
                [0.92, 0.85, 0.7, 1.],
            );
        }
        ui.text(
            atlas,
            36.,
            454.,
            &format!(
                "Objective: {} / {}   XP: {}",
                quest.progress, quest.goal, quest.experience
            ),
            gold,
        );
        ui.text(
            atlas,
            36.,
            480.,
            &format!(
                "Quest {} / {}   Text {} / {}",
                self.quest + 1,
                quests.len(),
                self.page + 1,
                text.len().div_ceil(10)
            ),
            gold,
        );
        for (x, label) in [
            (36., "< Quest"),
            (140., "Quest >"),
            (250., "< Text"),
            (350., "Text >"),
        ] {
            ui.text(atlas, x, 510., label, gold);
        }
        let label = match quest.marker() {
            Some(Marker::Available) => "Accept",
            Some(Marker::TurnIn) => "Complete",
            _ => "In progress",
        };
        ui.text(atlas, 36., 550., label, gold);
        ui.text(atlas, 430., 550., "Close", gold);
    }
    pub fn click(&mut self, view: &View, point: [f32; 2]) -> Option<Action> {
        if !self.contains(view, point) {
            return None;
        }
        let quests = view.interaction_quests();
        if inside([420., 540., 90., 30.], point) {
            return Some(Action::Close);
        }
        if inside([36., 500., 400., 30.], point) {
            match point[0] as u32 {
                0..140 => {
                    self.quest = self.quest.saturating_sub(1);
                    self.page = 0;
                }
                140..250 => {
                    self.quest = (self.quest + 1).min(quests.len().saturating_sub(1));
                    self.page = 0;
                }
                250..350 => self.page = self.page.saturating_sub(1),
                _ => self.page = self.page.saturating_add(1),
            }
            return None;
        }
        let quest = quests.get(self.quest.min(quests.len().saturating_sub(1)))?;
        if !inside([36., 540., 150., 30.], point) || !quest.interactable {
            return None;
        }
        match quest.marker()? {
            Marker::Available => Some(Action::Accept(quest.id, quest.giver_life?)),
            Marker::TurnIn => Some(Action::Claim(quest.id)),
            Marker::Active => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dialogue_wrap_retains_words_and_long_tokens() {
        assert_eq!(
            lines("one two three\nfour", 7),
            vec!["one two", "three", "four"]
        );
        assert_eq!(lines("abcdefghijk", 4), vec!["abcd", "efgh", "ijk"]);
    }
}
