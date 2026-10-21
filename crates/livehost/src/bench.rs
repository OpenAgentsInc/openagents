//! `livehost bench`: edits a copy of the demo sites, deploys each change,
//! and times it until the host serves it and a connected browser shows it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use nostr::domain::RelaySigner;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Notify;

use crate::deploy::{self, Report, Target};
use crate::load::{self, LoadReport, LoadTarget, percentile};
use crate::ms_since;
use crate::store::Store;

pub struct Bench {
    /// The host's address, such as `127.0.0.1:8090`.
    pub host: String,
    pub store: Store,
    pub relay: String,
    pub channel: String,
    pub site: PathBuf,
    pub site_key: RelaySigner,
    pub game: Option<(PathBuf, RelaySigner)>,
    /// The host name that reaches the game tenant, such as `game.localhost`.
    pub game_host: String,
    pub runs: usize,
    pub cases: Vec<String>,
    /// A Chrome DevTools port; the bench opens the pages there.
    pub cdp_port: Option<u16>,
    /// Concurrency and seconds for the load test during swaps.
    pub load: Option<(usize, u64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub case: String,
    pub run: usize,
    pub tiers: Vec<String>,
    pub phases: Vec<(String, f64)>,
    pub deploy_ms: f64,
    pub host_ms: f64,
    pub browser_ms: Option<f64>,
    pub browser_kind: Option<String>,
    /// For the game case: whether the site tenant kept its manifest.
    pub other_tenant_unchanged: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Results {
    pub runs: Vec<Run>,
    pub load_baseline: Option<LoadReport>,
    pub load_during_swaps: Option<(LoadReport, usize)>,
}

struct Seen {
    kind: &'static str,
    value: Value,
    at: Instant,
}

#[derive(Default)]
struct Watcher {
    seen: Mutex<Vec<Seen>>,
    notify: Notify,
}

impl Watcher {
    /// Waits for a message seen at or after `since` that `matches`.
    async fn wait(
        &self,
        since: Instant,
        timeout: Duration,
        matches: impl Fn(&Seen) -> bool,
    ) -> Result<(Instant, Value)> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(seen) = self
                .seen
                .lock()
                .expect("seen")
                .iter()
                .find(|seen| seen.at >= since && matches(seen))
            {
                return Ok((seen.at, seen.value.clone()));
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                bail!("timed out");
            }
        }
    }
}

