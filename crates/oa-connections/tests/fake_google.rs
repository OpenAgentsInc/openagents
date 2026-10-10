//! The Google OAuth flow and the Drive tools against a fake Google.

use std::sync::Arc;

use oa_connections::fake::{self, Content, FakeFile, FakeGoogle};
use oa_connections::google::drive::{self, Call, Drive, DriveError, PdfText};
use oa_connections::google::oauth::{self, Client, OAuthError, Pkce};
use oa_connections::google::{DRIVE_READONLY, SHEETS_READONLY};
use serde_json::json;

const FOLDER: &str = "folder-finances-0001";
const SHEET: &str = "sheet-spending-00001";
const DOC: &str = "doc-budget-notes-0001";
const PDF: &str = "pdf-statement-000001";

fn files() -> Vec<FakeFile> {
    vec![
        FakeFile::new(FOLDER, "Finances", None, Content::Folder),
        FakeFile::new(
            SHEET,
            "Spending 2026",
            Some(FOLDER),
            Content::Sheet(vec![
                (
                    "October".into(),
                    vec![
                        vec!["Date".into(), "Item".into(), "Amount".into()],
                        vec![
                            "2026-10-02".into(),
                            "Groceries, weekly".into(),
                            "84.10".into(),
                        ],
                    ],
                ),
                (
                    "September".into(),
                    vec![vec!["Date".into(), "Item".into(), "Amount".into()]],
                ),
            ]),
        ),
        FakeFile::new(
            DOC,
            "Budget notes",
            Some(FOLDER),
            Content::Doc("Keep dining under 200 a month.".into()),
        ),
        FakeFile::new(
            PDF,
            "Bank statement",
            Some(FOLDER),
            Content::Pdf(b"%PDF-1.7 fake".to_vec()),
        ),
    ]
}

fn client() -> Client {
    Client::new("client.apps".into(), "client-secret".into())
}

fn pdf_text() -> PdfText {
    Arc::new(|bytes: Vec<u8>| {
        Box::pin(async move {
            assert!(bytes.starts_with(b"%PDF"));
            Ok::<_, String>("Statement: balance 1,204.55".to_owned())
        })
    })
}

#[tokio::test]
async fn connecting_exchanges_the_code_and_refreshes_and_revokes() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let endpoints = google.endpoints();
    let http = reqwest::Client::new();
    let pkce = Pkce::new();
    let grant = oauth::exchange(
        &http,
        &endpoints,
        &client(),
        "https://openagents.test/auth/google/callback",
        fake::CODE,
        &pkce.verifier,
    )
    .await
    .unwrap();
    assert_eq!(grant.refresh_token.as_deref(), Some(fake::REFRESH));
    assert_eq!(grant.email.as_deref(), Some(fake::EMAIL));
    assert!(grant.scopes.iter().any(|s| s == DRIVE_READONLY));
    assert!(drive::has_sheets(&grant.scopes));
    // Debug never shows a token.
    assert!(!format!("{grant:?}").contains("fake-refresh"));

    let access = oauth::refresh(&http, &endpoints, &client(), fake::REFRESH)
        .await
        .unwrap();
    assert_eq!(access.token, fake::ACCESS);

    assert!(oauth::revoke(&http, &endpoints, fake::REFRESH).await);
    assert_eq!(
        oauth::refresh(&http, &endpoints, &client(), fake::REFRESH)
            .await
            .unwrap_err(),
        OAuthError::Refused
    );
    assert_eq!(
        oauth::exchange(&http, &endpoints, &client(), "x", "wrong", &pkce.verifier)
            .await
            .err(),
        Some(OAuthError::Refused)
    );
}

#[tokio::test]
async fn the_tools_search_list_and_read_docs_sheets_and_pdfs() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let drive = Drive::new(
        reqwest::Client::new(),
        google.endpoints(),
        fake::ACCESS.into(),
        true,
    );
    let pdf = pdf_text();

    let listed = drive::run(
        &drive,
        &Call::parse("drive_list_folder", json!({"folder": FOLDER})).unwrap(),
        None,
    )
    .await
    .unwrap();
    let names: Vec<&str> = listed.result["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Bank statement", "Budget notes", "Spending 2026"]);

    let found = drive::run(
        &drive,
        &Call::parse("drive.search", json!({"query": "dining"})).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(found.listed.len(), 1);
    assert_eq!(found.listed[0].id, DOC);

    let sheet = drive::run(&drive, &Call::Read { file: SHEET.into() }, Some(&pdf))
        .await
        .unwrap();
    assert_eq!(sheet.result["format"], "csv");
    let text = sheet.result["text"].as_str().unwrap();
    assert!(text.contains("# October\nDate,Item,Amount\n2026-10-02,\"Groceries, weekly\",84.10\n"));
    assert!(text.contains("# September"));
    assert_eq!(sheet.read[0].name, "Spending 2026");

    let doc = drive::run(&drive, &Call::Read { file: DOC.into() }, None)
        .await
        .unwrap();
    assert_eq!(doc.result["text"], "Keep dining under 200 a month.");

    let statement = drive::run(&drive, &Call::Read { file: PDF.into() }, Some(&pdf))
        .await
        .unwrap();
    assert_eq!(statement.result["text"], "Statement: balance 1,204.55");
    // Without a PDF reader a PDF is refused, not guessed at.
    assert_eq!(
        drive.read(PDF, None).await.unwrap_err(),
        DriveError::Unsupported("PDF".into())
    );
    // A folder isn't read as a file.
    assert!(matches!(
        drive.read(FOLDER, None).await,
        Err(DriveError::Invalid(_))
    ));
}

#[tokio::test]
async fn without_sheets_access_only_the_first_tab_is_read_and_a_bad_token_is_refused() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY]).await;
    let drive = Drive::new(
        reqwest::Client::new(),
        google.endpoints(),
        fake::ACCESS.into(),
        false,
    );
    let read = drive.read(SHEET, None).await.unwrap();
    assert_eq!(read.format, "csv");
    assert!(read.text.contains("Groceries"));
    assert!(!read.text.contains("September"));
    assert!(read.note.is_some());
    assert!(
        !google
            .requests()
            .iter()
            .any(|r| r.contains("/v4/spreadsheets"))
    );

    let stale = Drive::new(
        reqwest::Client::new(),
        google.endpoints(),
        "ya29.stale".into(),
        false,
    );
    assert_eq!(
        stale.list_folder(FOLDER, None).await.unwrap_err(),
        DriveError::Unauthorized
    );
    assert_eq!(
        drive.read("missing-file-00001", None).await.unwrap_err(),
        DriveError::NotFound
    );
}
