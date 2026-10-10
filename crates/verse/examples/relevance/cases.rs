//! The non-visual half of the relevance visualizer: the case (an issue and
//! its candidate files), the backends that judge it, the System One request
//! each file becomes, and the scoreboard the answers land on.
//!
//! The request is the one `scripts/bench/clef-relevance-bench.py` sends in
//! its `seq` mode: a state of `RUN <nonce>`, the issue, and one fenced file,
//! with a single `noul` question keyed `relevant`. Keeping the shape lets the
//! visualizer's numbers be read beside the benchmark's
//! (`docs/inference/clef-jev-relevance-bench.md`).

use std::time::Duration;

/// The question every backend answers about every file.
pub const QUESTION: &str = "Is this file relevant to solving the issue?";
/// Bytes of a file the state carries; the benchmark's `--cap` default.
pub const FILE_CAP: usize = 4096;
/// Characters of an issue body the state carries. Ollama refuses request
/// bodies over 64 KiB, so a long issue is cut.
pub const BODY_CAP: usize = 6000;
/// A probability at or above this counts as "relevant".
pub const THRESHOLD: f64 = 0.5;

/// A GitHub issue's title and body.
#[derive(Clone, Debug)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
}

/// Where a candidate file came from. None of these read the issue's text:
/// the decision model is the only thing in the loop that does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Changed by the commit that fixed the issue: the ground truth.
    Fix,
    /// Beside a fix file in the same directory, untouched by the fix: a hard
    /// negative, as the benchmark's dataset picks them.
    Sibling,
    /// In a directory a recent commit on main touched.
    Recent,
    /// Anywhere in the repository.
    Random,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Fix => "fix",
            Origin::Sibling => "sibling",
            Origin::Recent => "recent",
            Origin::Random => "random",
        }
    }
}

/// One file the backends judge.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub origin: Origin,
}

impl Candidate {
    /// The ground truth when the case has a fix: changed by it or not.
    pub fn relevant(&self, labeled: bool) -> Option<bool> {
        labeled.then_some(self.origin == Origin::Fix)
    }
}

/// The commits on main whose subject names the issue, and what they changed.
#[derive(Clone, Debug)]
pub struct Fix {
    pub commits: Vec<String>,
    pub files: Vec<String>,
}

/// An issue, the revision its files were read at, and the candidates.
#[derive(Clone, Debug)]
pub struct Case {
    pub issue: Issue,
    pub fix: Option<Fix>,
    pub rev: String,
    pub candidates: Vec<Candidate>,
}

impl Case {
    pub fn labeled(&self) -> bool {
        self.fix.is_some()
    }
    pub fn labels(&self) -> Vec<Option<bool>> {
        let labeled = self.labeled();
        self.candidates
            .iter()
            .map(|c| c.relevant(labeled))
            .collect()
    }
}