async fn follow_sse(url: String, host: Option<String>, kind: &'static str, watcher: Arc<Watcher>) {
    let client = reqwest::Client::new();
    loop {
        let mut request = client.get(&url);
        if let Some(host) = &host {
            request = request.header(reqwest::header::HOST, host);
        }
        if let Ok(mut response) = request.send().await {
            let mut buffer = String::new();
            while let Ok(Some(chunk)) = response.chunk().await {
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(end) = buffer.find("\n\n") {
                    let message: String = buffer.drain(..end + 2).collect();
                    for line in message.lines() {
                        if let Some(data) = line.strip_prefix("data:")
                            && let Ok(value) = serde_json::from_str(data.trim())
                        {
                            let at = Instant::now();
                            watcher
                                .seen
                                .lock()
                                .expect("seen")
                                .push(Seen { kind, value, at });
                            watcher.notify.notify_waiters();
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

struct Case {
    name: &'static str,
    game: bool,
    file: &'static str,
    contains: &'static str,
    line: fn(u64) -> String,
    path: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "css",
        game: false,
        file: "static/style.css",
        contains: "--accent:",
        line: |n| format!("  --accent: hsl({}, 70%, 45%);", n % 360),
        path: "/",
    },
    Case {
        name: "copy",
        game: false,
        file: "content/index.json",
        contains: "\"tagline\"",
        line: |n| format!("  \"tagline\": \"Edited copy, revision {n}.\","),
        path: "/",
    },
    Case {
        name: "template",
        game: false,
        file: "templates/index.html",
        contains: "data-bench=\"template\"",
        line: |n| format!("    <p data-bench=\"template\">Template revision {n}.</p>"),
        path: "/",
    },
    Case {
        name: "module",
        game: false,
        file: "../../guests/module/src/lib.rs",
        contains: "const STAMP",
        line: |n| format!("const STAMP: &str = \"module-{n}\";"),
        path: "/",
    },
    Case {
        name: "handler",
        game: false,
        file: "../../guests/handler/src/lib.rs",
        contains: "const STAMP",
        line: |n| format!("const STAMP: &str = \"handler-{n}\";"),
        path: "/hello",
    },
    Case {
        name: "game-asset",
        game: true,
        file: "assets/sprite.svg",
        contains: "data-bench=\"fill\"",
        line: |n| {
            format!(
                "  <circle data-bench=\"fill\" cx=\"32\" cy=\"32\" r=\"28\" fill=\"hsl({}, 80%, 55%)\"/>",
                n % 360
            )
        },
        path: "/",
    },
];

fn edit(root: &Path, case: &Case, n: u64) -> Result<()> {
    let path = root.join(case.file);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut found = false;
    let edited: Vec<String> = text
        .lines()
        .map(|line| {
            if !found && line.contains(case.contains) {
                found = true;
                (case.line)(n)
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !found {
        bail!("{} has no line with {}", path.display(), case.contains);
    }
    std::fs::write(&path, edited.join("\n") + "\n")?;
    Ok(())
}

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Bench {
    fn target<'a>(&'a self, signer: &'a RelaySigner) -> Target<'a> {
        Target {
            store: &self.store,
            relay: &self.relay,
            signer,
            channel: &self.channel,
        }
    }

    async fn manifest_of(&self, tenant: &str) -> Result<Option<String>> {
        let state: Value = reqwest::get(format!("http://{}/_livehost/state", self.host))
            .await?
            .json()
            .await?;
        Ok(state["tenants"][tenant]["manifest"]
            .as_str()
            .map(str::to_owned))
    }

    pub async fn run(&self) -> Result<Results> {
        let watcher = Arc::new(Watcher::default());
        let base = format!("http://{}", self.host);
        tokio::spawn(follow_sse(
            format!("{base}/_livehost/acks"),
            None,
            "ack",
            watcher.clone(),
        ));
        tokio::spawn(follow_sse(
            format!("{base}/_livehost/events"),
            None,
            "site",
            watcher.clone(),
        ));
        let port = self.host.rsplit(':').next().unwrap_or("80");
        if self.game.is_some() {
            tokio::spawn(follow_sse(
                format!("{base}/_livehost/events"),
                Some(format!("{}:{port}", self.game_host)),
                "game",
                watcher.clone(),
            ));
        }

        // Make sure each tenant has a deploy, then open the pages.
        let report = deploy::deploy(
            &self.site,
            &self.target(&self.site_key),
            &Default::default(),
        )
        .await?;
        report.print();
        if let Some((game, key)) = &self.game {
            deploy::deploy(game, &self.target(key), &Default::default())
                .await?
                .print();
        }
        let wait = Duration::from_secs(120);
        if let Some(cdp) = self.cdp_port {
            let mut pages = vec![format!("{base}/"), format!("{base}/hello?name=bench")];
            if self.game.is_some() {
                pages.push(format!("http://{}:{port}/", self.game_host));
            }
            let client = reqwest::Client::new();
            let opened = Instant::now();
            for page in &pages {
                client
                    .put(format!("http://127.0.0.1:{cdp}/json/new?{page}"))
                    .send()
                    .await?
                    .error_for_status()
                    .context("opening a page in the browser")?;
            }
            watcher
                .wait(opened, wait, |seen| {
                    seen.kind == "ack" && seen.value["path"] == "/hello"
                })
                .await
                .context("the browser never loaded /hello")?;
        }

        let mut runs = Vec::new();
        for case_name in &self.cases {
            if case_name == "rollback" {
                for run in 0..self.runs {
                    runs.push(self.rollback(&watcher, run).await?);
                }
                continue;
            }
            let case = CASES
                .iter()
                .find(|case| case.name == case_name)
                .with_context(|| format!("no bench case {case_name}"))?;
            // One unmeasured run warms the build for this case.
            for run in 0..=self.runs {
                let result = self.measure(&watcher, case, run).await?;
                if run > 0 {
                    runs.push(result);
                }
            }
        }

        let (load_baseline, load_during_swaps) = match self.load {
            Some((concurrency, seconds)) => {
                let (baseline, during) = self.load(&watcher, concurrency, seconds).await?;
                (Some(baseline), Some(during))
            }
            None => (None, None),
        };
        Ok(Results {
            runs,
            load_baseline,
            load_during_swaps,
        })
    }

    async fn measure(&self, watcher: &Watcher, case: &Case, run: usize) -> Result<Run> {
        let (root, key, tenant) = match (&self.game, case.game) {
            (Some((game, key)), true) => (game, key, "game"),
            (None, true) => bail!("the {} case needs --game", case.name),
            _ => (&self.site, &self.site_key, "site"),
        };
        let other_before = if case.game {
            self.manifest_of("site").await?
        } else {
            None
        };
        let started = Instant::now();
        edit(root, case, stamp())?;
        let report = deploy::deploy(root, &self.target(key), &Default::default()).await?;
        let result = self
            .observe(watcher, case.name, run, tenant, case.path, started, report)
            .await?;
        let other_tenant_unchanged = if case.game {
            Some(self.manifest_of("site").await? == other_before)
        } else {
            None
        };
        Ok(Run {
            other_tenant_unchanged,
            ..result
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn observe(
        &self,
        watcher: &Watcher,
        case: &str,
        run: usize,
        tenant: &'static str,
        path: &str,
        started: Instant,
        report: Report,
    ) -> Result<Run> {
        report.print();
        let manifest = report
            .manifest
            .clone()
            .context("the edit changed nothing")?;
        let wait = Duration::from_secs(120);
        let (host_at, _) = watcher
            .wait(started, wait, |seen| {
                seen.kind == tenant && seen.value["manifest"] == manifest.as_str()
            })
            .await
            .with_context(|| format!("{case}: the host never served {manifest}"))?;
        let browser = if self.cdp_port.is_some() {
            let (at, value) = watcher
                .wait(started, wait, |seen| {
                    seen.kind == "ack"
                        && seen.value["tenant"] == tenant
                        && seen.value["path"] == path
                        && seen.value["manifest"] == manifest.as_str()
                })
                .await
                .with_context(|| format!("{case}: the browser never showed {manifest}"))?;
            Some((at, value["kind"].as_str().unwrap_or("").to_owned()))
        } else {
            None
        };
        // Let the other open pages settle before the next edit.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let since = |at: Instant| at.duration_since(started).as_secs_f64() * 1000.0;
        Ok(Run {
            case: case.to_owned(),
            run,
            tiers: report.tiers.clone(),
            phases: report.phases.clone(),
            deploy_ms: report.total_ms,
            host_ms: since(host_at),
            browser_ms: browser.as_ref().map(|(at, _)| since(*at)),
            browser_kind: browser.map(|(_, kind)| kind),
            other_tenant_unchanged: None,
        })
    }

    async fn rollback(&self, watcher: &Watcher, run: usize) -> Result<Run> {
        // Deploy a copy change, then time rolling it back.
        let copy = CASES
            .iter()
            .find(|case| case.name == "copy")
            .expect("copy case");
        let measured = self.measure(watcher, copy, run).await?;
        let _ = measured;
        let started = Instant::now();
        let report = deploy::rollback("site", &self.target(&self.site_key), None).await?;
        self.observe(watcher, "rollback", run, "site", "/", started, report)
            .await
    }

    async fn load(
        &self,
        watcher: &Watcher,
        concurrency: usize,
        seconds: u64,
    ) -> Result<(LoadReport, (LoadReport, usize))> {
        let base = format!("http://{}", self.host);
        let port = self.host.rsplit(':').next().unwrap_or("80");
        let mut targets = vec![
            LoadTarget {
                url: format!("{base}/"),
                host: None,
            },
            LoadTarget {
                url: format!("{base}/hello?name=load"),
                host: None,
            },
        ];
        if self.game.is_some() {
            targets.push(LoadTarget {
                url: format!("{base}/"),
                host: Some(format!("{}:{port}", self.game_host)),
            });
        }
        let duration = Duration::from_secs(seconds);
        let baseline = load::run(targets.clone(), concurrency, duration).await?;
        eprintln!("livehost bench: baseline {baseline:?}");

        let during = tokio::spawn(load::run(targets, concurrency, duration));
        let started = Instant::now();
        let mut swaps = 0;
        let order = ["copy", "css", "handler", "copy", "template", "css"];
        let mut index = 0;
        while started.elapsed() + Duration::from_secs(3) < duration {
            let case = CASES
                .iter()
                .find(|case| case.name == order[index % order.len()])
                .expect("case");
            index += 1;
            let at = Instant::now();
            edit(&self.site, case, stamp())?;
            let report = deploy::deploy(
                &self.site,
                &self.target(&self.site_key),
                &Default::default(),
            )
            .await?;
            let manifest = report.manifest.clone().context("no change")?;
            watcher
                .wait(at, Duration::from_secs(60), |seen| {
                    seen.kind == "site" && seen.value["manifest"] == manifest.as_str()
                })
                .await?;
            swaps += 1;
            eprintln!(
                "livehost bench: swap {swaps} ({}) live in {:.0} ms",
                case.name,
                ms_since(at)
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let during = during.await??;
        Ok((baseline, (during, swaps)))
    }
}

/// A Markdown table of medians and worst cases per case.
pub fn summary(results: &Results) -> String {
    let mut out = String::from(
        "| Case | Runs | Deploy command, median / worst | Edit to host serving, median / worst | Edit to browser updated, median / worst | Browser update |\n\
         | --- | --- | --- | --- | --- | --- |\n",
    );
    let mut cases: Vec<&str> = Vec::new();
    for run in &results.runs {
        if !cases.contains(&run.case.as_str()) {
            cases.push(&run.case);
        }
    }
    let stats = |mut values: Vec<f64>| -> String {
        if values.is_empty() {
            return "n/a".into();
        }
        values.sort_by(f64::total_cmp);
        format!(
            "{:.0} / {:.0} ms",
            percentile(&values, 0.5),
            values[values.len() - 1]
        )
    };
    for case in cases {
        let runs: Vec<&Run> = results.runs.iter().filter(|run| run.case == case).collect();
        let kinds: std::collections::BTreeSet<_> = runs
            .iter()
            .filter_map(|run| run.browser_kind.clone())
            .collect();
        out.push_str(&format!(
            "| {case} | {} | {} | {} | {} | {} |\n",
            runs.len(),
            stats(runs.iter().map(|run| run.deploy_ms).collect()),
            stats(runs.iter().map(|run| run.host_ms).collect()),
            stats(runs.iter().filter_map(|run| run.browser_ms).collect()),
            kinds.into_iter().collect::<Vec<_>>().join(", "),
        ));
    }
    if let Some(baseline) = &results.load_baseline {
        out.push_str(&format!("\nLoad without swaps: {}\n", load_line(baseline)));
    }
    if let Some((during, swaps)) = &results.load_during_swaps {
        out.push_str(&format!(
            "Load during {swaps} swaps: {}\n",
            load_line(during)
        ));
    }
    out
}

fn load_line(report: &LoadReport) -> String {
    format!(
        "{} requests in {:.1} s, {:.0} requests/s, p50 {:.2} ms, p99 {:.2} ms, max {:.1} ms, \
         {} errors, {} mixed-version responses, {} manifests seen",
        report.requests,
        report.seconds,
        report.rps,
        report.p50_ms,
        report.p99_ms,
        report.max_ms,
        report.errors,
        report.mixed,
        report.manifests_seen
    )
}
