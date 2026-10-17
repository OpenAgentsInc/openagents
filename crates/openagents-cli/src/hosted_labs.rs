//! Account Labs use the SDK; local commands remain for unhosted kinds.
use crate::{Args, Output};
use lab_terminal::{CancellationToken, Graphics, Hosted};
use openagents_login::Saved;
use serde_json::{Value, json};
use std::io::IsTerminal;

fn saved_from(
    get: impl Fn(&str) -> Option<String>,
    load: impl FnOnce() -> Option<Saved>,
) -> Result<Option<Saved>, String> {
    if let Some(saved) = Saved::from_env(&get) {
        return Ok(Some(saved));
    }
    if openagents_login::token_env_set(&get) {
        return Err(
            "OPENAGENTS_TOKEN isn't a login key. Set a sign-in key or session token, or unset it."
                .into(),
        );
    }
    Ok(load())
}
fn saved() -> Result<Option<Saved>, String> {
    saved_from(
        |name| std::env::var(name).ok(),
        || Saved::load_file(&crate::keys::sign_in_dir(None)),
    )
}
async fn connect(saved: &Saved, space: &str, cancel: &CancellationToken) -> Result<Hosted, String> {
    Hosted::connect(
        &saved.origin,
        saved.token(),
        space,
        std::env::var("OPENAGENTS_TEST_RUN").as_deref() == Ok("1"),
        cancel,
    )
    .await
}
fn graphics(output: &Output) -> Option<Graphics> {
    if output.json() || !std::io::stdout().is_terminal() {
        return None;
    }
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        Some(Graphics::Kitty)
    } else if std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app") {
        Some(Graphics::Iterm2)
    } else {
        None
    }
}
async fn emit(
    output: &Output,
    hosted: &Hosted,
    value: &Value,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let text = hosted.render(value)?;
    let mut image = String::new();
    if let Some(graphics) = graphics(output) {
        if let Ok(card) = serde_json::from_value(value["card"].clone()) {
            // The document and text remain useful if its preview isn't ready.
            if let Ok(rendered) = hosted.images(&card, graphics, cancel).await {
                image = rendered;
            }
        }
    }
    output.emit(value, |_| format!("{text}\n{image}").trim_end().into());
    Ok(())
}
/// `None` means this kind has no hosted implementation. Errors and unsupported
/// hosted tools are handled here, rather than calling a different local Lab.
pub fn try_run(output: &Output, kind: &str, name: &str, args: &Value) -> Option<u8> {
    let saved = match saved() {
        Ok(Some(saved)) => saved,
        Ok(None) => return None,
        Err(e) => return Some(output.fail(kind, &e)),
    };
    let cancel = CancellationToken::new();
    let result=crate::runtime().block_on(async {
        tokio::select! {
            result=async {
                let hosted=connect(&saved,"me",&cancel).await?;
                if !hosted.registry.hosts(kind) {return Ok(None);}
                let value=hosted.invoke(kind,name,args.clone(),&cancel).await?;
                emit(output,&hosted,&value,&cancel).await?;
                Ok::<_,String>(Some(if value["refused"]==true || value["passed"]==false {1}else{0}))
            }=>result,
            _=tokio::signal::ctrl_c()=> {cancel.cancel();Err("Canceled. Check the document before repeating an edit.".into())}
        }
    });
    match result {
        Ok(code) => code,
        Err(error) => Some(output.fail(kind, &error)),
    }
}