/// The issue number a commit subject ends with, as in `Fix the thing (#123)`.
/// Commits on main name their issue this way; this parses that one bounded
/// field and routes nothing.
pub fn subject_issue(subject: &str) -> Option<u64> {
    let rest = subject.trim_end().strip_suffix(')')?;
    let start = rest.rfind("(#")?;
    let digits = &rest[start + 2..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Whether a tracked path is a source or document file worth judging:
/// text, written by people, and not a fixture, lockfile, or generated file.
pub fn eligible(path: &str) -> bool {
    const EXTENSIONS: &[&str] = &[
        "rs", "ts", "tsx", "js", "mjs", "py", "md", "toml", "sh", "swift", "kt", "go", "sql",
        "wgsl", "css", "html", "yaml", "yml",
    ];
    const SKIP: &[&str] = &[
        "/fixtures/",
        "/generated/",
        "/vendor/",
        "/node_modules/",
        "/snapshots/",
        "/captures/",
        "/dist/",
        "/traces/",
        "/artifacts/",
    ];
    let Some((_, extension)) = path.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS.contains(&extension)
        && !SKIP.iter().any(|s| path.contains(s))
        && !path.ends_with(".lock")
        && !path.starts_with("docs/ste/")
}

/// The directory a path is in, or "" at the root.
pub fn dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The fence language for a file, for the state's code block.
pub fn fence(path: &str) -> &'static str {
    match path.rsplit_once('.').map_or("", |(_, e)| e) {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "mjs" => "javascript",
        "py" => "python",
        "md" => "markdown",
        "toml" => "toml",
        "sh" => "sh",
        "swift" => "swift",
        "kt" => "kotlin",
        "go" => "go",
        "sql" => "sql",
        "wgsl" => "wgsl",
        "css" => "css",
        "html" => "html",
        "yaml" | "yml" => "yaml",
        _ => "",
    }
}

/// Cuts `text` to at most `cap` bytes on a character boundary.
pub fn cap(text: &str, cap: usize) -> (String, bool) {
    if text.len() <= cap {
        return (text.to_owned(), false);
    }
    let mut end = cap;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

/// The issue as the state's header, in the benchmark's shape.
pub fn issue_text(issue: &Issue) -> String {
    let (body, cut) = cap(issue.body.trim(), BODY_CAP);
    let tail = if cut { "\n[issue body truncated]" } else { "" };
    format!(
        "ISSUE #{}: {}\n\n{}{}\n",
        issue.number, issue.title, body, tail
    )
}

/// The state one file is judged in: a run nonce (so no answer comes from a
/// previous run's prompt cache), the issue, and the file.
pub fn state(issue: &Issue, candidate: &Candidate, nonce: &str) -> String {
    let marker = if candidate.truncated {
        "\n/* [truncated] */"
    } else {
        ""
    };
    format!(
        "RUN {nonce}\n\n{}\nFILE: {}\n```{}\n{}{marker}\n```\n",
        issue_text(issue),
        candidate.path,
        fence(&candidate.path),
        candidate.content,
    )
}

/// The System One request for one file: one `noul` question, `relevant`.
pub fn request(issue: &Issue, candidate: &Candidate, nonce: &str) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(
        state(issue, candidate, nonce),
        jev::Questions::new().with("relevant", jev::Noul::new(QUESTION)),
    )
}

/// How a backend authenticates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Auth {
    /// A loopback server that takes no key (Ollama, llama.cpp, psionic).
    Local,
    /// TypeSafe's hosted API, with `TYPESAFE_API_KEY`.
    TypeSafe,
}

/// A place a System One request can go.
#[derive(Clone, Debug, PartialEq)]
pub struct Backend {
    /// The lane's short name on the command line and the scoreboard.
    pub id: &'static str,
    /// What the lane is, for people.
    pub label: &'static str,
    pub base: String,
    pub model: String,
    pub auth: Auth,
    /// The lane's color, linear RGB, from the Greco-futurist palette.
    pub color: [f32; 3],
}

/// Lane colors. The first three are `docs/verse/greco-futurism.md`'s
/// marble, amber, and copper; the rest are the classical pigments a bronze
/// and limestone scene admits beside them: verdigris (weathered bronze),
/// Egyptian blue, gilt, and Tyrian purple.
pub const MARBLE: [f32; 3] = [0.807, 0.787, 0.729];
pub const AMBER: [f32; 3] = [1.0, 0.42, 0.073];
pub const COPPER: [f32; 3] = [0.62, 0.20, 0.06];
pub const VERDIGRIS: [f32; 3] = [0.10, 0.52, 0.40];
pub const EGYPTIAN_BLUE: [f32; 3] = [0.08, 0.20, 0.72];
pub const GILT: [f32; 3] = [0.80, 0.62, 0.16];
pub const TYRIAN: [f32; 3] = [0.42, 0.05, 0.34];
/// The ground-truth mark: the style's hedge, lifted to read in the dark.
pub const LAUREL: [f32; 3] = [0.10, 0.45, 0.06];

