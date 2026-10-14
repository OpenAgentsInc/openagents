//! `oa-solver-box`: runs Solver on one GitHub issue on a Google Cloud box
//! (#11560). Each progress line is printed as JSON as it comes, then the
//! result. Claiming runs from the website is the dispatcher's (#11562).

use std::process::ExitCode;

use solver_box::{Credentials, GoogleToken, Run, parse_issue};

const USAGE: &str = "Usage:
  oa-solver-box --local-issue OWNER/NAME#N [--model MODEL] [--image IMAGE] [--result FILE] [--metadata-token]
  oa-solver-box --serve [--site URL] [--name NAME] [--slots N] [--once] [--model MODEL] [--image IMAGE] [--metadata-token]

--local-issue runs Solver on the issue in the solver container on a box of
its own from our Box service, prints each progress line and the result as
JSON, and deletes the box.

--serve is the box dispatcher: it takes solver runs from the site (default
https://openagents.com) as work host NAME (default this computer's name),
SLOTS at a time (default 2), runs each on its own box, and reports its
progress and result; --once stops when nothing is waiting. The host token is
OA_WORK_HOST_TOKEN, else the openagents-work-host-token secret.

The image is OA_SOLVER_IMAGE when set. Reads BOAT_API_KEY (and
BOAT_API_BASE), the GitHub token from OA_SOLVER_GH_TOKEN or `gh auth token`
(with --serve, only for requesters who have not connected GitHub). Model
calls go to Vertex AI with a Google access token from gcloud, or the
metadata server with --metadata-token.";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut issue = None;
    let mut model = None;
    let mut image = None;
    let mut result_file = None;
    let mut google = GoogleToken::Gcloud;
    let mut serve = false;
    let mut once = false;
    let mut site = None;
    let mut name = None;
    let mut slots = 2usize;
    let mut words = args.iter();
    while let Some(arg) = words.next() {
        let mut value = || words.next().cloned();
        match arg.as_str() {
            "--serve" => serve = true,
            "--once" => once = true,
            "--site" => site = value(),
            "--name" => name = value(),
            "--slots" => match value().and_then(|n| n.parse().ok()) {
                Some(n) => slots = n,
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "--local-issue" => issue = value(),
            "--model" => model = value(),
            "--image" => image = value(),
            "--result" => result_file = value(),
            "--metadata-token" => google = GoogleToken::Metadata,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if serve {
        return serve_runs(Serve {
            site,
            name,
            slots,
            once,
            model,
            image,
            google,
        })
        .await;
    }
    let Some((repo, number)) = issue.as_deref().and_then(parse_issue) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut run = Run::new(&repo, number);
    if let Some(model) = model {
        run.model = model;
    }
    if let Some(image) = image {
        run.image = image;
    }
    let Some(github_token) = github_token() else {
        eprintln!("No GitHub token: set OA_SOLVER_GH_TOKEN or sign in with gh.");
        return ExitCode::from(2);
    };
    let credentials = Credentials {
        github_token,
        openagents_token: std::env::var("OPENAGENTS_TOKEN")
            .ok()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty()),
    };
    let client = match boat::Client::from_env().await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("The Box service is not reachable: {e}");
            return ExitCode::from(2);
        }
    };
    let result = solver_box::run(&client, &run, &credentials, &google, |progress| {
        println!("{}", progress.json());
    })
    .await;
    println!("{}", serde_json::json!({"result": result}));
    if let Some(path) = result_file
        && let Err(e) = std::fs::write(
            &path,
            serde_json::to_vec_pretty(&result).unwrap_or_default(),
        )
    {
        eprintln!("Could not write {path}: {e}");
    }
    if result["ok"] == true {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn github_token() -> Option<String> {
    if let Ok(token) = std::env::var("OA_SOLVER_GH_TOKEN")
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_owned());
    }
    let out = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !token.is_empty()).then_some(token)
}

struct Serve {
    site: Option<String>,
    name: Option<String>,
    slots: usize,
    once: bool,
    model: Option<String>,
    image: Option<String>,
    google: GoogleToken,
}

async fn serve_runs(serve: Serve) -> ExitCode {
    let Some(token) = host_token() else {
        eprintln!("No work host token: set OA_WORK_HOST_TOKEN.");
        return ExitCode::from(2);
    };
    let name = serve.name.unwrap_or_else(|| {
        std::process::Command::new("hostname")
            .arg("-s")
            .output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "solver-box".into())
    });
    let base = serve
        .site
        .unwrap_or_else(|| "https://openagents.com".into());
    let site = match solver_box::dispatch::Site::new(&base, &name, &token) {
        Ok(site) => site,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let boat = match boat::Client::from_env().await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("The Box service is not reachable: {e}");
            return ExitCode::from(2);
        }
    };
    let dispatcher = solver_box::dispatch::Dispatcher {
        boat,
        google: serve.google,
        pool_github_token: github_token(),
        model: serve.model,
        image: serve.image,
    };
    eprintln!(
        "Taking solver runs from {base} as {name}, {} at a time.",
        serve.slots
    );
    dispatcher.serve(&site, serve.slots, serve.once).await;
    ExitCode::SUCCESS
}

/// The work host token: `OA_WORK_HOST_TOKEN`, else Secret Manager's
/// `openagents-work-host-token`. Never printed.
fn host_token() -> Option<String> {
    if let Ok(token) = std::env::var("OA_WORK_HOST_TOKEN")
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_owned());
    }
    let out = std::process::Command::new("gcloud")
        .args([
            "secrets",
            "versions",
            "access",
            "latest",
            "--secret",
            "openagents-work-host-token",
            "--project",
            "openagentsgemini",
        ])
        .output()
        .ok()?;
    let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !token.is_empty()).then_some(token)
}
