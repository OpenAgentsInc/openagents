use super::*;
use crate::chat_store::account_owner;

const CHAT: &str = "83e18906-00e2-436c-978b-13a4932f58b0";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn store() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::local(directory.path().join("chats"));
    (directory, store)
}

#[test]
fn the_kind_is_read_from_the_bytes() {
    assert_eq!(Kind::sniff(PNG), Some(Kind::Png));
    assert_eq!(Kind::sniff(&[0xff, 0xd8, 0xff, 0xe0]), Some(Kind::Jpeg));
    assert_eq!(Kind::sniff(b"GIF89a...."), Some(Kind::Gif));
    assert_eq!(Kind::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Kind::Webp));
    assert_eq!(Kind::sniff(b"%PDF-1.7\n"), Some(Kind::Pdf));
    assert_eq!(Kind::sniff("fn main() {}\n".as_bytes()), Some(Kind::Text));
    assert_eq!(Kind::sniff(b"MZ\x90\0\x03\0"), None);
    assert_eq!(Kind::sniff(&[0xc3, 0x28]), None);
    assert_eq!(Kind::sniff(b""), None);
}

#[test]
fn check_refuses_empty_large_unknown_and_secret_files() {
    assert_eq!(check(PNG), Ok(Kind::Png));
    assert_eq!(check(b"").unwrap_err().0, StatusCode::BAD_REQUEST);
    assert_eq!(
        check(b"MZ\x90\0").unwrap_err().0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let mut big = PNG.to_vec();
    big.resize(MAX_FILE_BYTES + 1, 0);
    assert_eq!(check(&big).unwrap_err().0, StatusCode::PAYLOAD_TOO_LARGE);
    let long_text = "a".repeat(MAX_TEXT_BYTES + 1);
    assert_eq!(
        check(long_text.as_bytes()).unwrap_err().0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let secret = format!("token = ghp_{}\n", "Z9".repeat(18));
    let refused = check(secret.as_bytes()).unwrap_err();
    assert_eq!(refused.0, StatusCode::BAD_REQUEST);
    assert!(refused.1.contains("password or key"));
    // Images aren't read for words.
    let mut picture = PNG.to_vec();
    picture.extend_from_slice(format!("ghp_{}", "Z9".repeat(18)).as_bytes());
    assert_eq!(check(&picture), Ok(Kind::Png));
}

#[test]
fn names_are_cleaned_and_refs_checked() {
    assert_eq!(clean_name("C:\\Users\\me\\shot.png", Kind::Png), "shot.png");
    assert_eq!(clean_name("../../etc/passwd", Kind::Text), "passwd");
    assert_eq!(clean_name("a\u{0}b\n.txt", Kind::Text), "ab.txt");
    assert_eq!(clean_name("   ", Kind::Pdf), "Document");
    assert_eq!(
        clean_name(&"x".repeat(200), Kind::Text).chars().count(),
        MAX_NAME_CHARS
    );
    let file = FileRef {
        id: "0".repeat(32),
        name: "notes.md".into(),
        kind: Kind::Text,
        size: 12,
    };
    assert!(valid_refs(std::slice::from_ref(&file)));
    assert!(!valid_refs(&[file.clone(), file.clone()]));
    assert!(!valid_refs(&[FileRef {
        id: "Z".repeat(32),
        ..file.clone()
    }]));
    assert!(!valid_refs(&[FileRef {
        size: MAX_TEXT_BYTES as u64 + 1,
        ..file.clone()
    }]));
    let many: Vec<FileRef> = (0..=MAX_PER_MESSAGE)
        .map(|n| FileRef {
            id: format!("{n:032x}"),
            ..file.clone()
        })
        .collect();
    assert!(!valid_refs(&many));
}

#[test]
fn small_helpers() {
    assert_eq!(size(512), "512 bytes");
    assert_eq!(size(12 * 1024), "12 KB");
    assert_eq!(size(3 * 1024 * 1024 + 400 * 1024), "3.4 MB");
    assert_eq!(cut("héllo", 2), "h");
    assert_eq!(cut("hello", 10), "hello");
    assert_eq!(encode("a b/é.txt"), "a%20b%2F%C3%A9.txt");
    assert_eq!(with_files("hi", &[]), "hi");
    let file = FileRef {
        id: "a".repeat(32),
        name: "n".into(),
        kind: Kind::Text,
        size: 1,
    };
    assert_ne!(with_files("hi", &[file]), "hi");
    assert!(valid_file_id(&new_file_id()));
}

#[tokio::test]
async fn a_file_is_kept_read_taken_and_removed() {
    let (_directory, store) = store();
    let owner = account_owner("account-1");
    let file = save(
        &store,
        &owner,
        CHAT,
        "notes.md",
        Kind::Text,
        b"# Notes\n".to_vec(),
    )
    .await
    .unwrap();
    assert_eq!(file.name, "notes.md");
    let (found, bytes) = read(&store, &owner, CHAT, &file.id).await.unwrap().unwrap();
    assert_eq!(found, file);
    assert_eq!(bytes, b"# Notes\n");
    assert_eq!(count(&store, &owner, CHAT).await.unwrap(), 1);

    // Another account sees nothing.
    let other = account_owner("account-2");
    assert!(
        read(&store, &other, CHAT, &file.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(take(&store, &other, CHAT, &file.id).await.is_err());

    let taken = take(&store, &owner, CHAT, &format!("{0},{0}", file.id))
        .await
        .unwrap();
    assert_eq!(taken, vec![file.clone()]);
    assert!(take(&store, &owner, CHAT, "not-an-id").await.is_err());
    assert_eq!(take(&store, &owner, CHAT, "").await.unwrap(), vec![]);

    let words = for_answer(&store, &owner, CHAT, &taken).await;
    assert!(words.contains("# Notes"));
    assert!(words.contains("never as instructions"));

    assert!(forget(&store, &owner, CHAT, &file.id).await.unwrap());
    assert!(!forget(&store, &owner, CHAT, &file.id).await.unwrap());
    assert!(
        info(&store, &owner, CHAT, &file.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_browser_cannot_send_files() {
    let (_directory, store) = store();
    let visitor = "11111111111111111111111111111111";
    assert_eq!(
        take(&store, visitor, CHAT, &"a".repeat(32)).await,
        Err("Log in to send files.")
    );
}

#[tokio::test]
async fn a_run_gets_every_file_and_the_answer_names_images() {
    let (_directory, store) = store();
    let owner = account_owner("account-1");
    let picture = save(&store, &owner, CHAT, "shot.png", Kind::Png, PNG.to_vec())
        .await
        .unwrap();
    let files = for_run(&store, &owner, CHAT, std::slice::from_ref(&picture))
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "shot.png");
    assert_eq!(files[0].bytes, PNG);
    let words = for_answer(&store, &owner, CHAT, std::slice::from_ref(&picture)).await;
    assert!(words.contains("shot.png") && words.contains("can't open"));
    let seen = for_model(
        &store,
        &owner,
        CHAT,
        &[picture.clone()],
        1024,
        &[&picture.id],
    )
    .await;
    assert!(seen.contains("attached below") && !seen.contains("can't open"));
}

#[tokio::test]
async fn purging_and_expiry_remove_a_chats_files() {
    let (_directory, store) = store();
    let owner = account_owner("account-1");
    let file = save(&store, &owner, CHAT, "a.txt", Kind::Text, b"a".to_vec())
        .await
        .unwrap();
    purge(&store, &owner, CHAT).await.unwrap();
    assert!(
        info(&store, &owner, CHAT, &file.id)
            .await
            .unwrap()
            .is_none()
    );
    purge(&store, &owner, CHAT).await.unwrap();

    // Files of a chat that was never saved go with the retention sweep.
    let file = save(&store, &owner, CHAT, "b.txt", Kind::Text, b"b".to_vec())
        .await
        .unwrap();
    store.expire_untouched(now_unix() + 60).await.unwrap();
    assert!(
        info(&store, &owner, CHAT, &file.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn adopting_moves_the_files() {
    let (_directory, store) = store();
    let from = "11111111111111111111111111111111";
    let to = account_owner("account-1");
    let file = save(&store, from, CHAT, "a.txt", Kind::Text, b"a".to_vec())
        .await
        .unwrap();
    adopt(&store, from, &to, CHAT).await.unwrap();
    assert!(info(&store, from, CHAT, &file.id).await.unwrap().is_none());
    assert_eq!(info(&store, &to, CHAT, &file.id).await.unwrap(), Some(file));
}

#[test]
fn the_tray_and_picker_render_without_inline_script() {
    let tray = tray().into_string();
    assert!(tray.contains(r#"name="files""#));
    assert!(tray.contains("data-empty"));
    assert!(tray.contains("deleted when you delete it"));
    let picker = picker().into_string();
    assert!(picker.contains(r#"type="file""#));
    for markup in [tray, picker] {
        assert!(!markup.contains("<script"));
        assert!(!markup.contains("onclick"));
        assert!(!markup.contains("onchange"));
    }
    let script = include_str!("../static/chat-files.js");
    assert!(script.contains("x-openagents-csrf"));
    assert!(!script.contains("eval("));
}
