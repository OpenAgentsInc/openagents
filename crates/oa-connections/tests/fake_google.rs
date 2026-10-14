//! The Google OAuth flow and the Drive tools against a fake Google.

use std::sync::Arc;

use oa_connections::fake::{self, Content, FakeFile, FakeGoogle};
use oa_connections::google::drive::{
    self, Call, Drive, DriveError, LookAt, MediaKind, PdfText, Readers, Seen, Spent,
};
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
    let pdf = Readers {
        pdf: Some(pdf_text()),
        media: None,
    };
    let none = Readers::default();

    let listed = drive::run(
        &drive,
        &Call::parse("drive_list_folder", json!({"folder": FOLDER})).unwrap(),
        &none,
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
        &none,
    )
    .await
    .unwrap();
    assert_eq!(found.listed.len(), 1);
    assert_eq!(found.listed[0].id, DOC);

    let sheet = drive::run(&drive, &Call::Read { file: SHEET.into() }, &pdf)
        .await
        .unwrap();
    assert_eq!(sheet.result["format"], "csv");
    let text = sheet.result["text"].as_str().unwrap();
    assert!(text.contains("# October\nDate,Item,Amount\n2026-10-02,\"Groceries, weekly\",84.10\n"));
    assert!(text.contains("# September"));
    assert_eq!(sheet.read[0].name, "Spending 2026");

    let doc = drive::run(&drive, &Call::Read { file: DOC.into() }, &none)
        .await
        .unwrap();
    assert_eq!(doc.result["text"], "Keep dining under 200 a month.");

    let statement = drive::run(&drive, &Call::Read { file: PDF.into() }, &pdf)
        .await
        .unwrap();
    assert_eq!(statement.result["text"], "Statement: balance 1,204.55");
    // Without a PDF reader a PDF is refused, not guessed at.
    assert_eq!(
        drive.read(PDF, &none).await.unwrap_err(),
        DriveError::Unsupported("PDF".into())
    );
    // A folder isn't read as a file.
    assert!(matches!(
        drive.read(FOLDER, &none).await,
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
    let read = drive.read(SHEET, &Readers::default()).await.unwrap();
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
        drive
            .read("missing-file-00001", &Readers::default())
            .await
            .unwrap_err(),
        DriveError::NotFound
    );
}

const PHOTO: &str = "photo-devlog-000001";
const CLIP: &str = "video-devlog-000001";
const LONG: &str = "video-devlog-long-01";
const HUGE: &str = "video-devlog-huge-01";

/// A looker that reads the bytes it is lent and says what it got.
fn looker() -> LookAt {
    Arc::new(|drive: Drive, media: drive::Media| {
        Box::pin(async move {
            let bytes = drive
                .media_bytes(&media.file.id, 1024)
                .await
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(Seen {
                text: format!(
                    "A {} of {} bytes: {}",
                    media.kind.word(),
                    bytes.len(),
                    String::from_utf8_lossy(&bytes)
                ),
                spent: Spent {
                    input: 600,
                    output: 40,
                    usd_micros: 1_600,
                },
            })
        })
    })
}

/// Photos and videos come back described, with what looking cost; a
/// video too long or too large is refused before anything is fetched,
/// and without a looker they are refused, not guessed at.
#[tokio::test]
async fn photos_and_videos_are_described_within_their_caps() {
    let mut files = files();
    files.push(FakeFile::new(
        PHOTO,
        "IMG_7408.HEIC",
        Some(FOLDER),
        Content::Media("image/heic".into(), b"heic bytes".to_vec(), None),
    ));
    files.push(FakeFile::new(
        CLIP,
        "IMG_7389.MOV",
        Some(FOLDER),
        Content::Media(
            "video/quicktime".into(),
            b"mov bytes".to_vec(),
            Some(12_400),
        ),
    ));
    files.push(FakeFile::new(
        LONG,
        "Long.MOV",
        Some(FOLDER),
        Content::Media("video/quicktime".into(), b"x".to_vec(), Some(3_600_000)),
    ));
    let mut huge = FakeFile::new(
        HUGE,
        "Huge.MOV",
        Some(FOLDER),
        Content::Media("video/quicktime".into(), b"x".to_vec(), Some(1_000)),
    );
    huge.size = Some(drive::MAX_VIDEO_BYTES + 1);
    files.push(huge);
    let google = FakeGoogle::start(files, &[DRIVE_READONLY]).await;
    let drive = Drive::new(
        reqwest::Client::new(),
        google.endpoints(),
        fake::ACCESS.into(),
        false,
    );
    let readers = Readers {
        pdf: None,
        media: Some(looker()),
    };

    let listed = drive::run(
        &drive,
        &Call::parse("drive_list_folder", json!({"folder": FOLDER})).unwrap(),
        &readers,
    )
    .await
    .unwrap();
    let clip = listed.result["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "IMG_7389.MOV")
        .unwrap();
    assert_eq!(clip["kind"], "video");
    assert_eq!(clip["seconds"], 13);

    let photo = drive::run(&drive, &Call::Read { file: PHOTO.into() }, &readers)
        .await
        .unwrap();
    assert_eq!(photo.result["format"], "description");
    assert_eq!(photo.result["text"], "A photo of 10 bytes: heic bytes");
    assert_eq!(photo.read[0].media(), Some(MediaKind::Image));
    assert_eq!(photo.spent.map(|s| s.usd_micros), Some(1_600));

    let video = drive.read(CLIP, &readers).await.unwrap();
    assert_eq!(video.text, "A video of 9 bytes: mov bytes");
    assert!(video.note.unwrap().contains("video"));

    assert_eq!(
        drive.read(LONG, &readers).await.unwrap_err(),
        DriveError::TooLong
    );
    assert_eq!(
        drive.read(HUGE, &readers).await.unwrap_err(),
        DriveError::MediaTooLarge(MediaKind::Video, 500)
    );
    assert!(
        DriveError::TooLong.to_string().contains("too long"),
        "{}",
        DriveError::TooLong
    );
    // Neither refused file's bytes were fetched.
    assert!(
        !google
            .requests()
            .iter()
            .any(|r| (r.contains(LONG) || r.contains(HUGE)) && r.contains("alt=media"))
    );
    // Without a looker a video is refused.
    assert_eq!(
        drive.read(CLIP, &Readers::default()).await.unwrap_err(),
        DriveError::Unsupported("video".into())
    );
}