/// Every lane the visualizer knows, in scoreboard order. The ports are the
/// benchmark's (`scripts/bench/clef-relevance-bench.py`); `coderos-4080`
/// is the 4080's llama-server over an ssh tunnel, and `psionic` is the
/// slot for our own `/v1/systemone` (`docs/inference/clef-native.md`),
/// read from `PSIONIC_SYSTEMONE_URL` once it exists.
pub fn registry() -> Vec<Backend> {
    let local = |id, label, base: &str, model: &str, color| Backend {
        id,
        label,
        base: base.into(),
        model: model.into(),
        auth: Auth::Local,
        color,
    };
    vec![
        Backend {
            id: "jev",
            label: "Jev (TypeSafe API)",
            base: "https://api.typesafe.ai".into(),
            model: "jev-latest".into(),
            auth: Auth::TypeSafe,
            color: MARBLE,
        },
        local(
            "ollama-flash",
            "Clef-Flash 9B, Ollama, Mac",
            "http://127.0.0.1:11434",
            "clef-flash",
            AMBER,
        ),
        local(
            "ollama-clef",
            "Clef 27B, Ollama, Mac",
            "http://127.0.0.1:11434",
            "clef",
            COPPER,
        ),
        local(
            "llamacpp-flash",
            "Clef-Flash Q4, llama.cpp Metal, Mac",
            "http://127.0.0.1:18093",
            "clef-flash",
            VERDIGRIS,
        ),
        local(
            "coderos-4080",
            "Clef-Flash Q4, llama.cpp, CoderOS 4080",
            "http://127.0.0.1:21091",
            "clef-flash",
            EGYPTIAN_BLUE,
        ),
        local(
            "llamacpp-27b",
            "Clef 27B Q4, llama.cpp Metal, Mac",
            "http://127.0.0.1:18094",
            "clef",
            TYRIAN,
        ),
        local(
            "psionic",
            "Psionic native (planned)",
            &std::env::var("PSIONIC_SYSTEMONE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:18100".into()),
            "clef-flash",
            GILT,
        ),
    ]
}

/// The lanes a run compares unless told otherwise.
pub const DEFAULT_BACKENDS: &str =
    "jev,ollama-flash,ollama-clef,llamacpp-flash,coderos-4080,psionic";

/// Resolves a comma-separated list of lane ids. Also takes the owner's
/// first spelling, `clef-ollama`, `clef-llamacpp`, and `jev`, where
/// `model` (`clef-flash` or `clef`) picks the Clef size.
pub fn backends(list: &str, model: Option<&str>) -> Result<Vec<Backend>, String> {
    let all = registry();
    let mut chosen: Vec<Backend> = Vec::new();
    for name in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let id = match (name, model) {
            ("clef-ollama", Some("clef")) => "ollama-clef",
            ("clef-ollama", _) => "ollama-flash",
            ("clef-llamacpp", _) => "llamacpp-flash",
            (other, _) => other,
        };
        let mut backend = all.iter().find(|b| b.id == id).cloned().ok_or_else(|| {
            let ids: Vec<_> = all.iter().map(|b| b.id).collect();
            format!("unknown backend {name}; known: {}", ids.join(", "))
        })?;
        if let (Some(m), Auth::Local) = (model, backend.auth)
            && name.starts_with("clef-llamacpp")
        {
            backend.model = m.into();
        }
        if !chosen.iter().any(|b| b.id == backend.id) {
            chosen.push(backend);
        }
    }
    if chosen.is_empty() {
        return Err("no backends chosen".into());
    }
    Ok(chosen)
}

/// The `host:port` of a base URL, for the reachability probe.
pub fn host_port(base: &str) -> Option<String> {
    let rest = base.split_once("://").map_or(base, |(_, r)| r);
    let authority = rest.split('/').next()?;
    if authority.is_empty() {
        return None;
    }
    let default = if base.starts_with("https://") {
        443
    } else {
        80
    };
    Some(if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:{default}")
    })
}

/// A small deterministic generator, so a seed replays a case.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

/// The pools candidates are drawn from, each already filtered to files that
/// exist at the case's revision.
#[derive(Clone, Debug, Default)]
pub struct Pools {
    pub fix: Vec<String>,
    pub siblings: Vec<String>,
    pub recent: Vec<String>,
    pub all: Vec<String>,
}

