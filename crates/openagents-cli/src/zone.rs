//! Inside a zone: the Lagrange 1 construction sandbox, driven by commands
//! instead of a controller. `run` steps the simulation headlessly on this
//! machine and prints what happened; `send` addresses the same verbs to the
//! client that simulates the zone on another machine, as NIP-MV `23302`
//! zone commands.

use std::time::Duration;

use glam::DVec3;
use serde_json::{Value, json};
use verse::mv::{self, Arg, Command as Wire, Received};
use verse_lagrange::station::{AIRLOCK, DEPOT, JIG, SPAWN};
use verse_lagrange::{Command, PartKind, PartState, Station};

use crate::relay::{DEFAULT_WAIT, unix_now};
use crate::{Args, Output, out};

const USAGE: &str = "usage: openagents zone COMMAND [OPTIONS]
  info                      The Lagrange zone: landmarks, parts, slots, limits.
  run VERB... [--dt SECONDS] [--timeout SECONDS] [--trace]
                            Simulate the zone headlessly and apply verbs in
                            order, printing the snapshot after each.
  build [--timeout SECONDS] Fly every part from the depot to its jig slot.
  send VERB... --to OPERATOR_PUBKEY [--zone lagrange]
                            Publish the verbs as zone commands to the client
                            that simulates the zone, and wait for its report.
  listen [--wait SECONDS]   Print zone commands addressed to this identity.
Verbs: fly X,Y,Z | fly depot|jig|airlock|spawn|PART | grab | install | release |
       stop | wait SECONDS | status | parts
  install                   Carry the held part to its jig slot and latch it.
Options for send/listen: --as PROFILE, --relay URL, --world ID.";

const VERBS: &[&str] = &[
    "fly", "grab", "install", "release", "stop", "wait", "status", "parts",
];

/// One verb with its arguments, as typed or as received.
#[derive(Clone, Debug, PartialEq)]
pub struct Verb {
    pub name: String,
    pub args: Vec<Arg>,
}

impl Verb {
    /// Groups words into verbs: a verb name followed by its arguments,
    /// until the next verb name.
    pub fn parse_all(words: &[String]) -> Result<Vec<Verb>, String> {
        let mut verbs: Vec<Verb> = Vec::new();
        for word in words {
            if VERBS.contains(&word.as_str()) {
                verbs.push(Verb {
                    name: word.clone(),
                    args: Vec::new(),
                });
                continue;
            }
            let Some(current) = verbs.last_mut() else {
                return Err(format!(
                    "`{word}` is not a verb; verbs are {}",
                    VERBS.join(", ")
                ));
            };
            if current.name == "fly"
                && let Ok(point) = Args::vec3(word)
            {
                current
                    .args
                    .extend(point.iter().map(|n| Arg::Number(f64::from(*n))));
            } else if let Ok(number) = word.parse::<f64>() {
                current.args.push(Arg::Number(number));
            } else {
                current.args.push(Arg::Text(word.clone()));
            }
            if current.args.len() > mv::MAX_COMMAND_ARGS {
                return Err(format!("`{}` has too many arguments", current.name));
            }
        }
        if verbs.is_empty() {
            return Err("at least one verb is required".into());
        }
        Ok(verbs)
    }

    fn from_wire(command: &Wire) -> Self {
        Self {
            name: command.cmd.clone(),
            args: command.args.clone(),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "cmd": self.name, "args": self.args })
    }
}

/// Where to hover to reach a part: beside its stowage, inside grab range.
fn beside(stowage: DVec3) -> DVec3 {
    stowage + DVec3::new(2.0, 0.0, 0.0)
}

/// `depot` lands beside the next stowed part when one remains; a part
/// name lands beside that part's stowage.
fn landmark(name: &str, station: &Station) -> Option<DVec3> {
    Some(match name {
        "depot" => station
            .next_part()
            .map_or(DEPOT, |kind| beside(kind.stowage())),
        "jig" => JIG,
        "airlock" => AIRLOCK,
        "spawn" => SPAWN,
        _ => beside(part_by_name(name)?.stowage()),
    })
}

fn part_by_name(name: &str) -> Option<PartKind> {
    PartKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name().eq_ignore_ascii_case(&name.replace('-', " ")))
}

