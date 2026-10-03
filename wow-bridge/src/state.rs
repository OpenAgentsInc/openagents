//! Project decoded descriptors into bounded agent observations.
use benilla_protocol::{Character, ObjectFields};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub struct Entity {
    pub position: [f32; 3],
    pub orientation: f32,
    pub fields: ObjectFields,
    pub kind: String,
}

pub struct State {
    pub guid: u64,
    pub name: String,
    pub map: u32,
    pub position: [f32; 3],
    pub orientation: f32,
    pub entities: BTreeMap<u64, Entity>,
    pub items: BTreeMap<u64, ObjectFields>,
    pub speed: f32,
    pub deaths: u64,
    pub earned_xp: u64,
    pub turned_in: Vec<u32>,
}

impl State {
    pub fn new(c: &Character) -> Self {
        Self {
            guid: c.guid,
            name: c.name.clone(),
            map: c.map,
            position: [c.position.x, c.position.y, c.position.z],
            orientation: 0.0,
            entities: BTreeMap::new(),
            items: BTreeMap::new(),
            speed: 7.0,
            deaths: 0,
            earned_xp: 0,
            turned_in: Vec::new(),
        }
    }

    pub fn fields(&self) -> Option<&ObjectFields> {
        self.entities.get(&self.guid).map(|e| &e.fields)
    }

    pub fn observation(&self, radius: f32) -> Value {
        let own = self.fields();
        let mut inventory = BTreeMap::<u32, u32>::new();
        let mut bags = Vec::new();
        if let Some(fields) = own {
            for slot in 0..16 {
                if let Some(guid) = fields.player_pack_slot(slot).filter(|g| *g != 0)
                    && let Some(item) = self.items.get(&guid)
                {
                    let entry = item.object_entry().unwrap_or(0);
                    let count = item.item_stack_count().unwrap_or(1);
                    *inventory.entry(entry).or_default() += count;
                    bags.push(json!({"bag":255,"slot":23+slot,"entry":entry,"count":count,"guid":guid.to_string()}));
                }
            }
        }
        let quests: Vec<Value> = own
            .map(|f| {
                (0..20)
                    .filter_map(|s| f.player_quest_log(s))
                    .filter(|q| q.quest_id != 0)
                    .map(|q| json!({"id":q.quest_id,"state":q.state,"counters":q.counters}))
                    .collect()
            })
            .unwrap_or_default();
        let mut nearby: Vec<_> = self.entities.iter().filter(|(g,e)| **g != self.guid && distance(self.position,e.position)<=radius)
            .map(|(guid,e)| json!({"guid":guid.to_string(),"kind":e.kind,"entry":e.fields.object_entry(),
                "position":e.position,"health":e.fields.unit_health(),"level":e.fields.unit_level()})).collect();
        nearby.sort_by(|a, b| {
            fn d(v: &Value, p: [f32; 3]) -> f32 {
                let a = v["position"].as_array().unwrap();
                distance(
                    p,
                    [
                        a[0].as_f64().unwrap() as f32,
                        a[1].as_f64().unwrap() as f32,
                        a[2].as_f64().unwrap() as f32,
                    ],
                )
            }
            d(a, self.position).total_cmp(&d(b, self.position))
        });
        nearby.truncate(256);
        json!({"name":self.name,"guid":self.guid.to_string(),"position":self.position,"map":self.map,
            "level":own.and_then(ObjectFields::unit_level),"xp":own.and_then(ObjectFields::player_xp),
            "earned_xp":self.earned_xp,"health":own.and_then(ObjectFields::unit_health),
            "powers":(0..5).map(|i| own.and_then(|f|f.unit_power(i))).collect::<Vec<_>>(),
            "gold":own.and_then(ObjectFields::player_money),"inventory":inventory,"bags":bags,
            "quests":quests,"turned_in":self.turned_in,"nearby":nearby,"deaths":self.deaths})
    }
}

pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}
