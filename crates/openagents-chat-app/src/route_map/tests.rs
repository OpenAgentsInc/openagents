use super::layout::{self, Camera, Detail, Direction, Layout, Point};
use super::records::{AdoptionRecord, Records, ReleaseRecord, ResultRecord, Verdict};
use super::sources::{PluginSource, RouteScore, Sources, TestSetSource};
use super::*;
use openagents_connect::control::{EngineAccount, EngineReport, EngineRoute, UsageWindow};

const PUBLISHER: &str = "ab";

fn plugin(dir: &str, slug: Option<&str>, name: &str) -> PluginSource {
    PluginSource {
        dir: dir.into(),
        slug: slug.map(str::to_string),
        name: name.into(),
        summary: format!("{name} does one thing."),
        publisher: slug.map(|_| PUBLISHER.to_string()),
        version: slug.map(|_| "0.1.0".to_string()),
        wasm: vec![format!("{dir}-guest")],
        workflows: vec![name.to_lowercase()],
        skills: vec![],
        knowledge: vec![],
        tests: vec![TestSetSource {
            dir: format!("{dir}/evals"),
            should_fire: 4,
            should_not_fire: 2,
        }],
    }
}

fn result(id: &str, slug: &str, verdict: Verdict, at: u64) -> ResultRecord {
    ResultRecord {
        id: id.into(),
        created_at: at,
        evaluator: "runner".into(),
        trainer: "trainer".into(),
        subject: format!("{PUBLISHER}:{slug}/{slug}"),
        subject_release: None,
        suite_release: "suite".into(),
        verdict,
        with: 5,
        without: Some(2),
        total: 6,
        checks: None,
        validates: None,
        confirmed: 0,
        disputed: 0,
    }
}

fn records(results: Vec<ResultRecord>) -> Records {
    Records {
        schema: records::SCHEMA.into(),
        results,
        ..Records::default()
    }
}

fn with_plugins(plugins: Vec<PluginSource>) -> Sources {
    let mut sources = Sources::committed();
    sources.plugins = plugins;
    sources.showcase = None;
    sources
}

fn kinds(map: &Map) -> Vec<GapKind> {
    map.gaps.iter().map(|gap| gap.kind).collect()
}

fn gap_on(map: &Map, node: &str, kind: GapKind) -> Option<Gap> {
    let node = map.find(node)?;
    map.gaps
        .iter()
        .find(|gap| gap.node == node && gap.kind == kind)
        .cloned()
}

/// The committed snapshot parses, and the map holds every route it lists,
/// in its order, each under its family, plus the front.
#[test]
fn every_route_of_the_router_is_on_the_map() {
    let sources = Sources::committed();
    let records = Records::committed();
    assert_eq!(sources.schema, sources::SCHEMA);
    assert_eq!(records.schema, records::SCHEMA);
    assert!(
        sources.routes.len() >= 20,
        "the router offers 20 routes or more"
    );
    let map = Map::committed();
    let routes: Vec<&str> = map
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Route)
        .map(|n| n.label.as_str())
        .collect();
    let listed: Vec<&str> = sources.routes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(routes, listed);
    for route in &sources.routes {
        let node = map.find(&format!("route:{}", route.id)).unwrap();
        let family = map.nodes[node].parent.unwrap();
        assert_eq!(map.nodes[family].id, format!("family:{}", route.family));
        assert_eq!(
            map.nodes[map.nodes[family].parent.unwrap()].kind,
            Kind::Front
        );
    }
    for word in [
        "meta",
        "work.dispatch",
        "eval.run",
        "presentation.open",
        "capability.missing",
    ] {
        assert!(map.find(&format!("route:{word}")).is_some(), "{word}");
    }
}

