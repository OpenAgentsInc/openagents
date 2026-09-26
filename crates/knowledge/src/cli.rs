//! The `kb` command: finding, writing, measuring, and admitting entries.
//! Publishing to and syncing from a relay are `microcoder kb publish`,
//! `sync`, and `publish-evidence`, which hold the relay connection.

use std::path::{Path, PathBuf};

use crate::evidence::{self, Evaluator, Verdict};
use crate::harvest::{self, OpenRouterProposer, Written};
use crate::lint::{Corpus, default_corpora, lint};
use crate::remote::{self, Trust, TrustConfig};
use crate::search::{OpenRouterEmbedder, Retriever};
use crate::{
    Base, Entry, Kind, Status, archive, default_cache, default_dir, pending, set_evidence,
    set_status, template, today,
};

/// The `kb` command's help.
pub const USAGE: &str = "usage: kb <command> [options]

Finding entries:
  search <text>     list the entries that best match the text, with their scores
  show <id>         print one entry in full, with its digest
  lint              check every entry: its fields, its citation, no template
                    text, and that it neither names nor quotes a benchmark task

Writing entries:
  add <id> --kind KIND --title TEXT [--author NAME]
                    write a candidate entry file to fill in, then run kb lint
  harvest <run> [--model SLUG]
                    ask a model to propose general entries from a finished run
                    (a directory, or a name under ~/.openagents/microcoder/runs);
                    they're written as candidates, and a near-duplicate of an
                    existing entry becomes its next version
  harvest-trace <trajectory.json> --task NAME [--model SLUG]
                    the same from another agent's winning ATIF trajectory: what
                    it knew or did that a cheaper agent would miss; NAME is the
                    task it solved, which no entry may name

Admitting entries:
  evidence [ids] [--attach]
                    measure each entry from recorded runs: paired tasks with and
                    without it, never on a task it was written from; write a
                    NIP-EVAL report per entry, and with --attach add a line to
                    the entry's evidence
  admit <id> --reviewer NAME
                    admit an entry after reading it; the admission names you
  admit <id> --evidence
                    admit an entry only if its recorded report passes the rule
  withdraw <id> [--reason TEXT]
                    mark an entry wrong; it's never shown, and its file stays
  review [--apply]  list admitted entries shown often that never help, and
                    candidates the rule would admit; --apply demotes the first

Sharing entries (microcoder only):
  publish --relay URL [ids]      sign and publish entries (NIP-KB)
  sync --relay URL [--author KEY]...
                                 fetch, check, and cache other authors' entries
  publish-evidence --relay URL [ids]
                                 publish evidence reports for published entries

Options:
  --dir DIR         the knowledge directory (default OPENAGENTS_KNOWLEDGE, or
                    knowledge/ in the checkout this binary was built from)
  --candidates      include candidate entries, not only admitted ones
  --trust MODE      remote entries search includes: own, listed, or all
                    (default the mode in ~/.openagents/knowledge/trust.json, else own)
  --remote DIR      the synced entries (default ~/.openagents/knowledge/remote)
  --limit N         search results to list (default 10)
  --corpus DIR      a directory of benchmark tasks the lint checks against; it
                    can repeat (default Terminal-Bench 4 under ~/.openagents)
  --lexical         search by words alone, without embeddings
  --runs DIR        run records (default ~/.openagents/microcoder/runs)
  --evidence-dir DIR  evidence reports (default ~/.openagents/knowledge/evidence)";

/// Every option `kb` takes.
#[derive(Debug, Default)]
pub struct Options {
    pub dir: PathBuf,
    pub candidates: bool,
    pub limit: usize,
    pub corpora: Vec<PathBuf>,
    pub lexical: bool,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub model: Option<String>,
    /// For `harvest-trace`: the task the trajectory solved.
    pub task: Option<String>,
    pub runs: Option<PathBuf>,
    pub evidence_dir: Option<PathBuf>,
    pub attach: bool,
    pub reviewer: Option<String>,
    pub by_evidence: bool,
    pub reason: Option<String>,
    pub apply: bool,
    pub trust: Option<String>,
    pub remote: Option<PathBuf>,
    pub relay: Option<String>,
    pub authors: Vec<String>,
    /// Words that aren't options, in order.
    pub words: Vec<String>,
}

