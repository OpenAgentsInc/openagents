//! Video Lab's magnetic timeline. The browser sends intents; Rust owns edits,
//! links, time snapping, and the presentation shared with the local player.

use serde_json::{Value, json};

type Result<T> = std::result::Result<T, String>;

fn number(v: &Value, key: &str, default: f64) -> f64 {
    v[key].as_f64().unwrap_or(default)
}

fn shots(doc: &Value) -> Result<&Vec<Value>> {
    doc["shots"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Open a film to use the timeline.".into())
}

fn starts(doc: &Value) -> Result<Vec<(String, f64, f64)>> {
    let mut at = 0.0;
    shots(doc)?
        .iter()
        .map(|s| {
            let name = s["name"]
                .as_str()
                .ok_or("A video clip needs a name.")?
                .to_owned();
            let dur = number(s, "dur", 0.0);
            if !dur.is_finite() || dur <= 0.0 {
                return Err("A clip needs a positive length.".into());
            }
            let out = (name, at, dur);
            at += dur;
            Ok(out)
        })
        .collect()
}

fn link(doc: &Value, id: &str) -> Option<String> {
    if let Some(links) = doc["timeline"]["links"].as_object() {
        return links.get(id).and_then(Value::as_str).map(str::to_owned);
    }
    if let Some(i) = id
        .strip_prefix("text:")
        .and_then(|s| s.parse::<usize>().ok())
    {
        let cap = &doc["captions"][i];
        if let Some(name) = cap["shot"].as_str() {
            return Some(name.to_owned());
        }
        let at = number(cap, "at", 0.0);
        return starts(doc)
            .ok()?
            .into_iter()
            .find(|(_, t, dur)| at >= *t && at < t + dur)
            .map(|s| s.0);
    }
    None
}

fn initialize(doc: &mut Value) -> Result<()> {
    let mut links = serde_json::Map::new();
    if let Some(existing) = doc["timeline"]["links"].as_object() {
        links = existing.clone();
    } else {
        for i in 0..doc["captions"].as_array().map_or(0, Vec::len) {
            let id = format!("text:{i}");
            if let Some(name) = link(doc, &id) {
                links.insert(id, json!(name));
            }
        }
    }
    if !doc["timeline"].is_object() {
        doc["timeline"] = json!({});
    }
    doc["timeline"]["links"] = Value::Object(links);
    Ok(())
}

fn duration(media: &Value, src: &Value, default: f64) -> f64 {
    src.as_str()
        .map(|s| number(&media[s], "duration", default))
        .unwrap_or(default)
}

/// Describes every clip without changing the saved cut.
pub fn view(doc: &Value, media: &Value) -> Result<Value> {
    let positions = starts(doc)?;
    let total: f64 = positions.iter().map(|s| s.2).sum();
    let mut clips = Vec::new();
    for (i, (name, at, dur)) in positions.iter().enumerate() {
        let s = &doc["shots"][i];
        clips.push(json!({"id":format!("video:{name}"),"track":"video","label":s["label"].as_str().unwrap_or(name),
            "shot":name,"at":at,"dur":dur,"speed":number(s,"speed",1.0),"source":s["src"],
            "source_at":number(s,"in",0.0),"transition":number(s,"xfade",0.0),"title":s["title"].is_object()}));
        if s["sound"].is_object() {
            clips.push(json!({"id":format!("sound:{name}"),"track":"audio","label":format!("{name} · original audio"),
                "at":at,"dur":dur,"link":name,"embedded":true,"source":s["src"],"source_at":number(s,"in",0.0)}));
        }
    }
    if doc["music"].is_string() || doc["score"].is_object() {
        let at = number(&doc["timeline"], "music_at", 0.0);
        clips.push(json!({"id":"music","track":"audio","label":"Music","at":at,"dur":(total-at).max(0.0),"link":link(doc,"music"),"source":doc["music"]}));
    }
    if doc["voice"].is_object() {
        let v = &doc["voice"];
        clips.push(
            json!({"id":"voice","track":"audio","label":filename(&v["src"]),"at":number(v,"at",0.0),
            "dur":duration(media,&v["src"],1.0),"link":link(doc,"voice"),"source":v["src"]}),
        );
    }
    for (i, v) in doc["voices"].as_array().into_iter().flatten().enumerate() {
        let id = format!("audio:{i}");
        clips.push(
            json!({"id":id,"track":"audio","label":filename(&v["src"]),"at":number(v,"at",0.0),
            "dur":duration(media,&v["src"],1.0),"link":link(doc,&id),"source":v["src"]}),
        );
    }
    for (i, c) in doc["captions"].as_array().into_iter().flatten().enumerate() {
        let id = format!("text:{i}");
        clips.push(json!({"id":id,"track":"text","label":c["text"],"at":c["at"],"dur":c["dur"],"link":link(doc,&id)}));
    }
    Ok(
        json!({"duration":total,"fps":number(doc,"fps",30.0),"clips":clips,"font":"Geist","transition":"Dissolve"}),
    )
}

fn filename(src: &Value) -> &str {
    src.as_str()
        .and_then(|s| s.rsplit('/').next())
        .unwrap_or("Audio")
}

fn target<'a>(doc: &'a mut Value, id: &str) -> Result<&'a mut Value> {
    if id == "voice" && doc["voice"].is_object() {
        return Ok(&mut doc["voice"]);
    }
    let (prefix, index) = id.split_once(':').ok_or("Choose a clip first.")?;
    let key = match prefix {
        "audio" => "voices",
        "text" => "captions",
        _ => return Err("Choose an audio or text clip.".into()),
    };
    let i = index.parse::<usize>().map_err(|_| "Choose a valid clip.")?;
    doc.get_mut(key)
        .and_then(Value::as_array_mut)
        .and_then(|xs| xs.get_mut(i))
        .ok_or_else(|| "That clip no longer exists. Reload the film.".into())
}