/// Members come from the sources: every prepared answer under a route it
/// answers, the product knowledge under product.kb, Coder under
/// work.dispatch with every engine and plugin, decks under
/// presentation.open, and the edges follow a request.
#[test]
fn members_and_edges_follow_a_request() {
    let map = Map::committed();
    let sources = map.sources().clone();
    for answer in &sources.answers {
        let node = map
            .find(&format!("answer:{}", answer.id))
            .expect(&answer.id);
        let parent = &map.nodes[map.nodes[node].parent.unwrap()];
        assert!(answer.routes.contains(&parent.label), "{}", answer.id);
        assert_eq!(map.nodes[node].kind, Kind::Answer);
    }
    let coder = map.find("coder").unwrap();
    assert_eq!(
        map.nodes[map.nodes[coder].parent.unwrap()].id,
        "route:work.dispatch"
    );
    for engine in &sources.engines {
        let node = map.find(&format!("engine:{}", engine.id)).unwrap();
        assert_eq!(map.nodes[node].parent, Some(coder));
        assert!(
            map.edges
                .iter()
                .any(|e| e.from == coder && e.to == node && e.kind == EdgeKind::HandsOff)
        );
    }
    for plugin in &sources.plugins {
        let node = map.find(&format!("plugin:{}", plugin.dir)).unwrap();
        assert_eq!(map.nodes[node].kind, Kind::Plugin);
        assert!(
            map.edges
                .iter()
                .any(|e| e.from == coder && e.to == node && e.kind == EdgeKind::Admits)
        );
    }
    for entry in &sources.knowledge.product {
        assert!(map.find(&format!("knowledge:{}", entry.id)).is_some());
    }
    for deck in &sources.decks {
        let node = map.find(&format!("deck:{}", deck.id)).unwrap();
        assert_eq!(
            map.nodes[map.nodes[node].parent.unwrap()].label,
            "presentation.open"
        );
    }
    let wallet = map
        .find("screen:wallet")
        .expect("the wallet's offer opens a screen");
    assert_eq!(map.nodes[wallet].kind, Kind::Screen);
    assert!(
        map.edges
            .iter()
            .any(|e| e.to == wallet && e.kind == EdgeKind::Opens)
    );
    // Every node's id is unique, and every edge is in range.
    let mut ids: Vec<&str> = map.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), map.nodes.len());
    assert!(
        map.edges
            .iter()
            .all(|e| e.from < map.nodes.len() && e.to < map.nodes.len())
    );
}

/// The hosted runner's sample plugins are never on the map; the other
/// committed plugins keep their real statuses (Outline not packaged).
#[test]
fn the_sample_plugins_are_not_on_the_map() {
    let map = Map::committed();
    for dir in crate::eval_cards::catalog_dirs() {
        assert!(map.find(&format!("plugin:{dir}")).is_none(), "{dir}");
    }
    let stage = |dir: &str| map.nodes[map.find(&format!("plugin:{dir}")).unwrap()].stage;
    assert_eq!(stage("crates/plugin-outline"), Some(Stage::NotPackaged));
}

/// Zero, one, or many plugins: the map builds, Coder holds exactly them,
/// and the showcase is marked only when it is one of them.
#[test]
fn zero_one_or_many_plugins() {
    for count in [0, 1, 7] {
        let plugins: Vec<PluginSource> = (0..count)
            .map(|n| {
                plugin(
                    &format!("crates/plugin-p{n}"),
                    Some(&format!("p{n}")),
                    &format!("P{n}"),
                )
            })
            .collect();
        let mut sources = with_plugins(plugins);
        if count > 0 {
            sources.showcase = Some("crates/plugin-p0".into());
        }
        let map = Map::build(sources, records(vec![]), Local::default());
        let coder = map.find("coder").unwrap();
        let held: Vec<usize> = map
            .children(coder)
            .into_iter()
            .filter(|&c| map.nodes[c].kind == Kind::Plugin)
            .collect();
        assert_eq!(held.len(), count);
        assert_eq!(
            map.nodes.iter().filter(|n| n.showcase).count(),
            count.min(1)
        );
        let layout = Layout::of(&map);
        assert_eq!(layout.positions.len(), map.nodes.len());
        if count > 0 {
            let showcase = map.find("plugin:crates/plugin-p0").unwrap();
            let inspector = map.inspect(showcase);
            assert!(
                inspector
                    .steps
                    .iter()
                    .any(|s| s.label() == "Make one like this"
                        && matches!(s, NextStep::Chat { message, .. } if message.starts_with("Help me make a plugin like P0")))
            );
        }
    }
}