impl Options {
    /// The runs directory.
    ///
    /// # Errors
    ///
    /// When neither `--runs` nor `HOME` names one.
    pub fn runs(&self) -> Result<PathBuf, String> {
        self.runs
            .clone()
            .or_else(evidence::default_runs)
            .ok_or("no runs directory: pass --runs".to_string())
    }

    /// The evidence directory.
    ///
    /// # Errors
    ///
    /// When neither `--evidence-dir` nor `HOME` names one.
    pub fn evidence_dir(&self) -> Result<PathBuf, String> {
        self.evidence_dir
            .clone()
            .or_else(evidence::default_dir)
            .ok_or("no evidence directory: pass --evidence-dir".to_string())
    }

    /// The trust setting: the trust file, with `--trust` over its mode.
    ///
    /// # Errors
    ///
    /// A bad trust file or mode.
    pub fn trust(&self) -> Result<TrustConfig, String> {
        let mut config = match remote::trust_file() {
            Some(path) => TrustConfig::read(&path)?,
            None => TrustConfig::default(),
        };
        if let Some(mode) = &self.trust {
            config.mode = Trust::parse(mode)
                .ok_or(format!("--trust wants own, listed, or all, not {mode}"))?;
        }
        Ok(config)
    }
}

/// Parses `kb`'s arguments after the command.
///
/// # Errors
///
/// An unknown option or a missing value.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        dir: default_dir(),
        limit: 10,
        ..Options::default()
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--dir" => o.dir = PathBuf::from(value()?),
            "--candidates" => o.candidates = true,
            "--limit" => {
                let text = value()?;
                o.limit = text
                    .parse()
                    .map_err(|_| format!("--limit wants a number, not {text}"))?;
            }
            "--corpus" => o.corpora.push(PathBuf::from(value()?)),
            "--lexical" => o.lexical = true,
            "--kind" => o.kind = Some(value()?),
            "--title" => o.title = Some(value()?),
            "--author" => {
                let text = value()?;
                o.author = Some(text.clone());
                o.authors.push(text);
            }
            "--model" => o.model = Some(value()?),
            "--task" => o.task = Some(value()?),
            "--runs" => o.runs = Some(PathBuf::from(value()?)),
            "--evidence-dir" => o.evidence_dir = Some(PathBuf::from(value()?)),
            "--attach" => o.attach = true,
            "--reviewer" => o.reviewer = Some(value()?),
            "--evidence" => o.by_evidence = true,
            "--reason" => o.reason = Some(value()?),
            "--apply" => o.apply = true,
            "--trust" => o.trust = Some(value()?),
            "--remote" => o.remote = Some(PathBuf::from(value()?)),
            "--relay" => o.relay = Some(value()?),
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag}\n\n{USAGE}"));
            }
            word => o.words.push(word.to_string()),
        }
    }
    Ok(o)
}