fn part_json(station: &Station) -> Vec<Value> {
    station
        .parts
        .iter()
        .map(|part| {
            json!({
                "kind": part.kind.name(),
                "mass_kg": part.kind.mass(),
                "state": format!("{:?}", part.state).to_lowercase(),
                "pos": part.body.pos.to_array(),
                "vel": part.body.vel.to_array(),
                "slot": part.kind.slot().to_array(),
                "stowage": part.kind.stowage().to_array(),
            })
        })
        .collect()
}

fn snapshot_json(station: &Station) -> Value {
    let snapshot = station.snapshot();
    let mut value = serde_json::to_value(&snapshot).unwrap_or(Value::Null);
    value["pos"] = json!(station.astronaut.pos.to_array());
    value["vel"] = json!(station.astronaut.vel.to_array());
    value["hands"] = json!(station.hands().to_array());
    value["target"] = json!(station.target.map(|t| t.to_array()));
    value
}

fn render_snapshot(value: &Value) -> String {
    let pos = value["pos"]
        .as_array()
        .map(|p| {
            p.iter()
                .map(|n| format!("{:.1}", n.as_f64().unwrap_or(0.0)))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        "at {pos}  speed {:.2} m/s  carrying {}  installed {}/{}  propellant {:.0}%  dv {:.1} m/s{}",
        value["speed_m_s"].as_f64().unwrap_or(0.0),
        value["carrying"].as_str().unwrap_or("nothing"),
        value["installed"],
        value["total"],
        value["propellant_fraction"].as_f64().unwrap_or(0.0) * 100.0,
        value["delta_v_remaining_m_s"].as_f64().unwrap_or(0.0),
        value["message"]
            .as_str()
            .map(|m| format!("\n  {m}"))
            .unwrap_or_default()
    )
}

/// A headless simulation that applies verbs and settles between them.
pub struct Sim {
    pub station: Station,
    pub dt: f64,
    pub timeout: f64,
    pub elapsed: f64,
}

impl Sim {
    pub fn new(dt: f64, timeout: f64) -> Self {
        Self {
            station: Station::new(),
            dt: dt.clamp(0.005, 0.1),
            timeout: timeout.clamp(1.0, 3600.0),
            elapsed: 0.0,
        }
    }

    fn idle(&mut self, seconds: f64) {
        let steps = (seconds / self.dt).ceil().max(0.0) as u64;
        for _ in 0..steps {
            self.station.step(self.dt, &Command::default());
            self.elapsed += self.dt;
        }
    }

    /// Flies so the held part sits on its slot, then releases it. The part
    /// hangs off the hands, so the pilot aims at the slot minus that offset
    /// and corrects until the latch is within range.
    fn install(&mut self) -> Result<PartKind, String> {
        let carried = |station: &Station| {
            station
                .parts
                .iter()
                .find(|part| part.state == PartState::Carried)
                .map(|part| (part.kind, part.body.pos))
        };
        let (kind, _) = carried(&self.station).ok_or("Nothing is held")?;
        for _ in 0..6 {
            let (_, pos) = carried(&self.station).ok_or("Nothing is held")?;
            let offset = pos - self.station.astronaut.pos;
            self.station.fly_to(kind.slot() - offset)?;
            self.settle()?;
            self.idle(0.5);
            if self
                .station
                .latch_distance()
                .is_some_and(|distance| distance <= verse_lagrange::station::LATCH_RANGE * 0.5)
            {
                break;
            }
        }
        let released = self.station.release()?;
        self.idle(0.5);
        let installed = self
            .station
            .parts
            .iter()
            .any(|part| part.kind == released && part.state == PartState::Installed);
        if !installed {
            return Err(format!(
                "the {} did not latch: {}",
                released.name().to_lowercase(),
                self.station.message.clone().unwrap_or_default()
            ));
        }
        Ok(released)
    }

    /// Steps until the autopilot arrives or `timeout` passes.
    fn settle(&mut self) -> Result<(), String> {
        let start = self.elapsed;
        while self.station.target.is_some() {
            self.station.step(self.dt, &Command::default());
            self.elapsed += self.dt;
            if self.elapsed - start > self.timeout {
                return Err(format!(
                    "did not arrive within {:.0} s (at {:?})",
                    self.timeout,
                    self.station.astronaut.pos.to_array()
                ));
            }
        }
        Ok(())
    }

    /// Applies one verb; returns what changed.
    pub fn apply(&mut self, verb: &Verb) -> Result<Value, String> {
        let numbers = |verb: &Verb| -> Option<Vec<f64>> {
            verb.args
                .iter()
                .map(|arg| match arg {
                    Arg::Number(n) => Some(*n),
                    Arg::Text(_) => None,
                })
                .collect()
        };
        match verb.name.as_str() {
            "fly" => {
                let target = match verb.args.as_slice() {
                    [Arg::Text(name)] => landmark(name, &self.station)
                        .ok_or_else(|| format!("`{name}` is not a landmark or part"))?,
                    _ => match numbers(verb).as_deref() {
                        Some([x, y, z]) => DVec3::new(*x, *y, *z),
                        _ => return Err("fly takes X,Y,Z or a landmark name".into()),
                    },
                };
                // Carry the held part onto its latch by aiming the hands, not the chest.
                let target = if self.station.snapshot().carrying.is_some() {
                    target - (self.station.hands() - self.station.astronaut.pos)
                } else {
                    target
                };
                self.station.fly_to(target)?;
                self.settle()?;
                Ok(json!({ "arrived": self.station.astronaut.pos.to_array() }))
            }
            "grab" => {
                let kind = self.station.grab()?;
                self.idle(0.5);
                Ok(json!({ "grabbed": kind.name() }))
            }
            "install" => {
                let kind = self.install()?;
                Ok(json!({ "released": kind.name(), "installed": true }))
            }
            "release" => {
                let kind = self.station.release()?;
                self.idle(0.5);
                let installed = self
                    .station
                    .parts
                    .iter()
                    .any(|part| part.kind == kind && part.state == PartState::Installed);
                Ok(json!({ "released": kind.name(), "installed": installed }))
            }
            "stop" => {
                self.station.step(
                    self.dt,
                    &Command {
                        direction: DVec3::new(1e-3, 0.0, 0.0),
                        yaw: f64::NAN,
                        climb: false,
                    },
                );
                self.idle(2.0);
                Ok(json!({ "stopped": true }))
            }
            "wait" => {
                let seconds = match numbers(verb).as_deref() {
                    Some([seconds]) if *seconds > 0.0 => seconds.min(self.timeout),
                    _ => return Err("wait takes SECONDS".into()),
                };
                self.idle(seconds);
                Ok(json!({ "waited_s": seconds }))
            }
            "status" => Ok(json!({ "status": snapshot_json(&self.station) })),
            "parts" => Ok(json!({ "parts": part_json(&self.station) })),
            other => Err(format!("`{other}` is not a verb the zone recognizes")),
        }
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("zone", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["trace"]) {
        Ok(args) => args,
        Err(message) => return output.usage("zone", &message, USAGE),
    };
    let result = match command.as_str() {
        "info" => Ok(info(output)),
        "run" => local(output, &args, None),
        "build" => local(output, &args, Some(build_plan())),
        "send" => send(output, &args),
        "listen" => listen(output, &args),
        other => return output.usage("zone", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail("zone", &message),
    }
}

fn info(output: &Output) -> u8 {
    let station = Station::new();
    output.emit(
        &json!({
            "zone": "lagrange",
            "landmarks": {
                "depot": DEPOT.to_array(),
                "jig": JIG.to_array(),
                "airlock": AIRLOCK.to_array(),
                "spawn": SPAWN.to_array(),
            },
            "limits": {
                "eva_range_m": verse_lagrange::station::EVA_RANGE,
                "grab_range_m": verse_lagrange::station::GRAB_RANGE,
                "latch_range_m": verse_lagrange::station::LATCH_RANGE,
                "latch_speed_m_s": verse_lagrange::station::LATCH_SPEED,
                "speed_limit_m_s": verse_lagrange::station::SPEED_LIMIT,
            },
            "verbs": VERBS,
            "parts": part_json(&station),
            "status": snapshot_json(&station),
        }),
        |value| {
            let mut rows = vec![vec![
                "part".to_owned(),
                "mass".to_owned(),
                "state".to_owned(),
                "slot".to_owned(),
            ]];
            for part in value["parts"].as_array().into_iter().flatten() {
                rows.push(vec![
                    part["kind"].as_str().unwrap_or("").to_owned(),
                    format!("{:.0} kg", part["mass_kg"].as_f64().unwrap_or(0.0)),
                    part["state"].as_str().unwrap_or("").to_owned(),
                    part["slot"].to_string(),
                ]);
            }
            format!(
                "Lagrange 1 construction zone\nlandmarks: {}\n{}\n{}",
                value["landmarks"],
                out::table(&rows),
                render_snapshot(&value["status"])
            )
        },
    );
    0
}

fn build_plan() -> Vec<Verb> {
    let mut plan = Vec::new();
    for kind in PartKind::ALL {
        let stow = beside(kind.stowage());
        plan.push(Verb {
            name: "fly".into(),
            args: stow.to_array().iter().map(|n| Arg::Number(*n)).collect(),
        });
        plan.push(Verb {
            name: "grab".into(),
            args: Vec::new(),
        });
        plan.push(Verb {
            name: "install".into(),
            args: Vec::new(),
        });
    }
    plan
}

fn local(output: &Output, args: &Args, plan: Option<Vec<Verb>>) -> Result<u8, String> {
    let verbs = match plan {
        Some(plan) => plan,
        None => Verb::parse_all(args.positional())?,
    };
    let dt: f64 = args.number("dt", 0.02)?;
    let timeout: f64 = args.number("timeout", 240.0)?;
    let mut sim = Sim::new(dt, timeout);
    let mut steps = Vec::new();
    let mut failed = false;
    for verb in &verbs {
        let outcome = sim.apply(verb);
        let ok = outcome.is_ok();
        let mut step = json!({
            "verb": verb.to_json(),
            "ok": ok,
            "t": sim.elapsed,
        });
        match outcome {
            Ok(result) => step["result"] = result,
            Err(message) => step["error"] = message.into(),
        }
        if args.switch("trace") || !ok {
            step["status"] = snapshot_json(&sim.station);
        }
        if output.json() {
            output.line(&step, |_| String::new());
        } else {
            println!(
                "{:>7.1}s {} {}{}",
                step["t"].as_f64().unwrap_or(0.0),
                verb.name,
                verb.args
                    .iter()
                    .map(|arg| match arg {
                        Arg::Number(n) => format!("{n:.1}"),
                        Arg::Text(text) => text.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(","),
                match (&step["result"], &step["error"]) {
                    (_, Value::String(error)) => format!(" -> error: {error}"),
                    (result, _) if result.get("status").is_some() =>
                        format!("\n  {}", render_snapshot(&result["status"])),
                    (result, _) if result.get("parts").is_some() => format!(
                        "\n{}",
                        result["parts"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|part| format!(
                                "  {:<16} {:<10} {}",
                                part["kind"].as_str().unwrap_or(""),
                                part["state"].as_str().unwrap_or(""),
                                part["pos"]
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    (result, _) => format!(" -> {result}"),
                }
            );
        }
        steps.push(step);
        if !ok {
            failed = true;
            break;
        }
    }
    let final_status = snapshot_json(&sim.station);
    if output.json() {
        output.line(
            &json!({ "done": !failed, "elapsed_s": sim.elapsed, "status": final_status, "parts": part_json(&sim.station) }),
            |_| String::new(),
        );
    } else {
        println!("{}", render_snapshot(&final_status));
    }
    Ok(if failed { crate::EXIT_FAILURE } else { 0 })
}

fn send(output: &Output, args: &Args) -> Result<u8, String> {
    let verbs = Verb::parse_all(args.positional())?;
    let Some(to) = args.option("to") else {
        return Err("--to OPERATOR_PUBKEY is required".into());
    };
    if to.len() != 64 || !to.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("--to takes a 64-hex public key".into());
    }
    let zone = args.option("zone").unwrap_or("lagrange").to_owned();
    let mut context = crate::world::Context::open(args)?;
    let wait = Duration::from_secs(args.number::<u64>("wait", 10)?.max(1));
    let mut results = Vec::new();
    let mut accepted = true;
    for verb in &verbs {
        let id = verse::identity::random_hex(4);
        let command = Wire {
            v: 1,
            zone: zone.clone(),
            cmd: verb.name.clone(),
            args: verb.args.clone(),
            t: unix_now() * 1000,
            id: id.clone(),
        };
        let event = mv::command_event(
            &context.identity.signer,
            &context.world,
            to,
            &command,
            unix_now(),
        );
        let published = context.client.publish(event, DEFAULT_WAIT)?;
        // The operator reports as a gesture to us naming the command id.
        let world = context.world.clone();
        let me = context.pubkey().to_owned();
        let mut report: Option<Value> = None;
        if published.accepted {
            context.client.subscribe(
                vec![json!({
                    "kinds": [mv::GESTURE_KIND],
                    "#w": [world],
                    "#p": [me],
                    "authors": [to],
                })],
                true,
                wait,
                |event| {
                    if report.is_none()
                        && let Ok(Received::Gesture { gesture, .. }) = mv::decode(event, &world)
                        && (gesture.g == "zone-ok" || gesture.g == "zone-refused")
                        && gesture.id == id
                    {
                        report = Some(json!({ "outcome": gesture.g, "at": gesture.at }));
                    }
                },
            )?;
        }
        accepted &= published.accepted;
        let value = json!({
            "verb": verb.to_json(),
            "id": id,
            "event": published.id,
            "relayed": published.accepted,
            "message": published.message,
            "report": report,
        });
        output.line(&value, |value| {
            format!(
                "{} {} {}",
                if value["relayed"].as_bool().unwrap_or(false) {
                    "sent"
                } else {
                    "refused"
                },
                value["verb"]["cmd"].as_str().unwrap_or(""),
                match value["report"]["outcome"].as_str() {
                    Some(outcome) => format!("-> {outcome}"),
                    None => "(no report from operator)".to_owned(),
                }
            )
        });
        results.push(value);
        if !published.accepted {
            break;
        }
    }
    context.client.close();
    Ok(if accepted { 0 } else { crate::EXIT_FAILURE })
}

fn listen(output: &Output, args: &Args) -> Result<u8, String> {
    let wait = Duration::from_secs(args.number::<u64>("wait", 30)?.max(1));
    let mut context = crate::world::Context::open(args)?;
    let world = context.world.clone();
    let me = context.pubkey().to_owned();
    context.client.subscribe(
        vec![json!({ "kinds": [mv::COMMAND_KIND], "#w": [world], "#p": [me] })],
        true,
        wait,
        |event| {
            if let Ok(Received::Command { pubkey, command, .. }) = mv::decode(event, &world) {
                let verb = Verb::from_wire(&command);
                output.line(
                    &json!({ "from": pubkey, "zone": command.zone, "id": command.id, "verb": verb.to_json() }),
                    |value| {
                        format!(
                            "{} {} {}",
                            value["from"].as_str().map(|k| &k[..8]).unwrap_or(""),
                            value["zone"].as_str().unwrap_or(""),
                            value["verb"]
                        )
                    },
                );
            }
        },
    )?;
    context.client.close();
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verbs_with_arguments() {
        let words: Vec<String> = [
            "fly", "-12,-6,4", "grab", "fly", "jig", "release", "wait", "2",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let verbs = Verb::parse_all(&words).unwrap();
        assert_eq!(verbs.len(), 5);
        assert_eq!(verbs[0].args.len(), 3);
        assert_eq!(verbs[2].args, vec![Arg::Text("jig".into())]);
        assert_eq!(verbs[4].args, vec![Arg::Number(2.0)]);
        assert!(Verb::parse_all(&["3".to_owned()]).is_err());
    }

    #[test]
    fn builds_the_keel_headlessly() {
        let mut sim = Sim::new(0.02, 600.0);
        for verb in build_plan() {
            let result = sim.apply(&verb);
            assert!(result.is_ok(), "{verb:?}: {result:?}");
        }
        let snapshot = sim.station.snapshot();
        assert_eq!(snapshot.installed, snapshot.total, "{:?}", snapshot.message);
    }
}
