//! A standalone SDK consumer. No server, GUI, engine, or global CLI credentials.
use openagents::{
    CancellationToken, Client, CredentialRecord, CredentialStore, Deployment, MessageRole, Session,
    StorageError,
};
use std::{error::Error, sync::Mutex};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Default)]
struct Memory(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Memory {
    fn load(&self, _: &Deployment) -> std::result::Result<Option<CredentialRecord>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, record: &CredentialRecord) -> std::result::Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(record.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> std::result::Result<(), StorageError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
#[derive(Default)]
struct Args {
    api: Option<String>,
    website: Option<String>,
    token_env: Option<String>,
    chat: Option<String>,
    project: Option<String>,
    no_browser: bool,
    verify: bool,
}
impl Args {
    fn read() -> Result<Self> {
        let mut args = Self::default();
        let mut input = std::env::args().skip(1);
        while let Some(flag) = input.next() {
            match flag.as_str() {
                "--api" => args.api = Some(input.next().ok_or("--api requires a URL")?),
                "--website" => args.website = Some(input.next().ok_or("--website requires a URL")?),
                "--token-env" => {
                    args.token_env =
                        Some(input.next().ok_or("--token-env requires a variable name")?)
                }
                "--chat" => {
                    args.chat = Some(input.next().ok_or("--chat requires an existing chat ID")?)
                }
                "--project" => {
                    args.project = Some(
                        input
                            .next()
                            .ok_or("--project requires a shared project ID")?,
                    )
                }
                "--no-browser" => args.no_browser = true,
                "--verify" => args.verify = true,
                "--help" => {
                    println!(
                        "SDK history example [--api URL --website URL] [--chat ID] [--project ID] [--no-browser] [--token-env NAME] [--verify]"
                    );
                    std::process::exit(0);
                }
                _ => return Err("Unknown option; use --help".into()),
            }
        }
        Ok(args)
    }
    fn deployment(&self) -> Result<Deployment> {
        match (&self.api, &self.website) {
            (None, None) => Ok(Deployment::production()),
            (Some(api), Some(site)) => Ok(Deployment::new(api, site)?),
            _ => Err("Set both --api and --website".into()),
        }
    }
}
fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = url;
    }
}
async fn login(client: &Client, args: &Args, cancel: &CancellationToken) -> Result<Session> {
    if let Some(name) = &args.token_env {
        // The caller explicitly selects this input; the SDK reads no environment.
        let token = std::env::var(name).map_err(|_| "The selected token variable isn't set")?;
        return Ok(client.authenticate(&token).await?);
    }
    let mut login = client
        .start_login("SDK history example", "SDK example")
        .await?;
    println!(
        "Approve {} at {}",
        login.user_code(),
        login.verification_url()
    );
    if !args.no_browser {
        open_browser(login.verification_url().as_str());
    }
    Ok(client.wait_for_login(&mut login, cancel).await?)
}
async fn browse(
    client: &Client,
    session: &Session,
    args: &Args,
    cancel: &CancellationToken,
) -> Result<()> {
    let account = client.account(session).await?;
    if !args.verify {
        println!("Signed in as {}", account.account.label);
    }
    let mut projects = Vec::new();
    let mut directory = client.history_projects(session)?.with_page_size(200)?;
    while let Some(page) = directory.next_page(client, session, cancel).await? {
        projects.extend(page.data);
    }
    let mut reader = if let Some(id) = &args.project {
        let project = projects
            .iter()
            .find(|p| p.id() == id && p.shared)
            .ok_or("That shared project isn't available")?;
        client.shared_history(session, project)?
    } else {
        client.history(session)?
    }
    .with_page_size(if args.verify { 1 } else { 200 })?;
    let mut index = client.history_index(session)?;
    while let Some(page) = reader.next_page(client, session, cancel).await? {
        index.extend(page.data)?;
    }
    let rows = index
        .sorted(false)
        .into_iter()
        .chain(index.sorted(true))
        .collect::<Vec<_>>();
    if !args.verify {
        for row in &rows {
            println!(
                "{}  {}{}",
                row.chat.id(),
                row.title,
                if row.archived_unix.is_some() {
                    " (archived)"
                } else {
                    ""
                }
            );
        }
        for project in projects.iter().filter(|p| p.shared) {
            println!("Shared project: {}  {}", project.id(), project.name);
        }
    }
    let selected = if args.verify {
        rows.iter()
            .filter(|r| r.title == "SDK history fixture")
            .map(|r| r.chat.clone())
            .collect::<Vec<_>>()
    } else if let Some(id) = &args.chat {
        vec![client.chat(session, id)?]
    } else {
        rows.first()
            .map(|r| vec![r.chat.clone()])
            .unwrap_or_default()
    };
    if args.verify && selected.len() != 3 {
        return Err("Expected the three dedicated-account fixtures".into());
    }
    let mut total = 0;
    let mut files = 0;
    for chat in &selected {
        let mut transcript = client.transcript(session, chat)?.with_page_size(200)?;
        let (mut count, mut thinking, mut tools) = (0, 0, 0);
        while let Some(page) = transcript.next_page(client, session, cancel).await? {
            let first_page = count == 0;
            if !args.verify && first_page {
                println!("\n{}", page.thread.title);
            }
            for message in page.data {
                if message.id != format!("msg_{count}") {
                    return Err("Messages were not in order".into());
                }
                count += 1;
                if message.role == MessageRole::Thinking {
                    thinking += 1;
                }
                if !args.verify {
                    println!("\n{}\n{}", message.role.as_str(), message.text);
                }
                if let Some(tool) = message.tool {
                    tools += 1;
                    if args.verify {
                        assert_eq!(tool.output, "Saved fixture output");
                    } else {
                        println!(
                            "{} · {}\nInput:\n{}\nOutput:\n{}",
                            tool.name,
                            tool.status.as_str(),
                            tool.input,
                            tool.output
                        );
                    }
                }
                for file in message.files {
                    if let Some(reference) = file.download_ref() {
                        let bytes = client.download(session, reference, cancel).await?;
                        files += 1;
                        if !args.verify {
                            println!("Saved file: {} ({} bytes)", file.name, bytes.len());
                        }
                    }
                }
                if !args.verify && !message.extra.is_empty() {
                    println!("More saved details: {:?}", message.extra);
                }
            }
            // Captures describe the transcript, so read each only on the first page.
            if first_page {
                for capture in page.captures {
                    if let Some(reference) = capture.download_ref() {
                        let bytes = client.download(session, reference, cancel).await?;
                        files += 1;
                        if !args.verify {
                            println!("Saved capture: {} ({} bytes)", capture.name, bytes.len());
                        }
                    } else if !args.verify {
                        println!("Capture unavailable: {}", capture.name);
                    }
                }
            }
        }
        if args.verify {
            assert_eq!(count, 205);
            assert_eq!(thinking, 1);
            assert_eq!(tools, 1);
        }
        total += count;
    }
    println!(
        "Read {} chats, {total} messages, and {files} saved files.",
        selected.len()
    );
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::read()?;
    let client = Client::new(args.deployment()?)?;
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        signal.cancel();
    });
    let store = Memory::default();
    let session = login(&client, &args, &cancel).await?;
    // Save the known issued credential before account discovery or other requests.
    store.save(&session.record())?;
    let record = store
        .load(client.deployment())?
        .ok_or("Login wasn't saved")?;
    let outcome = async {
        // A second client demonstrates explicit restoration of the caller's store.
        let restarted = Client::new(args.deployment()?)?;
        let restored = restarted.restore(&record).await?;
        browse(&restarted, &restored, &args, &cancel).await
    }
    .await;
    let revoked = client.sign_out_record(&record).await;
    store.clear(client.deployment())?;
    outcome?;
    revoked?;
    println!("Signed out and cleared this example's login.");
    Ok(())
}