/// Picks `k` candidate paths: up to half from the fix (the ground truth),
/// up to a quarter from the fix's directory siblings (hard negatives), then
/// half of what is left from directories recent commits touched, and the
/// rest from anywhere. A pool that runs short leaves its share to the next.
/// The issue's text plays no part: picking files by matching words in it is
/// the judgment the model is being tested on. The result is sorted by path,
/// so files of one directory stand together on the plaza.
pub fn select(pools: &Pools, k: usize, rng: &mut Rng) -> Vec<(String, Origin)> {
    let mut chosen: Vec<(String, Origin)> = Vec::new();
    let fix = &pools.fix;
    let mut take =
        |pool: &[String], origin: Origin, n: usize, chosen: &mut Vec<(String, Origin)>| {
            // A fix file is ground truth: it never enters under another label.
            let mut pool: Vec<&String> = pool
                .iter()
                .filter(|p| origin == Origin::Fix || !fix.contains(p))
                .filter(|p| !chosen.iter().any(|(c, _)| c == *p))
                .collect();
            pool.sort();
            pool.dedup();
            rng.shuffle(&mut pool);
            for path in pool.into_iter().take(n) {
                chosen.push((path.clone(), origin));
            }
        };
    take(&pools.fix, Origin::Fix, k.div_ceil(2), &mut chosen);
    let left = k - chosen.len();
    take(
        &pools.siblings,
        Origin::Sibling,
        (k / 4).min(left),
        &mut chosen,
    );
    let left = k - chosen.len();
    take(&pools.recent, Origin::Recent, left.div_ceil(2), &mut chosen);
    let left = k - chosen.len();
    take(&pools.all, Origin::Random, left, &mut chosen);
    let left = k - chosen.len();
    take(&pools.recent, Origin::Recent, left, &mut chosen);
    chosen.sort_by(|a, b| a.0.cmp(&b.0));
    chosen
}

/// The order the lanes walk the files in: one shuffle shared by every lane,
/// so they judge the same file at about the same time and the plaza lights
/// up across its whole ring.
pub fn order(n: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    Rng::new(seed ^ 0x5EED).shuffle(&mut order);
    order
}

/// Where a lane stands.
#[derive(Clone, Debug, PartialEq)]
pub enum LaneStatus {
    Waiting,
    Running,
    Offline(String),
    Done,
}

/// One lane's answers and timings.
#[derive(Clone, Debug)]
pub struct Lane {
    pub backend: Backend,
    pub status: LaneStatus,
    /// Probability per candidate, once answered.
    pub p: Vec<Option<f64>>,
    pub latencies: Vec<Duration>,
    pub errors: usize,
    pub last_error: Option<String>,
    /// Seconds since the run started when the lane answered first and last.
    pub first: Option<f64>,
    pub last: Option<f64>,
    pub started: Option<f64>,
}

/// Precision and recall at [`THRESHOLD`] over the files a lane answered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quality {
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub accuracy: f64,
    pub judged: usize,
}

impl Lane {
    pub fn new(backend: Backend, files: usize) -> Self {
        Self {
            backend,
            status: LaneStatus::Waiting,
            p: vec![None; files],
            latencies: Vec::new(),
            errors: 0,
            last_error: None,
            first: None,
            last: None,
            started: None,
        }
    }
    pub fn answered(&self) -> usize {
        self.p.iter().filter(|p| p.is_some()).count()
    }
    /// Decisions per second from the lane's start to its latest answer.
    pub fn rate(&self) -> Option<f64> {
        let (start, last) = (self.started?, self.last?);
        let n = self.latencies.len();
        (n > 0 && last > start).then(|| n as f64 / (last - start))
    }
    pub fn p50(&self) -> Option<Duration> {
        percentile(&self.latencies, 0.5)
    }
    pub fn p90(&self) -> Option<Duration> {
        percentile(&self.latencies, 0.9)
    }
    pub fn quality(&self, labels: &[Option<bool>]) -> Option<Quality> {
        quality(&self.p, labels)
    }
}

/// The `q` quantile by nearest rank, as the benchmark computes it.
pub fn percentile(xs: &[Duration], q: f64) -> Option<Duration> {
    if xs.is_empty() {
        return None;
    }
    let mut xs = xs.to_vec();
    xs.sort();
    let i = ((q * (xs.len() - 1) as f64).round() as usize).min(xs.len() - 1);
    Some(xs[i])
}

