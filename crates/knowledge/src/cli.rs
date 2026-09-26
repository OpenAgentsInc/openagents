//! The `kb` command: `search`, `show`, and `lint`.

use std::path::PathBuf;

use crate::lint::{Corpus, default_corpora, lint};
use crate::search::{OpenRouterEmbedder, Retriever};
use crate::{Base, default_cache, default_dir};

/// The `kb` command's help.
pub const USAGE: &str = "usage: kb <command> [options]

Commands:
  search <text>   list the entries that best match the text, with their scores
  show <id>       print one entry in full, with its digest
  lint            check every entry: its fields, its citation, and that it
                  neither names nor quotes a benchmark task

Options:
  --dir DIR       the knowledge directory (default OPENAGENTS_KNOWLEDGE, or
                  knowledge/ in the checkout this binary was built from)
  --candidates    include candidate entries, not only admitted ones
  --limit N       search results to list (default 10)
  --corpus DIR    a directory of benchmark tasks the lint checks against; it
                  can repeat (default Terminal-Bench 4 under ~/.openagents)
  --lexical       search by words alone, without embeddings";

/// Runs `kb` with `args` and returns the exit code: 0 on success, 1 when
/// the lint finds problems or an entry isn't there, 2 on bad usage.
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
    let mut dir = default_dir();
    let mut candidates = false;
    let mut limit = 10usize;
    let mut corpora: Vec<PathBuf> = Vec::new();
    let mut lexical = false;
    let mut words: Vec<String> = Vec::new();
    let mut iter = args.iter();
    let command = iter.next().ok_or(USAGE)?.clone();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--dir" => dir = PathBuf::from(value()?),
            "--candidates" => candidates = true,
            "--limit" => {
                let text = value()?;
                limit = text
                    .parse()
                    .map_err(|_| format!("--limit wants a number, not {text}"))?;
            }
            "--corpus" => corpora.push(PathBuf::from(value()?)),
            "--lexical" => lexical = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag}\n\n{USAGE}"));
            }
            word => words.push(word.to_string()),
        }
    }
    match command.as_str() {
        "search" => {
            if words.is_empty() {
                return Err("kb search needs the text to search for".to_string());
            }
            let base = Base::load(&dir, candidates)?;
            let query = words.join(" ");
            let retriever = if lexical {
                Retriever::lexical(base, "--lexical was given")
            } else {
                match OpenRouterEmbedder::from_env() {
                    Ok(embedder) => Retriever::new(base, embedder, default_cache()),
                    Err(error) => Retriever::lexical(base, &error),
                }
            };
            let search = retriever.search(&query, limit).await;
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
                    "{:>2}. {:.3}  {}  [{}, {}]  words {:.2}{}  {}",
                    rank + 1,
                    hit.score,
                    hit.id,
                    entry.kind,
                    entry.status,
                    hit.lexical,
                    hit.semantic
                        .map_or(String::new(), |s| format!(" · cosine {s:.3}")),
                    entry.title
                );
            }
            Ok(0)
        }
        "show" => {
            let [id] = words.as_slice() else {
                return Err("kb show needs one entry ID".to_string());
            };
            let (entries, _) = Base::read(&dir);
            let Some(entry) = entries.iter().find(|e| &e.id == id) else {
                eprintln!("no entry {id} in {}", dir.display());
                return Ok(1);
            };
            let path = dir.join(format!("{id}.md"));
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("can't read {}: {e}", path.display()))?;
            println!("{}\n{}", entry.digest, text.trim_end());
            Ok(0)
        }
        "lint" => {
            let (entries, mut problems) = Base::read(&dir);
            if corpora.is_empty() {
                corpora = default_corpora();
            }
            let corpus = Corpus::read(&corpora);
            for absent in &corpus.absent {
                println!("skipped {}: it isn't there", absent.display());
            }
            problems.extend(lint(&entries, &corpus));
            println!(
                "checked {} entries in {} against {} tasks and {} test files",
                entries.len(),
                dir.display(),
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
        other => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}