/// Runs `kb` with `args` and returns the exit code: 0 on success, 1 when
/// the lint finds problems, an entry isn't there, or a check refuses, 2 on
/// bad usage.
pub async fn main(args: &[String]) -> u8 {
    match run(args).await {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

async fn run(args: &[String]) -> Result<u8, String> {
    let command = args.first().ok_or(USAGE)?.clone();
    let o = parse(&args[1..])?;
    match command.as_str() {
        "search" => search(&o).await,
        "show" => show(&o),
        "lint" => lint_all(&o),
        "add" => add(&o),
        "harvest" => harvest_run(&o).await,
        "harvest-trace" => harvest_trace(&o).await,
        "evidence" => measure(&o),
        "admit" => admit(&o),
        "withdraw" => withdraw(&o),
        "review" => review(&o),
        "publish" | "sync" | "publish-evidence" => Err(format!(
            "kb {command} runs from microcoder: microcoder kb {command} ..."
        )),
        "-h" | "--help" => Err(USAGE.to_string()),
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}

async fn search(o: &Options) -> Result<u8, String> {
    if o.words.is_empty() {
        return Err("kb search needs the text to search for".to_string());
    }
    let own = remote::key_file().and_then(|p| remote::own_pubkey(&p));
    let remote_dir = o.remote.clone().or_else(remote::default_dir);
    let (base, loaded) = remote::load(
        &o.dir,
        remote_dir.as_deref(),
        &o.trust()?,
        own.as_deref(),
        o.candidates,
    )?;
    if !loaded.remote.is_empty() {
        println!(
            "{} local entries and {} synced from {} authors",
            loaded.local,
            loaded.remote.values().sum::<usize>(),
            loaded.remote.len()
        );
    }
    let query = o.words.join(" ");
    let retriever = if o.lexical {
        Retriever::lexical(base, "--lexical was given")
    } else {
        match OpenRouterEmbedder::from_env() {
            Ok(embedder) => Retriever::new(base, embedder, default_cache()),
            Err(error) => Retriever::lexical(base, &error),
        }
    };
    let search = retriever.search(&query, o.limit).await;
    match &search.lexical_only {
        Some(why) => println!("ranked by words alone: {why}"),
        None => println!(
            "ranked by words and embeddings (${:.8} for embeddings)",
            search.usd
        ),
    }
    for (rank, hit) in search.hits.iter().enumerate() {
        let entry = retriever.base.get(&hit.id).ok_or("an entry went missing")?;
        println!(
            "{:>2}. {:.3}  {}  [{}, {}, {}]  words {:.2}{}  {}",
            rank + 1,
            hit.score,
            hit.id,
            entry.kind,
            entry.status,
            entry.author,
            hit.lexical,
            hit.semantic
                .map_or(String::new(), |s| format!(" · cosine {s:.3}")),
            entry.title
        );
    }
    Ok(0)
}

fn one_id<'a>(o: &'a Options, command: &str) -> Result<&'a str, String> {
    match o.words.as_slice() {
        [id] => Ok(id),
        _ => Err(format!("kb {command} needs one entry ID")),
    }
}

fn read_entry(dir: &Path, id: &str) -> Result<(PathBuf, String, Entry), String> {
    let path = dir.join(format!("{id}.md"));
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("no entry {id} in {}", dir.display()))?;
    let entry = Entry::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, text, entry))
}

fn show(o: &Options) -> Result<u8, String> {
    let id = one_id(o, "show")?;
    match read_entry(&o.dir, id) {
        Ok((_, text, entry)) => {
            println!("{}\n{}", entry.digest, text.trim_end());
            if let Some((path, next)) = pending(&o.dir, id, entry.version) {
                println!(
                    "\nVersion {} is waiting as a candidate in {}; kb admit promotes it.",
                    next.version,
                    path.display()
                );
            }
            Ok(0)
        }
        Err(error) => {
            eprintln!("{error}");
            Ok(1)
        }
    }
}

fn corpus(o: &Options) -> Corpus {
    let dirs = if o.corpora.is_empty() {
        default_corpora()
    } else {
        o.corpora.clone()
    };
    Corpus::read(&dirs)
}

fn lint_all(o: &Options) -> Result<u8, String> {
    let (entries, mut problems) = Base::read(&o.dir);
    let corpus = corpus(o);
    for absent in &corpus.absent {
        println!("skipped {}: it isn't there", absent.display());
    }
    problems.extend(lint(&entries, &corpus));
    println!(
        "checked {} entries in {} against {} tasks and {} test files",
        entries.len(),
        o.dir.display(),
        corpus.names.len(),
        corpus.tests.len()
    );
    for problem in &problems {
        println!("- {problem}");
    }
    if problems.is_empty() {
        println!("no problems");
        Ok(0)
    } else {
        println!("{} problems", problems.len());
        Ok(1)
    }
}

