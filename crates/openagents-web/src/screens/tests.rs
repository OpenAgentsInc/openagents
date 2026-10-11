use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode, header};
use openagents_ui::shell::Theme;

use super::*;

async fn body(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

/// Every id is unique and every state renders as a whole page in both
/// themes, framable by this site, with no machine talk.
#[tokio::test]
async fn every_state_renders_in_both_themes() {
    let all = screens();
    let mut ids = std::collections::BTreeSet::new();
    for screen in &all {
        assert!(ids.insert(screen.id), "duplicate screen {}", screen.id);
        assert!(!screen.states.is_empty(), "{} has no states", screen.id);
        let mut states = std::collections::BTreeSet::new();
        for state in &screen.states {
            assert!(
                states.insert(state.id),
                "{}: duplicate {}",
                screen.id,
                state.id
            );
            for theme in [Theme::Light, Theme::Dark] {
                let route = format!("{PAGE}/{}/{}?theme={}", screen.id, state.id, theme.as_str());
                let response = render(state, &HeaderMap::new(), Some(theme));
                assert!(
                    !response.status().is_redirection(),
                    "{route}: {}",
                    response.status()
                );
                if let Some(policy) = response.headers().get(header::CONTENT_SECURITY_POLICY) {
                    let policy = policy.to_str().unwrap();
                    assert!(
                        !policy.contains("frame-ancestors 'none'"),
                        "{route}: {policy}"
                    );
                }
                let html = body(response).await;
                assert!(
                    html.contains(&format!("data-theme=\"{}\"", theme.as_str())),
                    "{route}: theme"
                );
                crate::copy_guard::assert_plain(&route, &html);
            }
        }
    }
}

/// The index lists every screen, and a screen page frames every state.
#[tokio::test]
async fn the_index_and_screen_pages_list_everything() {
    let all = screens();
    let html = body(index(HeaderMap::new()).await).await;
    for screen in &all {
        assert!(
            html.contains(&screen_href(screen)),
            "index lacks {}",
            screen.id
        );
    }
    crate::copy_guard::assert_plain(PAGE, &html);
    let first = &all[0];
    let response = screen(HeaderMap::new(), Path(first.id.to_owned())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let policy = response.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(policy.contains("frame-src 'self'"), "{policy}");
    let html = body(response).await;
    for state in &first.states {
        assert!(
            html.contains(&state_href(first, state, Theme::Dark)),
            "{}",
            state.id
        );
    }
    let missing = screen(HeaderMap::new(), Path("nope".to_owned())).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