/// The ladder, from records: candidate, a result, reproduced by a check,
/// validated on a second test set, adopted.
#[test]
fn the_plugin_ladder_reads_the_records() {
    let p = plugin("crates/plugin-x", Some("x"), "X");
    let none = plugin("crates/plugin-y", None, "Y");
    assert_eq!(stage_of(&none, &records(vec![])), Stage::NotPackaged);
    assert_eq!(stage_of(&p, &records(vec![])), Stage::Candidate);
    let original = result("r1", "x", Verdict::Better, 10);
    assert_eq!(
        stage_of(&p, &records(vec![original.clone()])),
        Stage::Result(Verdict::Better)
    );
    assert_eq!(
        stage_of(&p, &records(vec![result("r0", "x", Verdict::Worse, 9)])),
        Stage::Result(Verdict::Worse)
    );
    let mut confirmed = original.clone();
    confirmed.confirmed = 3;
    assert_eq!(
        stage_of(&p, &records(vec![confirmed.clone()])),
        Stage::Reproduced
    );
    let mut validation = result("v1", "x", Verdict::Better, 11);
    validation.validates = Some("r1".into());
    assert_eq!(
        stage_of(&p, &records(vec![validation.clone(), confirmed.clone()])),
        Stage::Validated
    );
    let mut adopted = records(vec![validation, confirmed]);
    adopted.releases.push(ReleaseRecord {
        id: "rel".into(),
        pubkey: PUBLISHER.into(),
        package: format!("{PUBLISHER}:x"),
        version: "0.1.0".into(),
        created_at: 1,
    });
    adopted.adoptions.push(AdoptionRecord {
        defaults_release: "defaults".into(),
        release: "rel".into(),
        admission: "sha256:00".into(),
        at: 12,
    });
    assert_eq!(stage_of(&p, &adopted), Stage::Adopted);
    // Another publisher's result of the same slug isn't this plugin's.
    let mut foreign = result("f", "x", Verdict::Better, 13);
    foreign.subject = "cd:x/x".into();
    assert_eq!(stage_of(&p, &records(vec![foreign])), Stage::Candidate);
}

/// Each plugin gap: no package, no result, needs a check, needs a
/// validation, ready to adopt, and didn't help, with its next step.
#[test]
fn plugin_gaps_and_their_next_steps() {
    let plugins = vec![
        plugin("crates/plugin-a", None, "A"),
        plugin("crates/plugin-b", Some("b"), "B"),
        plugin("crates/plugin-c", Some("c"), "C"),
        plugin("crates/plugin-d", Some("d"), "D"),
        plugin("crates/plugin-e", Some("e"), "E"),
        plugin("crates/plugin-f", Some("f"), "F"),
    ];
    let mut d = result("d1", "d", Verdict::Better, 3);
    d.confirmed = 1;
    let mut e = result("e1", "e", Verdict::Better, 4);
    e.confirmed = 3;
    let mut ev = result("e2", "e", Verdict::Better, 5);
    ev.validates = Some("e1".into());
    let map = Map::build(
        with_plugins(plugins),
        records(vec![
            result("c1", "c", Verdict::Better, 2),
            d,
            ev,
            e,
            result("f1", "f", Verdict::NoClearChange, 6),
        ]),
        Local::default(),
    );
    let on = |dir: &str, kind| gap_on(&map, &format!("plugin:crates/plugin-{dir}"), kind);
    let a = on("a", GapKind::NotPackaged).unwrap();
    assert!(
        matches!(&a.step, NextStep::Command { command, .. } if command == "openagents ext eval init crates/plugin-a")
    );
    let b = on("b", GapKind::NoResult).unwrap();
    assert!(
        matches!(&b.step, NextStep::Command { command, .. } if command.starts_with("openagents ext eval run crates/plugin-b"))
    );
    let c = on("c", GapKind::NeedsCheck).unwrap();
    assert!(
        matches!(&c.step, NextStep::Chat { message, .. } if message == "Check someone's C result")
    );
    assert!(
        c.evidence
            .iter()
            .any(|l| l.target == Target::Event("c1".into()))
    );
    let d = on("d", GapKind::NeedsValidation).unwrap();
    assert!(
        matches!(&d.step, NextStep::Chat { message, .. } if message == "Help me write a second test set for D")
    );
    assert!(on("e", GapKind::ReadyToAdopt).is_some());
    let f = on("f", GapKind::NoHelpYet).unwrap();
    assert!(f.detail.contains("No clear change"));
}