/// Precision, recall, and accuracy of `p` against labels, over files that
/// have both. None when nothing is labeled and answered.
pub fn quality(p: &[Option<f64>], labels: &[Option<bool>]) -> Option<Quality> {
    let (mut tp, mut fp, mut fn_, mut right, mut judged) = (0, 0, 0, 0, 0);
    for (p, label) in p.iter().zip(labels) {
        let (Some(p), Some(label)) = (p, label) else {
            continue;
        };
        judged += 1;
        let said = *p >= THRESHOLD;
        match (said, *label) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => {}
        }
        if said == *label {
            right += 1;
        }
    }
    if judged == 0 {
        return None;
    }
    let ratio = |a: usize, b: usize| (b > 0).then(|| a as f64 / b as f64);
    Some(Quality {
        precision: ratio(tp, tp + fp),
        recall: ratio(tp, tp + fn_),
        accuracy: right as f64 / judged as f64,
        judged,
    })
}

/// Whether the lanes that answered a file split across the threshold.
pub fn disagree(ps: &[Option<f64>]) -> bool {
    let answered: Vec<f64> = ps.iter().flatten().copied().collect();
    answered.iter().any(|p| *p >= THRESHOLD) && answered.iter().any(|p| *p < THRESHOLD)
}

/// One file's row in the ranking.
#[derive(Clone, Debug)]
pub struct Row {
    pub file: usize,
    pub mean: Option<f64>,
    pub p: Vec<Option<f64>>,
    pub disagree: bool,
}