fn add(o: &Options) -> Result<u8, String> {
    let id = one_id(o, "add")?;
    let kind = o
        .kind
        .as_deref()
        .ok_or("kb add needs --kind: method, edge-case, slip, environment, or tool".to_string())?;
    let kind = Kind::parse(kind).ok_or(format!(
        "unknown kind {kind}: use method, edge-case, slip, environment, or tool"
    ))?;
    let title = o.title.as_deref().ok_or("kb add needs --title")?;
    let author = o.author.clone().unwrap_or_else(|| "openagents".to_string());
    let text = template(id, kind, title, &author)?;
    let path = o.dir.join(format!("{id}.md"));
    if path.exists() {
        eprintln!(
            "{} already exists; edit it, or pick another ID",
            path.display()
        );
        return Ok(1);
    }
    std::fs::write(&path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
    println!(
        "wrote {}: a candidate with template text marked TODO. Fill in the summary, \
applies_when, tags, citations, and body, then run kb lint.",
        path.display()
    );
    Ok(0)
}

async fn harvest_trace(o: &Options) -> Result<u8, String> {
    let path = PathBuf::from(one_id(o, "harvest-trace")?);
    let task = o
        .task
        .clone()
        .ok_or("kb harvest-trace needs --task NAME: the task the trajectory solved")?;
    let record = harvest::trace_record(&path, &task)?;
    harvest_and_report(o, record, &path).await
}

async fn harvest_run(o: &Options) -> Result<u8, String> {
    let run = one_id(o, "harvest")?;
    let mut run_dir = PathBuf::from(run);
    if !run_dir.is_dir() {
        run_dir = o.runs()?.join(run);
    }
    if !run_dir.is_dir() {
        return Err(format!("no run {run}: pass a run directory or its name"));
    }
    let record = harvest::record(&run_dir)?;
    harvest_and_report(o, record, &run_dir).await
}

async fn harvest_and_report(
    o: &Options,
    record: harvest::Record,
    run_dir: &Path,
) -> Result<u8, String> {
    let model = o
        .model
        .clone()
        .unwrap_or_else(|| harvest::MODEL.to_string());
    let proposer = OpenRouterProposer::from_env(&model)?;
    let (entries, _) = Base::read(&o.dir);
    let base = Base { entries };
    let retriever = match OpenRouterEmbedder::from_env() {
        Ok(embedder) => Some(Retriever::new(base, embedder, default_cache())),
        Err(error) => {
            println!(
                "no embeddings ({error}); near-duplicates are found only by ID and the model's `updates`"
            );
            None
        }
    };
    println!("reading {} with {model}", run_dir.display());
    let result =
        harvest::harvest_record(record, &o.dir, &proposer, retriever.as_ref(), &corpus(o)).await?;
    if result.proposals.is_empty() {
        println!("the model proposed no entries");
    }
    for (id, written) in &result.proposals {
        match written {
            Written::New(path) => println!("- {id}: new candidate at {}", path.display()),
            Written::Version {
                path,
                pending: true,
            } => println!(
                "- {id}: a new version, waiting in {} because the current version is admitted; \
kb admit {id} promotes it",
                path.display()
            ),
            Written::Version { path, .. } => println!(
                "- {id}: a new version at {}; the earlier one moved to versions/",
                path.display()
            ),
            Written::Refused(why) => println!("- {id}: not written: {why}"),
        }
    }
    println!("${:.5} for the model and embeddings", result.usd);
    Ok(0)
}

fn selected(o: &Options) -> Result<Vec<Entry>, String> {
    let (entries, problems) = Base::read(&o.dir);
    if let Some(problem) = problems.first() {
        return Err(format!("{}: {problem}", o.dir.display()));
    }
    for id in &o.words {
        if !entries.iter().any(|e| &e.id == id) {
            return Err(format!("no entry {id} in {}", o.dir.display()));
        }
    }
    Ok(entries
        .into_iter()
        .filter(|e| o.words.is_empty() || o.words.contains(&e.id))
        .collect())
}

fn verdict_word(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
        Verdict::Inconclusive => "inconclusive",
    }
}

fn measure(o: &Options) -> Result<u8, String> {
    let runs = evidence::scan(&o.runs()?);
    let dir = o.evidence_dir()?;
    println!(
        "{} runs with a summary in {}",
        runs.len(),
        o.runs()?.display()
    );
    println!("rule: {}", evidence::RULE);
    for entry in selected(o)? {
        if entry.status == Status::Withdrawn {
            continue;
        }
        let path = o.dir.join(format!("{}.md", entry.id));
        let document = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let measured = evidence::measure(&entry, &runs);
        let (report, artifacts) =
            evidence::report(&measured, &document, &runs, &Evaluator::local(), None);
        let report_path = evidence::report_path(&dir, &entry.id, entry.version);
        let text = evidence::write(&report_path, &report, &artifacts)?;
        println!(
            "{} v{} [{}]: {} with it; {}: {}{}",
            entry.id,
            entry.version,
            entry.status,
            crate::evidence::count(measured.runs_with, "run"),
            crate::evidence::tally(&measured),
            verdict_word(measured.verdict),
            if measured.excluded_tasks.is_empty() {
                String::new()
            } else {
                format!(
                    " (not counting {}, which it was written from)",
                    measured.excluded_tasks.join(", ")
                )
            }
        );
        if o.attach {
            let line = evidence::line(&measured, &crate::digest(text.as_bytes()));
            let mut lines: Vec<String> = entry
                .evidence
                .iter()
                .filter(|l| !l.starts_with("measured "))
                .cloned()
                .collect();
            lines.push(line);
            let updated = set_evidence(&document, &lines)?;
            std::fs::write(&path, updated).map_err(|e| e.to_string())?;
        }
    }
    println!("reports in {}", dir.display());
    Ok(0)
}