pub fn command(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(e) => return output.usage("lab", &e, crate::lab::USAGE),
    };
    let p = args.positional();
    let command = p.first().map(String::as_str).unwrap_or_default();
    let allowed: &[&str] = match command {
        "kinds" => &["space"],
        "tool" => &["space", "json"],
        "watch" => &["space", "since", "count"],
        _ => return output.usage("lab", "Choose kinds, tool, or watch.", crate::lab::USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|n| !allowed.contains(n))
    {
        return output.usage(
            "lab",
            &format!("Unknown option --{name}."),
            crate::lab::USAGE,
        );
    }
    if (command == "kinds" && p.len() != 1)
        || (command == "tool" && p.len() != 3)
        || (command == "watch" && p.len() != 2)
    {
        return output.usage("lab", "Check the command arguments.", crate::lab::USAGE);
    }
    let saved = match saved() {
        Ok(Some(saved)) => saved,
        Ok(None) => return output.fail("lab", "Sign in with coder login to use account Labs."),
        Err(e) => return output.fail("lab", &e),
    };
    let cancel = CancellationToken::new();
    let result = crate::runtime().block_on(async {
        tokio::select! {
            result = account_command(output, &saved, command, p, &args, &cancel) => result,
            _ = tokio::signal::ctrl_c() => {cancel.cancel(); Err("Canceled.".into())}
        }
    });
    match result {
        Ok(code) => code,
        Err(e) => output.fail("lab", &e),
    }
}

async fn account_command(
    output: &Output,
    saved: &Saved,
    command: &str,
    positional: &[String],
    args: &Args,
    cancel: &CancellationToken,
) -> Result<u8, String> {
    let hosted = connect(saved, args.option("space").unwrap_or("me"), cancel).await?;
    match command {
        "kinds" => {
            let value = json!({"kinds":hosted.registry.kinds});
            let text = hosted
                .registry
                .kinds
                .iter()
                .map(|kind| {
                    lab_terminal::clean(&format!(
                        "{}: {}\n  {}",
                        kind.kind,
                        kind.name,
                        kind.tools
                            .iter()
                            .map(|tool| tool.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n");
            output.emit(&value, |_| text);
        }
        "tool" => {
            let value: Value = serde_json::from_str(args.option("json").unwrap_or("{}"))
                .map_err(|_| "--json needs a JSON object.".to_owned())?;
            if !value.is_object() {
                return Err("--json needs a JSON object.".into());
            }
            let result = hosted
                .invoke(&positional[1], &positional[2], value, cancel)
                .await?;
            emit(output, &hosted, &result, cancel).await?;
            if result["refused"] == true || result["passed"] == false {
                return Ok(1);
            }
        }
        "watch" => {
            let number = |name: &str, default: u64| -> Result<u64, String> {
                args.option(name).map_or(Ok(default), |n| {
                    n.parse()
                        .map_err(|_| format!("--{name} needs a whole number."))
                })
            };
            let since = number("since", 0)?;
            let count = number("count", 1)?;
            if !(1..=1000).contains(&count) {
                return Err("--count must be between 1 and 1000.".into());
            }
            let mut events = hosted.events(&positional[1], since)?;
            for _ in 0..count {
                let event = events.next(cancel).await.map_err(|e| e.to_string())?;
                let mut latest = hosted.latest(&positional[1], cancel).await?;
                latest["event"] = json!(event);
                latest["cursor"] = json!(events.cursor());
                emit(output, &hosted, &latest, cancel).await?;
            }
        }
        _ => unreachable!(),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_bad_credentials_never_load_another_account() {
        assert!(
            saved_from(
                |name| (name == "OPENAGENTS_TOKEN").then(|| "oak_not_a_login".into()),
                || panic!("must not load saved login")
            )
            .is_err()
        );
        assert!(saved_from(|_| None, || None).unwrap().is_none());
    }
    #[test]
    fn existing_terminal_parsers_keep_document_and_version_arguments() {
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        let (name, args) = crate::cad::request(&words("open bracket --version 3")).unwrap();
        assert_eq!(name, "cad_open");
        assert_eq!(args, json!({"doc":"bracket","version":3}));
        let (name, args) = crate::lab::request(&words("open film --version 2")).unwrap();
        assert_eq!(name, "lab_open");
        assert_eq!(args["document"], "film");
    }
}