/// No plugin is in the chat's catalog (no product note is a tool), so a
/// plugin with no result is tested on a computer, never offered in chat.
#[test]
fn a_plugin_without_a_result_is_tested_on_a_computer() {
    let p = plugin("crates/plugin-p0", Some("p0"), "P0");
    let map = Map::build(with_plugins(vec![p]), records(vec![]), Local::default());
    let gap = gap_on(&map, "plugin:crates/plugin-p0", GapKind::NoResult).unwrap();
    assert!(
        matches!(&gap.step, NextStep::Command { label, .. } if label == "Run its tests"),
        "{:?}",
        gap.step
    );
}

/// Route gaps: nothing serves `capability.missing`, product questions with
/// no entry, a weak route, few examples, a route the record doesn't
/// measure, a route only the model answers, and frequent clarify from
/// this person's own counts.
#[test]
fn route_gaps_from_the_sources() {
    let map = Map::committed();
    let missing = gap_on(&map, "route:capability.missing", GapKind::NoPlugin).unwrap();
    assert!(
        matches!(&missing.step, NextStep::Chat { message, .. } if message == "Help me make a plugin that ")
    );
    assert!(missing.detail.contains("labeled examples land here"));
    let unanswered = gap_on(&map, "route:product.kb", GapKind::UnansweredQuestions).unwrap();
    assert!(
        matches!(&unanswered.step, NextStep::Command { command, .. } if command.starts_with("microcoder kb add"))
    );
    // Every route is in the committed per-route record except
    // `standing.rule` (#10157), which the router gained after the
    // 2026-10-01 record; the map names it until a new record measures it.
    let unmeasured: Vec<&str> = map
        .gaps
        .iter()
        .filter(|gap| gap.kind == GapKind::RouteUnmeasured)
        .map(|gap| map.nodes[gap.node].id.as_str())
        .collect();
    assert_eq!(unmeasured, ["route:standing.rule"]);

    let mut sources = Sources::committed();
    sources.measurement.routes.insert(
        "meta".into(),
        RouteScore {
            precision: 0.6,
            predicted: 10,
            recall: 0.95,
            labeled: 20,
        },
    );
    sources.labeled.routes.get_mut("end").unwrap().rows = 3;
    sources.measurement.routes.remove("presentation.open");
    sources
        .answers
        .retain(|a| !a.routes.iter().any(|r| r == "smalltalk"));
    let mut local = Local::default();
    local.routes.insert("clarify".into(), 8);
    local.routes.insert("meta".into(), 22);
    local.routes.insert("capability.missing".into(), 2);
    let map = Map::build(sources, records(vec![]), local);
    let weak = gap_on(&map, "route:meta", GapKind::WeakRoute).unwrap();
    assert!(weak.detail.contains("Precision 60%"));
    assert!(
        matches!(&weak.step, NextStep::Issue { url, .. } if url.starts_with(REPOSITORY) && url.contains("title=Router%3A%20labeled%20examples%20for%20meta"))
    );
    assert_eq!(
        map.nodes[map.find("route:meta").unwrap()].health,
        Health::Weak
    );
    assert!(gap_on(&map, "route:end", GapKind::FewExamples).is_some());
    assert!(gap_on(&map, "route:presentation.open", GapKind::RouteUnmeasured).is_some());
    assert!(gap_on(&map, "route:smalltalk", GapKind::ModelOnly).is_some());
    let clarify = gap_on(&map, "route:clarify", GapKind::FrequentClarify).unwrap();
    assert!(clarify.detail.contains("8 of your 32 replies"));
    let missing = gap_on(&map, "route:capability.missing", GapKind::NoPlugin).unwrap();
    assert!(missing.detail.contains("2 of your own replies"));
}