fn admit(o: &Options) -> Result<u8, String> {
    let id = one_id(o, "admit")?;
    let (mut path, mut text, mut entry) = read_entry(&o.dir, id)?;
    if let Some((waiting, next)) = pending(&o.dir, id, entry.version) {
        archive(&o.dir, id)?;
        std::fs::rename(&waiting, &path).map_err(|e| format!("can't promote {id}: {e}"))?;
        println!(
            "promoted version {} of {id}; version {} moved to versions/",
            next.version, entry.version
        );
        (path, text, entry) = read_entry(&o.dir, id)?;
    }
    let line = match (&o.reviewer, o.by_evidence) {
        (Some(name), false) => format!("admitted {} by review: {name}", today()),
        (None, true) => {
            let report = evidence::report_path(&o.evidence_dir()?, id, entry.version);
            let (verdict, digest) = evidence::recorded(&report)?;
            if verdict != Verdict::Pass {
                eprintln!(
                    "not admitted: the recorded evidence for {id} v{} is {}. The rule: {}",
                    entry.version,
                    verdict_word(verdict),
                    evidence::RULE
                );
                return Ok(1);
            }
            format!("admitted {} by measurement: {digest}", today())
        }
        _ => return Err("kb admit needs either --reviewer NAME or --evidence".to_string()),
    };
    let mut lines = entry.evidence.clone();
    lines.push(line.clone());
    let updated = set_evidence(&set_status(&text, Status::Admitted)?, &lines)?;
    Entry::parse(&updated)?;
    std::fs::write(&path, updated).map_err(|e| e.to_string())?;
    println!("{id} v{}: {line}", entry.version);
    Ok(0)
}

fn withdraw(o: &Options) -> Result<u8, String> {
    let id = one_id(o, "withdraw")?;
    let (path, text, entry) = read_entry(&o.dir, id)?;
    let reason = o
        .reason
        .clone()
        .unwrap_or_else(|| "marked wrong".to_string());
    let mut lines = entry.evidence.clone();
    lines.push(format!("withdrawn {}: {reason}", today()));
    let updated = set_evidence(&set_status(&text, Status::Withdrawn)?, &lines)?;
    std::fs::write(&path, updated).map_err(|e| e.to_string())?;
    println!(
        "{id} v{} is withdrawn: it's never shown, and its file stays. \
microcoder kb publish sends the withdrawal to a relay it was published to.",
        entry.version
    );
    Ok(0)
}

fn review(o: &Options) -> Result<u8, String> {
    let runs = evidence::scan(&o.runs()?);
    let entries = selected(o)?;
    let proposals = evidence::review(&entries, &runs);
    println!(
        "{} entries against {} runs. An admitted entry shown in {} or more runs, out of \
sample, with paired tasks and none favoring it, is demoted to candidate.",
        entries.len(),
        runs.len(),
        evidence::DEMOTE_MIN_RUNS
    );
    if proposals.is_empty() {
        println!("nothing to change");
    }
    for p in &proposals {
        match p.action {
            "demote" => println!("- demote {}: {}", p.id, p.reason),
            _ => println!(
                "- admit {}: {}; run kb evidence {} and kb admit {} --evidence",
                p.id, p.reason, p.id, p.id
            ),
        }
    }
    if o.apply {
        for p in proposals.iter().filter(|p| p.action == "demote") {
            let (path, text, entry) = read_entry(&o.dir, &p.id)?;
            let mut lines = entry.evidence.clone();
            lines.push(format!("demoted {}: {}", today(), p.reason));
            let updated = set_evidence(&set_status(&text, Status::Candidate)?, &lines)?;
            std::fs::write(&path, updated).map_err(|e| e.to_string())?;
            println!("demoted {}", p.id);
        }
    } else if proposals.iter().any(|p| p.action == "demote") {
        println!("run kb review --apply to demote them");
    }
    Ok(0)
}