fn move_attached(doc: &mut Value, before: &[(String, f64, f64)]) -> Result<()> {
    let after = starts(doc)?;
    let links = doc["timeline"]["links"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (id, name) in links {
        let name = name.as_str().ok_or("Choose a video clip to link to.")?;
        let old = before
            .iter()
            .find(|s| s.0 == name)
            .ok_or("The linked video no longer exists.")?;
        let new = after
            .iter()
            .find(|s| s.0 == name)
            .ok_or("The linked video no longer exists.")?;
        let delta = new.1 - old.1;
        if id == "music" {
            let at = number(&doc["timeline"], "music_at", 0.0) + delta;
            if at < -0.001 {
                return Err(
                    "That move would put linked music before the film. Unlink it first.".into(),
                );
            }
            doc["timeline"]["music_at"] = json!(at.max(0.0));
        } else {
            let clip = target(doc, &id)?;
            let at = number(clip, "at", 0.0) + delta;
            if at < -0.001 {
                return Err(
                    "That move would put a linked clip before the film. Unlink it first.".into(),
                );
            }
            clip["at"] = json!(at.max(0.0));
        }
    }
    Ok(())
}

fn reorder(doc: &mut Value, name: &str, at: f64) -> Result<()> {
    let before = starts(doc)?;
    let index = before
        .iter()
        .position(|s| s.0 == name)
        .ok_or("That video clip no longer exists.")?;
    let list = doc["shots"].as_array_mut().ok_or("Open a film first.")?;
    let shot = list.remove(index);
    let mut pos = 0.0;
    let mut best = (0, at.abs());
    for (i, s) in list.iter().enumerate() {
        pos += number(s, "dur", 0.0);
        if (at - pos).abs() < best.1 {
            best = (i + 1, (at - pos).abs());
        }
    }
    list.insert(best.0, shot);
    move_attached(doc, &before)
}

/// Applies one editor intent atomically. Callers save only a successful result.
pub fn edit(input: &Value, command: &Value, media: &Value) -> Result<Value> {
    let mut doc = input.clone();
    initialize(&mut doc)?;
    let id = command["id"].as_str().ok_or("Choose a clip first.")?;
    let current = view(&doc, media)?;
    let selected = current["clips"]
        .as_array()
        .and_then(|xs| xs.iter().find(|c| c["id"] == id))
        .ok_or("That clip no longer exists. Reload the film.")?;
    let op = command["op"].as_str().ok_or("Choose an edit.")?;
    let snap = |t: f64| {
        let fps = number(&doc, "fps", 30.0);
        (t * fps).round() / fps
    };
    match op {
        "move" => {
            let at = command["at"]
                .as_f64()
                .filter(|t| t.is_finite() && *t >= 0.0)
                .ok_or("Start time must be zero or later.")?;
            let at = snap(at);
            if let Some(name) = id.strip_prefix("video:") {
                reorder(&mut doc, name, at)?;
            } else if let Some(name) = selected["link"].as_str() {
                let positions = starts(&doc)?;
                let parent = positions
                    .iter()
                    .find(|s| s.0 == name)
                    .ok_or("The linked video no longer exists.")?;
                let desired = parent.1 + at - number(selected, "at", 0.0);
                if desired < -0.001 {
                    return Err("That move would put the video before the film.".into());
                }
                reorder(&mut doc, name, desired.max(0.0))?;
            } else if id == "music" {
                doc["timeline"]["music_at"] = json!(at);
            } else {
                target(&mut doc, id)?["at"] = json!(at);
            }
        }
        "link" => {
            if id.starts_with("video:") || selected["embedded"] == true {
                return Err("Choose a separate audio or text clip to link.".into());
            }
            let name = command["shot"].as_str().unwrap_or("");
            if name.is_empty() {
                doc["timeline"]["links"]
                    .as_object_mut()
                    .ok_or("Open the film again.")?
                    .remove(id);
            } else {
                if !starts(&doc)?.iter().any(|s| s.0 == name) {
                    return Err("Choose a video clip to link to.".into());
                }
                doc["timeline"]["links"][id] = json!(name);
            }
        }
        "speed" => {
            let name = id
                .strip_prefix("video:")
                .ok_or("Speed applies to video clips.")?;
            let speed = command["value"]
                .as_f64()
                .filter(|s| s.is_finite() && (0.25..=4.0).contains(s))
                .ok_or("Choose a speed from 0.25× to 4×.")?;
            let before = starts(&doc)?;
            let index = before
                .iter()
                .position(|s| s.0 == name)
                .ok_or("Choose a video clip.")?;
            let s = &doc["shots"][index];
            if !s["src"].is_string() || s["sound"].is_object() || s["play"].is_number() {
                return Err("Speed is available for video without speech or a held frame.".into());
            }
            let frames = (number(s, "dur", 0.0) * number(s, "speed", 1.0) / speed
                * number(&doc, "fps", 30.0))
            .round()
            .max(1.0);
            let dur = frames / number(&doc, "fps", 30.0);
            let exact_speed = number(s, "dur", 0.0) * number(s, "speed", 1.0) / dur;
            doc["shots"][index]["speed"] = json!(exact_speed);
            doc["shots"][index]["dur"] = json!(dur);
            move_attached(&mut doc, &before)?;
        }
        "text" => {
            if !id.starts_with("text:") {
                return Err("Choose a text clip.".into());
            }
            let text = command["value"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or("Text cannot be empty.")?;
            target(&mut doc, id)?["text"] = json!(text.trim());
        }
        "duration" => {
            if !id.starts_with("text:") {
                return Err("Choose a text clip.".into());
            }
            let dur = command["value"]
                .as_f64()
                .filter(|s| s.is_finite() && *s > 0.0)
                .ok_or("Length must be greater than zero.")?;
            let dur = snap(dur);
            target(&mut doc, id)?["dur"] = json!(dur);
        }
        "transition" => {
            let name = id.strip_prefix("video:").ok_or("Choose a video clip.")?;
            let index = starts(&doc)?
                .iter()
                .position(|s| s.0 == name)
                .ok_or("Choose a video clip.")?;
            if index == 0 {
                return Err("The first clip has no incoming transition.".into());
            }
            let enabled = command["value"]
                .as_bool()
                .ok_or("Choose Cut or Dissolve.")?;
            doc["shots"][index]["xfade"] = json!(if enabled { 0.5 } else { 0.0 });
            doc["shots"][index]["xfade_kind"] = json!("fade");
        }
        _ => return Err("That edit is not available.".into()),
    }
    let result = view(&doc, media)?;
    let total = number(&result, "duration", 0.0);
    for clip in result["clips"].as_array().into_iter().flatten() {
        if number(clip, "at", 0.0) + number(clip, "dur", 0.0) > total + 0.001 {
            return Err(format!(
                "{} would extend past the film. Move it earlier or unlink it first.",
                clip["label"].as_str().unwrap_or("A clip")
            ));
        }
        if number(clip, "dur", 0.0) <= 0.0 {
            return Err("A clip needs at least one frame.".into());
        }
    }
    Ok(doc)
}

/// Builds the music timing filter consumed by the existing ffmpeg adapter.
pub fn music_filter(doc: &Value) -> Result<String> {
    let at = number(&doc["timeline"], "music_at", 0.0);
    if !at.is_finite() || at < 0.0 {
        return Err("Music must start at zero or later.".into());
    }
    Ok(format!("adelay={0}|{0},apad", (at * 1000.0).round() as u64))
}

/// JSON protocol for the local service, renderer, and headless acceptance checks.
pub fn request(input: &Value) -> Result<Value> {
    let doc = &input["document"];
    let media = &input["media"];
    match input["action"].as_str().unwrap_or("view") {
        "view" => view(doc, media),
        "edit" => {
            let updated = edit(doc, &input["command"], media)?;
            Ok(json!({"document":updated,"timeline":view(&updated,media)?}))
        }
        "music" => Ok(json!({"filter":music_filter(doc)?})),
        "follow" => {
            let mut updated = input["command"].clone();
            move_attached(&mut updated, &starts(doc)?)?;
            Ok(updated)
        }
        _ => Err("That timeline action is not available.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut() -> Value {
        json!({"fps":30,"shots":[{"name":"a","src":"a.mp4","dur":4},
            {"name":"b","src":"b.mp4","dur":4},{"name":"c","src":"c.mp4","dur":4}],
            "music":"music.wav","voices":[{"src":"voice.wav","at":4.5}],
            "captions":[{"text":"Over B","at":5,"dur":1,"shot":"b"}]})
    }
    fn apply(doc: &Value, command: Value) -> Value {
        edit(doc, &command, &json!({"voice.wav":{"duration":1.5}})).unwrap()
    }
    #[test]
    fn linked_audio_and_text_preserve_offsets_in_both_directions() {
        let doc = apply(&cut(), json!({"op":"link","id":"audio:0","shot":"b"}));
        let moved = apply(&doc, json!({"op":"move","id":"video:b","at":0}));
        assert_eq!(moved["shots"][0]["name"], "b");
        assert_eq!(moved["voices"][0]["at"], 0.5);
        assert_eq!(moved["captions"][0]["at"], 1.0);
        let back = apply(&moved, json!({"op":"move","id":"audio:0","at":8.5}));
        assert_eq!(back["shots"][2]["name"], "b");
        assert_eq!(back["voices"][0]["at"], 8.5);
        assert_eq!(back["captions"][0]["at"], 9.0);
        assert_eq!(doc["shots"][0]["name"], "a");
    }
    #[test]
    fn unlinking_is_saved_and_independent_audio_can_move_by_one_frame() {
        let doc = apply(&cut(), json!({"op":"link","id":"text:0","shot":""}));
        let moved = apply(&doc, json!({"op":"move","id":"video:b","at":0}));
        assert_eq!(moved["captions"][0]["at"], 5);
        let moved = apply(&moved, json!({"op":"move","id":"audio:0","at":1.02}));
        assert!((moved["voices"][0]["at"].as_f64().unwrap() - 31.0 / 30.0).abs() < 1e-9);
        assert_eq!(moved["shots"][0]["name"], "b");
    }
    #[test]
    fn speed_preserves_source_range_and_moves_later_linked_clips() {
        let doc = apply(&cut(), json!({"op":"link","id":"audio:0","shot":"b"}));
        let doc = apply(&doc, json!({"op":"speed","id":"video:a","value":2}));
        assert_eq!(doc["shots"][0]["dur"], 2.0);
        assert_eq!(doc["shots"][0]["speed"], 2.0);
        assert_eq!(doc["voices"][0]["at"], 2.5);
        assert_eq!(doc["captions"][0]["at"], 3.0);
    }
    #[test]
    fn rejected_group_move_leaves_the_original_intact() {
        let original = cut();
        let doc = apply(&original, json!({"op":"link","id":"audio:0","shot":"c"}));
        let error = edit(
            &doc,
            &json!({"op":"move","id":"video:c","at":0}),
            &json!({}),
        )
        .unwrap_err();
        assert!(error.contains("before the film"));
        assert_eq!(doc["voices"][0]["at"], 4.5);
        assert_eq!(doc["shots"][0]["name"], "a");
    }
    #[test]
    fn invalid_commands_and_clips_past_the_end_are_refused() {
        for command in [
            json!({"op":"move","id":"audio:0","at":12}),
            json!({"op":"speed","id":"video:a","value":0}),
            json!({"op":"link","id":"audio:0","shot":"missing"}),
            json!({"op":"duration","id":"text:0","value":0.001}),
        ] {
            assert!(edit(&cut(), &command, &json!({})).is_err());
        }
    }
    #[test]
    fn title_and_original_audio_appear_as_clips() {
        let mut doc = cut();
        doc["shots"][0]["sound"] = json!({});
        doc["shots"][2] =
            json!({"name":"c","dur":4,"title":{"lines":[{"text":"The end","size":0.05}]}});
        let v = view(&doc, &json!({})).unwrap();
        let clips = v["clips"].as_array().unwrap();
        assert!(
            clips
                .iter()
                .any(|c| c["id"] == "sound:a" && c["link"] == "a")
        );
        assert!(
            clips
                .iter()
                .any(|c| c["id"] == "video:c" && c["title"] == true)
        );
    }
    #[test]
    fn music_delay_and_only_dissolve_style_are_preserved() {
        let doc = apply(&cut(), json!({"op":"move","id":"music","at":2}));
        assert_eq!(music_filter(&doc).unwrap(), "adelay=2000|2000,apad");
        let doc = apply(&doc, json!({"op":"transition","id":"video:b","value":true}));
        assert_eq!(doc["shots"][1]["xfade_kind"], "fade");
        assert_eq!(doc["shots"][1]["xfade"], 0.5);
    }
}