fn report(routes: Vec<EngineRoute>, accounts: Vec<EngineAccount>) -> EngineReport {
    EngineReport {
        enabled: true,
        adapter: "test".into(),
        model: "m".into(),
        routes,
        accounts,
        usage_probe: None,
        refresh_due: false,
    }
}

/// Engines: signed out and at the limit are gaps with Settings as the
/// next step; with no reading, nothing is claimed.
#[test]
fn engine_readiness_from_this_computer() {
    let map = Map::committed();
    assert!(!kinds(&map).contains(&GapKind::EngineSignedOut));
    let codex = map.find("engine:codex").unwrap();
    assert_eq!(map.nodes[codex].health, Health::Unmeasured);
    let local = Local {
        routes: BTreeMap::new(),
        engines: Some(report(
            vec![
                EngineRoute {
                    provider: "codex".into(),
                    name: "Codex".into(),
                    model: "gpt".into(),
                    signed_in: true,
                    usage: RouteUsage::Windows {
                        windows: vec![UsageWindow {
                            name: "five_hour".into(),
                            label: "5 hours".into(),
                            used_percent: 100,
                            resets_at: None,
                            resets: None,
                        }],
                        limit_reached: true,
                        used_percent: 100,
                    },
                },
                EngineRoute {
                    provider: "claude".into(),
                    name: "Claude Code".into(),
                    model: "opus".into(),
                    signed_in: true,
                    usage: RouteUsage::Off,
                },
            ],
            vec![EngineAccount {
                provider: "devin".into(),
                name: "Devin".into(),
                signed_in: false,
            }],
        )),
    };
    let map = Map::build(Sources::committed(), Records::committed(), local);
    let codex = map.find("engine:codex").unwrap();
    assert_eq!(
        map.evidence(codex).as_deref(),
        Some("Temporarily unavailable")
    );
    let inspector = map.inspect(codex);
    assert!(
        inspector
            .fields
            .iter()
            .any(|field| field.label == "On this computer"
                && field.value == "Temporarily unavailable; another engine runs")
    );
    let limit = gap_on(&map, "engine:codex", GapKind::EngineAtLimit).unwrap();
    assert_eq!(
        limit.step,
        NextStep::SignIn {
            label: "See usage".into(),
            engine: "codex".into()
        }
    );
    let devin = gap_on(&map, "engine:devin", GapKind::EngineSignedOut).unwrap();
    assert_eq!(devin.step.label(), "Sign in Devin");
    let claude = map.find("engine:claude_code").unwrap();
    assert_eq!(map.nodes[claude].health, Health::Good);
    assert!(map.nodes[claude].gaps.is_empty());
}

/// The inspector: a route says why the router sends things there and its
/// numbers with their records; a plugin its parts, results, and adoption;
/// Coder its engines and plugins; local counts say they stayed here.
#[test]
fn the_inspector_shows_why_numbers_and_records() {
    let mut local = Local::default();
    local.routes.insert("work.dispatch".into(), 5);
    let map = Map::build(Sources::committed(), Records::committed(), local);
    let dispatch = map.inspect(map.find("route:work.dispatch").unwrap());
    assert!(dispatch.why.as_deref().is_some_and(|w| !w.is_empty()));
    let precision = dispatch
        .fields
        .iter()
        .find(|f| f.label == "Held-out precision")
        .unwrap();
    assert!(precision.value.contains("read as work.dispatch"));
    assert!(
        matches!(&precision.link, Some(Link { target: Target::Path(p), .. }) if p.ends_with("report.json"))
    );
    assert!(
        dispatch
            .fields
            .iter()
            .any(|f| f.value.contains("counted on this computer"))
    );
    assert!(dispatch.members.iter().any(|(_, label)| label == "Coder"));

    let map_plugin = map.inspect(map.find("plugin:crates/plugin-outline").unwrap());
    let labels: Vec<&str> = map_plugin.fields.iter().map(|f| f.label.as_str()).collect();
    assert!(labels.contains(&"Where"), "{labels:?}");
    // Nothing on the map is adopted, so Coder shows no empty adopted field.
    let coder = map.inspect(map.find("coder").unwrap());
    assert!(
        !coder
            .fields
            .iter()
            .any(|f| f.label == "Adopted into everyone's Coder")
    );
    let missing = map.inspect(map.find("route:capability.missing").unwrap());
    assert!(
        missing
            .steps
            .iter()
            .any(|s| s.label() == "Draft a plugin in chat")
    );
    assert!(!missing.gaps.is_empty());
}