/// Files by their mean probability across the lanes that answered, highest
/// first; unanswered files last, in path order.
pub fn ranking(lanes: &[Lane], files: usize) -> Vec<Row> {
    let mut rows: Vec<Row> = (0..files)
        .map(|file| {
            let p: Vec<Option<f64>> = lanes.iter().map(|l| l.p[file]).collect();
            let answered: Vec<f64> = p.iter().flatten().copied().collect();
            let mean = (!answered.is_empty())
                .then(|| answered.iter().sum::<f64>() / answered.len() as f64);
            Row {
                file,
                mean,
                disagree: disagree(&p),
                p,
            }
        })
        .collect();
    rows.sort_by(|a, b| match (a.mean, b.mean) {
        (Some(x), Some(y)) => y.total_cmp(&x).then(a.file.cmp(&b.file)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.file.cmp(&b.file),
    });
    rows
}

/// Formats seconds for the tables: `0.84s`, `12.3s`.
pub fn secs(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 10.0 {
        format!("{s:.2}s")
    } else {
        format!("{s:.1}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue() -> Issue {
        Issue {
            number: 42,
            title: "Login crashes".into(),
            body: "Steps: open the app.".into(),
            state: "CLOSED".into(),
        }
    }

    fn candidate(path: &str, origin: Origin) -> Candidate {
        Candidate {
            path: path.into(),
            content: "fn login() {}".into(),
            truncated: false,
            origin,
        }
    }

    #[test]
    fn subjects_name_their_issue_at_the_end() {
        assert_eq!(subject_issue("Fix the login (#11106)"), Some(11106));
        assert_eq!(subject_issue("Fix (#12) the login"), None);
        assert_eq!(subject_issue("Fix (#)"), None);
        assert_eq!(subject_issue("Fix (#12a)"), None);
        assert_eq!(subject_issue("Plain subject"), None);
        assert_eq!(subject_issue("Two refs (#1) (#2)  "), Some(2));
    }

    #[test]
    fn eligibility_keeps_sources_and_drops_generated_files() {
        assert!(eligible("crates/jev/src/client.rs"));
        assert!(eligible("docs/verse/relevance-visualizer.md"));
        assert!(!eligible("Cargo.lock"));
        assert!(!eligible("crates/jev/tests/fixtures/systemone-ok.json"));
        assert!(!eligible("assets/logo.png"));
        assert!(!eligible("Makefile"));
        assert!(!eligible("crates/x/fixtures/a.rs"));
    }

    #[test]
    fn the_state_has_the_benchmark_shape() {
        let s = state(&issue(), &candidate("src/login.rs", Origin::Fix), "abc");
        assert!(s.starts_with("RUN abc\n\nISSUE #42: Login crashes\n\nSteps: open the app.\n"));
        assert!(s.contains("\nFILE: src/login.rs\n```rust\nfn login() {}\n```\n"));
    }

    #[test]
    fn long_bodies_and_files_are_cut_on_character_boundaries() {
        let (text, cut) = cap("ééé", 3);
        assert_eq!((text.as_str(), cut), ("é", true));
        let mut long = issue();
        long.body = "x".repeat(BODY_CAP + 10);
        assert!(issue_text(&long).ends_with("[issue body truncated]\n"));
        let mut c = candidate("a.rs", Origin::Random);
        c.truncated = true;
        assert!(state(&issue(), &c, "n").contains("/* [truncated] */\n```"));
    }

    #[test]
    fn the_request_asks_one_noul_question() {
        let body = request(&issue(), &candidate("src/login.rs", Origin::Fix), "n")
            .body("clef-flash")
            .unwrap();
        assert_eq!(body["model"], "clef-flash");
        assert_eq!(body["questions"]["relevant"]["type"], "noul");
        assert_eq!(body["questions"]["relevant"]["instructions"], QUESTION);
        assert!(
            body["state"]
                .as_str()
                .unwrap()
                .contains("FILE: src/login.rs")
        );
        assert_eq!(body["questions"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn backends_resolve_ids_and_the_first_spellings() {
        let b = backends("jev, ollama-clef,jev", None).unwrap();
        assert_eq!(
            b.iter().map(|b| b.id).collect::<Vec<_>>(),
            ["jev", "ollama-clef"]
        );
        assert_eq!(
            backends("clef-ollama", Some("clef")).unwrap()[0].id,
            "ollama-clef"
        );
        assert_eq!(
            backends("clef-ollama", Some("clef-flash")).unwrap()[0].id,
            "ollama-flash"
        );
        let l = backends("clef-llamacpp", Some("clef")).unwrap();
        assert_eq!((l[0].id, l[0].model.as_str()), ("llamacpp-flash", "clef"));
        assert!(backends("nope", None).is_err());
        assert!(backends("", None).is_err());
        assert_eq!(backends(DEFAULT_BACKENDS, None).unwrap().len(), 6);
        assert_eq!(backends("jev", None).unwrap()[0].auth, Auth::TypeSafe);
    }

    #[test]
    fn host_port_reads_the_authority() {
        assert_eq!(
            host_port("http://127.0.0.1:11434").as_deref(),
            Some("127.0.0.1:11434")
        );
        assert_eq!(
            host_port("https://api.typesafe.ai/").as_deref(),
            Some("api.typesafe.ai:443")
        );
        assert_eq!(host_port("http://").as_deref(), None);
    }

    fn paths(prefix: &str, n: usize) -> Vec<String> {
        (0..n).map(|i| format!("{prefix}/f{i:02}.rs")).collect()
    }

    #[test]
    fn selection_takes_its_shares_and_never_repeats() {
        let pools = Pools {
            fix: paths("fix", 3),
            siblings: paths("fix", 10),
            recent: paths("recent", 20),
            all: paths("all", 50),
        };
        let chosen = select(&pools, 12, &mut Rng::new(7));
        assert_eq!(chosen.len(), 12);
        let count = |o| chosen.iter().filter(|(_, x)| *x == o).count();
        assert_eq!(count(Origin::Fix), 3);
        assert_eq!(count(Origin::Sibling), 3);
        assert_eq!(count(Origin::Recent), 3);
        assert_eq!(count(Origin::Random), 3);
        let mut unique: Vec<_> = chosen.iter().map(|(p, _)| p).collect();
        unique.dedup();
        assert_eq!(unique.len(), 12);
        assert!(
            chosen.windows(2).all(|w| w[0].0 <= w[1].0),
            "sorted by path"
        );
        // A file in the fix is labeled fix even when a sibling pool holds it.
        for (path, origin) in &chosen {
            if pools.fix.contains(path) {
                assert_eq!(*origin, Origin::Fix);
            }
        }
    }

    #[test]
    fn selection_caps_the_fix_at_half_and_fills_short_pools() {
        let pools = Pools {
            fix: paths("fix", 30),
            siblings: vec![],
            recent: vec![],
            all: paths("all", 4),
        };
        let chosen = select(&pools, 10, &mut Rng::new(1));
        assert_eq!(chosen.iter().filter(|(_, o)| *o == Origin::Fix).count(), 5);
        assert_eq!(chosen.len(), 9, "the fix half, then every random file");
        let overlapping = Pools {
            fix: paths("fix", 8),
            all: paths("fix", 8),
            ..Pools::default()
        };
        let chosen = select(&overlapping, 8, &mut Rng::new(3));
        assert!(chosen.iter().all(|(_, o)| *o == Origin::Fix), "{chosen:?}");
        assert_eq!(chosen.len(), 4);
        let open = Pools {
            recent: paths("recent", 3),
            all: paths("all", 40),
            ..Pools::default()
        };
        let chosen = select(&open, 8, &mut Rng::new(2));
        assert_eq!(chosen.len(), 8);
        assert_eq!(
            chosen.iter().filter(|(_, o)| *o == Origin::Recent).count(),
            3
        );
    }

    #[test]
    fn selection_replays_from_a_seed() {
        let pools = Pools {
            all: paths("all", 100),
            ..Pools::default()
        };
        assert_eq!(
            select(&pools, 9, &mut Rng::new(5)),
            select(&pools, 9, &mut Rng::new(5))
        );
        assert_ne!(
            select(&pools, 9, &mut Rng::new(5)),
            select(&pools, 9, &mut Rng::new(6))
        );
        let mut o = order(10, 3);
        assert_ne!(o, (0..10).collect::<Vec<_>>());
        o.sort();
        assert_eq!(o, (0..10).collect::<Vec<_>>());
    }

    fn lane(ps: &[Option<f64>], ms: &[u64]) -> Lane {
        let mut lane = Lane::new(registry().remove(1), ps.len());
        lane.p = ps.to_vec();
        lane.latencies = ms.iter().map(|m| Duration::from_millis(*m)).collect();
        lane
    }

    #[test]
    fn quality_scores_against_the_fix() {
        let labels = [Some(true), Some(true), Some(false), Some(false), None];
        let p = [Some(0.9), Some(0.2), Some(0.7), Some(0.1), Some(0.99)];
        let q = quality(&p, &labels).unwrap();
        assert_eq!(q.judged, 4);
        assert_eq!(q.precision, Some(0.5));
        assert_eq!(q.recall, Some(0.5));
        assert_eq!(q.accuracy, 0.5);
        assert!(quality(&p, &[None; 5]).is_none());
        let none_said = quality(&[Some(0.1), Some(0.2)], &[Some(true), Some(false)]).unwrap();
        assert_eq!((none_said.precision, none_said.recall), (None, Some(0.0)));
    }

    #[test]
    fn latency_percentiles_and_rate() {
        let mut l = lane(&[Some(0.5); 4], &[400, 100, 300, 200]);
        assert_eq!(l.p50(), Some(Duration::from_millis(300)));
        assert_eq!(l.p90(), Some(Duration::from_millis(400)));
        assert_eq!(percentile(&[], 0.5), None);
        l.started = Some(1.0);
        l.last = Some(3.0);
        assert_eq!(l.rate(), Some(2.0));
    }

    #[test]
    fn ranking_orders_by_mean_and_marks_splits() {
        let a = lane(&[Some(0.9), Some(0.1), None, Some(0.6)], &[]);
        let b = lane(&[Some(0.7), Some(0.3), None, Some(0.2)], &[]);
        let rows = ranking(&[a, b], 4);
        assert_eq!(
            rows.iter().map(|r| r.file).collect::<Vec<_>>(),
            [0, 3, 1, 2]
        );
        assert!((rows[0].mean.unwrap() - 0.8).abs() < 1e-9);
        assert!(rows[1].disagree && !rows[0].disagree && !rows[2].disagree);
        assert_eq!(rows[3].mean, None);
        assert!(!disagree(&[Some(0.9), None]));
    }
}
