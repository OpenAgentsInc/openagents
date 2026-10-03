//! Bounded game actions use server outcomes as evidence.
use crate::session::Live;
use crate::state::distance;
use anyhow::{Result, bail, ensure};
use benilla_protocol::messages;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

impl Live {
    fn select(&mut self, args: &Value, alive: bool) -> Result<u64> {
        let guid = if let Some(g) = args["guid"].as_str() {
            g.parse::<u64>()?
        } else {
            let entry = args["entry"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("entry or guid required"))?;
            self.state
                .entities
                .iter()
                .filter(|(g, e)| {
                    **g != self.state.guid
                        && e.fields.object_entry().map(u64::from) == Some(entry)
                        && (!alive || e.fields.unit_health().unwrap_or(0) > 0)
                })
                .min_by(|(_, a), (_, b)| {
                    distance(self.state.position, a.position)
                        .total_cmp(&distance(self.state.position, b.position))
                })
                .map(|(g, _)| *g)
                .ok_or_else(|| anyhow::anyhow!("no matching visible target"))?
        };
        ensure!(
            self.state.entities.contains_key(&guid),
            "target is not visible"
        );
        self.writer.set_selection(guid)?;
        self.target = Some(guid);
        Ok(guid)
    }
    fn current(&self) -> Result<u64> {
        self.target
            .ok_or_else(|| anyhow::anyhow!("select a target first"))
    }
    fn in_range(&self, guid: u64, range: f32) -> Result<()> {
        let e = self
            .state
            .entities
            .get(&guid)
            .ok_or_else(|| anyhow::anyhow!("target left visibility"))?;
        ensure!(
            distance(self.state.position, e.position) <= range,
            "target is out of range"
        );
        Ok(())
    }
    pub fn action(&mut self, op: &str, args: &Value, seconds: u64) -> Result<Value> {
        self.pump(Duration::from_millis(100))?;
        match op {
            "target" => {
                let g = self.select(args, true)?;
                Ok(json!({"guid":g.to_string(),"position":self.state.entities[&g].position}))
            }
            "attack" => {
                let g = self.current()?;
                self.in_range(g, 5.0)?;
                let own = self.state.position;
                let dest = self.state.entities[&g].position;
                self.state.orientation = (dest[1] - own[1]).atan2(dest[0] - own[0]);
                self.writer.send_movement(
                    messages::opcode::MSG_MOVE_HEARTBEAT,
                    0,
                    own,
                    self.state.orientation,
                    0.0,
                    0,
                    None,
                    None,
                )?;
                self.writer.attack_swing(g)?;
                let end = Instant::now() + Duration::from_secs(seconds);
                let result = (|| -> Result<bool> {
                    while Instant::now() < end {
                        self.pump(Duration::from_millis(100))?;
                        ensure!(
                            self.state
                                .fields()
                                .and_then(|f| f.unit_health())
                                .unwrap_or(0)
                                > 0,
                            "player died"
                        );
                        let health = self
                            .state
                            .entities
                            .get(&g)
                            .and_then(|e| e.fields.unit_health());
                        if health == Some(0) {
                            return Ok(true);
                        }
                        ensure!(health.is_some(), "target left visibility");
                        let target = self.state.entities[&g].position;
                        let own = self.state.position;
                        let d = distance(own, target);
                        if d > 2.0 {
                            let step = (self.state.speed * 0.1).min(d - 1.5);
                            let next =
                                std::array::from_fn(|i| own[i] + (target[i] - own[i]) * step / d);
                            self.state.orientation = (target[1] - own[1]).atan2(target[0] - own[0]);
                            self.writer.send_movement(
                                messages::opcode::MSG_MOVE_HEARTBEAT,
                                1,
                                next,
                                self.state.orientation,
                                0.0,
                                0,
                                None,
                                None,
                            )?;
                            self.state.position = next;
                        }
                    }
                    Ok(false)
                })();
                let stop = self.writer.attack_stop();
                self.writer.send_movement(
                    messages::opcode::MSG_MOVE_STOP,
                    0,
                    self.state.position,
                    self.state.orientation,
                    0.0,
                    0,
                    None,
                    None,
                )?;
                let dead = result?;
                stop?;
                self.pump(Duration::from_millis(200))?;
                Ok(json!({"dead":dead,"guid":g.to_string(),"killed":self.state.killed}))
            }
            "cast" => {
                let spell = u32::try_from(
                    args["spell"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("spell required"))?,
                )?;
                let target = if args["target"].as_str() == Some("self") {
                    None
                } else {
                    Some(self.current()?)
                };
                self.cast_result = None;
                self.writer.cast_spell(spell, target)?;
                let end = Instant::now() + Duration::from_secs(seconds.min(10));
                while self.cast_result.is_none() && Instant::now() < end {
                    self.pump(Duration::from_millis(50))?;
                }
                let (id, success, reason) = self
                    .cast_result
                    .ok_or_else(|| anyhow::anyhow!("cast verdict did not arrive"))?;
                ensure!(id == spell && success, "cast refused: {reason:?}");
                Ok(json!({"spell":spell,"success":success}))
            }
            "loot" => {
                let g = self.current()?;
                self.in_range(g, 5.0)?;
                self.loot_window = None;
                self.loot_error = None;
                self.writer.loot(g)?;
                let end = Instant::now() + Duration::from_secs(seconds.min(10));
                while self.loot_window.is_none()
                    && self.loot_error.is_none()
                    && Instant::now() < end
                {
                    self.pump(Duration::from_millis(50))?;
                }
                ensure!(
                    self.loot_error.is_none(),
                    "loot refused: {:?}",
                    self.loot_error
                );
                let (guid, gold, items) = self
                    .loot_window
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("loot window did not arrive"))?;
                ensure!(guid == g, "unexpected loot window");
                let result = (|| -> Result<()> {
                    if gold > 0 {
                        self.writer.loot_money()?;
                    }
                    for item in &items {
                        if item.slot_type == 0 || item.slot_type == 4 {
                            self.writer.autostore_loot_item(item.slot)?;
                            self.pump(Duration::from_millis(100))?;
                        }
                    }
                    Ok(())
                })();
                let release = self.writer.loot_release(g);
                result?;
                release?;
                self.pump(Duration::from_millis(300))?;
                Ok(
                    json!({"gold":gold,"items":items.iter().map(|i|json!({"entry":i.item_id,"count":i.count})).collect::<Vec<_>>()}),
                )
            }
            "quest" => {
                let g = if args.get("entry").is_some() || args.get("guid").is_some() {
                    self.select(args, true)?
                } else {
                    self.current()?
                };
                self.in_range(g, 6.0)?;
                let q = u32::try_from(
                    args["quest"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("quest required"))?,
                )?;
                match args["action"].as_str() {
                    Some("accept") => {
                        self.writer.questgiver_hello(g)?;
                        self.pump(Duration::from_millis(100))?;
                        self.writer.questgiver_query_quest(g, q)?;
                        self.pump(Duration::from_millis(100))?;
                        self.writer.questgiver_accept_quest(g, q)?;
                    }
                    Some("complete") => self.writer.questgiver_complete_quest(g, q)?,
                    Some("reward") => {
                        self.writer.questgiver_request_reward(g, q)?;
                        self.pump(Duration::from_millis(100))?;
                        let reward = u32::try_from(args["reward"].as_u64().unwrap_or(0))?;
                        self.writer.questgiver_choose_reward(g, q, reward)?;
                    }
                    _ => bail!("quest action must be accept, complete, or reward"),
                }
                self.pump(Duration::from_millis(500))?;
                let obs = self.state.observation(40.0);
                if args["action"].as_str() == Some("accept") {
                    ensure!(
                        obs["quests"]
                            .as_array()
                            .is_some_and(|qs| qs.iter().any(|v| v["id"] == q)),
                        "quest acceptance not confirmed"
                    );
                }
                if args["action"].as_str() == Some("reward") {
                    ensure!(
                        self.state.turned_in.contains(&q),
                        "quest reward not confirmed"
                    );
                }
                Ok(obs)
            }
            "use" => {
                let bag = u8::try_from(args["bag"].as_u64().unwrap_or(255))?;
                let slot = u8::try_from(
                    args["slot"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("slot required"))?,
                )?;
                self.writer
                    .use_item(bag, slot, 0, messages::UseItemTarget::SelfImplicit)?;
                self.pump(Duration::from_millis(500))?;
                Ok(self.state.observation(40.0))
            }
            "vendor" => {
                ensure!(
                    args["action"].as_str() == Some("sell_junk"),
                    "only sell_junk is supported"
                );
                let g = if args.get("entry").is_some() || args.get("guid").is_some() {
                    self.select(args, true)?
                } else {
                    self.current()?
                };
                self.in_range(g, 6.0)?;
                self.vendor_seen = None;
                self.writer.list_inventory(g)?;
                let response_end = Instant::now() + Duration::from_secs(seconds.min(5));
                while self.vendor_seen.is_none() && Instant::now() < response_end {
                    self.pump(Duration::from_millis(50))?;
                }
                ensure!(
                    self.vendor_seen == Some(g),
                    "vendor inventory did not arrive"
                );
                let bags = self.state.observation(40.0)["bags"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let end = Instant::now() + Duration::from_secs(seconds.min(10));
                let mut sold = Vec::new();
                for item in bags {
                    ensure!(Instant::now() < end, "vendor operation timed out");
                    let entry = item["entry"].as_u64().unwrap() as u32;
                    let guid = item["guid"].as_str().unwrap().parse::<u64>()?;
                    if !self.qualities.contains_key(&entry) {
                        self.writer.item_query(entry, guid)?;
                        while !self.qualities.contains_key(&entry) && Instant::now() < end {
                            self.pump(Duration::from_millis(50))?;
                        }
                    }
                    if self.qualities.get(&entry) == Some(&0) {
                        let before = self.state.observation(0.0)["inventory"][entry.to_string()]
                            .as_u64()
                            .unwrap_or(0);
                        self.writer.sell_item(g, guid, 0)?;
                        self.pump(Duration::from_millis(300))?;
                        let after = self.state.observation(0.0)["inventory"][entry.to_string()]
                            .as_u64()
                            .unwrap_or(0);
                        ensure!(after < before, "vendor sale not confirmed");
                        sold.push(json!({"entry":entry,"count":before-after}));
                    }
                }
                Ok(json!({"sold":sold}))
            }
            _ => bail!("unknown game action"),
        }
    }
}