/// A screen reader hears each node's kind, name, state, and gap count.
#[test]
fn accessible_names_say_kind_state_and_gaps() {
    let map = Map::committed();
    let missing = map.find("route:capability.missing").unwrap();
    let name = map.accessible_name(missing);
    assert!(name.starts_with("Route: capability.missing"), "{name}");
    assert!(name.contains("gap"), "{name}");
    let outline_plugin = map.accessible_name(map.find("plugin:crates/plugin-outline").unwrap());
    assert!(
        outline_plugin.starts_with("Plugin: Outline, Not packaged"),
        "{outline_plugin}"
    );
    assert_eq!(
        map.accessible_name(map.find("coder").unwrap()),
        "Coder: Coder"
    );
    for node in 0..map.nodes.len() {
        assert!(!map.accessible_name(node).is_empty());
    }
    // The outline lists every node once, parents before children.
    let outline = map.outline();
    assert_eq!(outline.len(), map.nodes.len());
    for (at, &node) in outline.iter().enumerate() {
        if let Some(parent) = map.nodes[node].parent {
            assert!(outline[..at].contains(&parent));
        }
    }
}

/// Filters: a family, kinds, gaps only, unmeasured only.
#[test]
fn filters_narrow_nodes_and_gaps() {
    let map = Map::committed();
    let gym = Filter {
        family: Some("gym".into()),
        ..Filter::default()
    };
    assert!(gym.admits(&map, map.find("route:eval.run").unwrap()));
    assert!(!gym.admits(&map, map.find("route:meta").unwrap()));
    let plugins = Filter {
        kinds: vec![Kind::Plugin],
        ..Filter::default()
    };
    assert!(plugins.admits(&map, map.find("plugin:crates/plugin-outline").unwrap()));
    assert!(!plugins.admits(&map, map.find("coder").unwrap()));
    let gaps = Filter {
        gaps_only: true,
        ..Filter::default()
    };
    let listed = map.gaps_where(&gaps);
    assert_eq!(listed.len(), map.gaps.len());
    assert!(
        listed
            .iter()
            .all(|&g| !map.nodes[map.gaps[g].node].gaps.is_empty())
    );
    let unmeasured = Filter {
        unmeasured_only: true,
        ..Filter::default()
    };
    assert!(unmeasured.admits(&map, map.find("plugin:crates/plugin-outline").unwrap()));
    assert!(!unmeasured.admits(&map, map.find("route:meta").unwrap()));
    assert!(Filter::default().is_clear());
}

/// Positions depend only on the tree: local counts, engine readings, and
/// records move nothing. Nodes on one ring don't overlap.
#[test]
fn positions_hold_across_refreshes_and_rings_do_not_overlap() {
    let before = Layout::of(&Map::committed());
    let mut local = Local::default();
    local.routes.insert("meta".into(), 40);
    let after = Layout::of(&Map::build(Sources::committed(), records(vec![]), local));
    assert_eq!(before.positions, after.positions);
    let map = Map::committed();
    for a in 0..map.nodes.len() {
        for b in (a + 1)..map.nodes.len() {
            if map.nodes[a].depth != map.nodes[b].depth || map.nodes[a].depth == 0 {
                continue;
            }
            let gap = before.positions[a].distance(before.positions[b]);
            assert!(
                gap >= before.radii[a] + before.radii[b],
                "{} and {} overlap ({gap})",
                map.nodes[a].id,
                map.nodes[b].id
            );
        }
    }
    assert_eq!(
        before.positions[map.find("front").unwrap()],
        Point::default()
    );
}

/// Source growth must not crowd circles, even after weights reach their maximum.
#[test]
fn dense_tree_keeps_all_circles_apart_across_weight_refreshes() {
    let mut map = Map::committed();
    let parent = map.find("route:meta").unwrap();
    let template = map
        .nodes
        .iter()
        .find(|node| node.kind == Kind::Answer)
        .unwrap()
        .clone();
    for index in 0..100 {
        let mut node = template.clone();
        node.id = format!("answer:dense-{index}");
        node.parent = Some(parent);
        node.depth = map.nodes[parent].depth + 1;
        map.nodes.push(node);
    }
    let before = Layout::of(&map);
    for node in &mut map.nodes {
        node.weight = 1.0;
    }
    let after = Layout::of(&map);
    assert_eq!(before.positions, after.positions);
    assert_eq!(before.spans, after.spans);
    for a in 0..map.nodes.len() {
        for b in (a + 1)..map.nodes.len() {
            let distance = after.positions[a].distance(after.positions[b]);
            assert!(
                distance >= after.radii[a] + after.radii[b],
                "{} and {} overlap ({distance})",
                map.nodes[a].id,
                map.nodes[b].id
            );
        }
    }
}

/// The camera: screen and world round-trip, panning moves by the drag,
/// zooming keeps the anchor still and stays in bounds, fit holds every
/// node, and easing between cameras ends where it should.
#[test]
fn camera_transform_math() {
    let mut camera = Camera {
        center: Point::new(100.0, -50.0),
        zoom: 2.0,
    };
    let (w, h) = (800.0, 600.0);
    let world = Point::new(130.0, -20.0);
    let screen = camera.to_screen(world, w, h);
    assert_eq!(screen, Point::new(460.0, 360.0));
    let back = camera.to_world(screen, w, h);
    assert!(back.distance(world) < 1e-3);
    camera.pan(40.0, -20.0);
    assert_eq!(camera.center, Point::new(80.0, -40.0));
    let anchor = Point::new(200.0, 150.0);
    let under = camera.to_world(anchor, w, h);
    camera.zoom_at(layout::ZOOM_STEP, anchor, w, h);
    assert!((camera.zoom - 2.5).abs() < 1e-5);
    assert!(camera.to_world(anchor, w, h).distance(under) < 1e-3);
    camera.zoom_at(1000.0, anchor, w, h);
    assert_eq!(camera.zoom, layout::MAX_ZOOM);
    camera.zoom_at(1e-6, anchor, w, h);
    assert_eq!(camera.zoom, layout::MIN_ZOOM);

    let map = Map::committed();
    let layout = Layout::of(&map);
    let fit = Camera::fit(layout.bounds(), w, h, 24.0);
    for (p, r) in layout.positions.iter().zip(&layout.radii) {
        let s = fit.to_screen(*p, w, h);
        assert!(s.x + r * fit.zoom >= 23.0 && s.x - r * fit.zoom <= w - 23.0);
        assert!(s.y + r * fit.zoom >= 23.0 && s.y - r * fit.zoom <= h - 23.0);
    }
    // The minimum window's map (about 300 by 400 points) still fits it all.
    let small = Camera::fit(layout.bounds(), 300.0, 400.0, 12.0);
    for (p, r) in layout.positions.iter().zip(&layout.radii) {
        let s = small.to_screen(*p, 300.0, 400.0);
        assert!(s.x + r * small.zoom >= 11.0 && s.x - r * small.zoom <= 289.0);
    }
    let to = Camera {
        center: Point::new(10.0, 10.0),
        zoom: 3.0,
    };
    assert_eq!(fit.toward(&to, 0.0), fit);
    let end = fit.toward(&to, 1.0);
    assert!(end.center.distance(to.center) < 1e-3 && (end.zoom - 3.0).abs() < 1e-4);
}

/// Hit-testing finds the node under the pointer, the nearest of two, and
/// nothing in empty space or for a hidden node; small nodes keep a
/// 6-point target.
#[test]
fn hit_testing() {
    let map = Map::committed();
    let layout = Layout::of(&map);
    let (w, h) = (1000.0, 800.0);
    let camera = Camera::fit(layout.bounds(), w, h, 24.0);
    let all = |_: usize| true;
    let coder = map.find("coder").unwrap();
    let at = camera.to_screen(layout.positions[coder], w, h);
    assert_eq!(layout::hit(&layout, &camera, at, w, h, &all), Some(coder));
    let front = camera.to_screen(Point::default(), w, h);
    assert_eq!(
        layout::hit(&layout, &camera, front, w, h, &all),
        map.find("front")
    );
    // Off the map entirely.
    assert_eq!(
        layout::hit(&layout, &camera, Point::new(-500.0, -500.0), w, h, &all),
        None
    );
    // A hidden node can't be hit.
    let none = |i: usize| i != coder;
    assert_ne!(layout::hit(&layout, &camera, at, w, h, &none), Some(coder));
    // Zoomed far out, a small answer still takes a 6-point target.
    let answer = map.find("answer:meta.who").unwrap();
    let far = Camera {
        center: Point::default(),
        zoom: layout::MIN_ZOOM,
    };
    let p = far.to_screen(layout.positions[answer], w, h);
    let near = Point::new(p.x + 5.0, p.y);
    assert!(layout::hit(&layout, &far, near, w, h, &all).is_some());
}

/// Labels thin out as the map zooms out: families always, routes from mid
/// zoom, members closer, and evidence lines only zoomed in.
#[test]
fn level_of_detail_labels() {
    assert_eq!(layout::detail(Kind::Family, 1, 0.12), Detail::Name);
    assert_eq!(layout::detail(Kind::Route, 2, 0.2), Detail::None);
    assert_eq!(layout::detail(Kind::Route, 2, 0.6), Detail::Name);
    assert_eq!(layout::detail(Kind::Route, 2, 2.0), Detail::Evidence);
    assert_eq!(layout::detail(Kind::Answer, 3, 0.6), Detail::None);
    assert_eq!(layout::detail(Kind::Answer, 3, 1.0), Detail::Name);
    assert_eq!(layout::detail(Kind::Answer, 3, 4.0), Detail::Name);
    assert_eq!(layout::detail(Kind::Knowledge, 4, 1.0), Detail::None);
    assert_eq!(layout::detail(Kind::Plugin, 4, 2.0), Detail::Evidence);
}

/// Arrow keys move to the nearest node in that direction.
#[test]
fn arrow_keys_step_between_nodes() {
    let map = Map::committed();
    let layout = Layout::of(&map);
    let all = |_: usize| true;
    let front = map.find("front").unwrap();
    for direction in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        let to = layout::step(&layout, front, direction, &all).expect("a node that way");
        let (from, at) = (layout.positions[front], layout.positions[to]);
        match direction {
            Direction::Left => assert!(at.x < from.x),
            Direction::Right => assert!(at.x > from.x),
            Direction::Up => assert!(at.y < from.y),
            Direction::Down => assert!(at.y > from.y),
        }
    }
}

/// Record links open where a person reads them.
#[test]
fn links_open_on_github_or_njump() {
    assert_eq!(
        Target::Path("docs/x.md".into()).url(),
        format!("{REPOSITORY}/blob/main/docs/x.md")
    );
    assert_eq!(Target::Event("ab".into()).url(), "https://njump.me/ab");
    assert!(examples_issue("meta").contains("body="));
}

#[test]
#[ignore = "prints the committed map's gaps"]
fn print_gaps() {
    let map = Map::committed();
    println!(
        "{} nodes, {} edges, {} gaps",
        map.nodes.len(),
        map.edges.len(),
        map.gaps.len()
    );
    for gap in &map.gaps {
        println!(
            "{:?} {} | {} | {}",
            gap.kind,
            map.nodes[gap.node].id,
            gap.title,
            gap.step.label()
        );
    }
}
