use crate::server::config::AppState;
use crate::server::models::user::User;
use crate::server::services::repomap::RepomapService;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
};
use std::env;
use tracing::{error, info};

pub async fn generate_repomap(
    State(state): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    info!("Handling repomap generation request for {}/{}", owner, repo);

    // Get GitHub ID from params
    let github_id = match params.get("github_id") {
        Some(id) => {
            info!("Got GitHub ID from params: {}", id);
            match id.parse::<i64>() {
                Ok(id) => id,
                Err(_) => return error_response("Invalid GitHub ID"),
            }
        }
        None => {
            error!("No GitHub ID provided in params");
            return error_response("GitHub ID not provided");
        }
    };

    // Get user with GitHub token
    let user = match sqlx::query_as!(User, "SELECT * FROM users WHERE github_id = $1", github_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => return error_response("User not found"),
        Err(e) => return error_response(&format!("Database error: {}", e)),
    };

    // Get GitHub token
    let github_token = match user.github_token {
        Some(token) => token,
        None => return error_response("No GitHub token found"),
    };

    // Create temp directory
    let temp_dir = env::temp_dir().join("repomap_temp");
    let repomap_service = RepomapService::new(temp_dir, Some(github_token));

    // Generate repomap
    match repomap_service.generate_repomap(&owner, &repo).await {
        Ok(repomap) => {
            let xml = format!(
                r#"<doc xmlns="https://hyperview.org/hyperview">
                    <screen>
                        <styles>
                            <style id="container" flex="1" backgroundColor="black" padding="16" />
                            <style id="title" fontSize="24" color="white" marginBottom="16" />
                            <style id="content" color="white" fontFamily="monospace" />
                            <style id="scroll" flex="1" />
                        </styles>
                        <body style="container">
                            <text style="title">Repository Map: {}/{}</text>
                            <view style="scroll" scroll="true" scroll-orientation="vertical">
                                <text style="content">{}</text>
                            </view>
                        </body>
                    </screen>
                </doc>"#,
                owner, repo, repomap
            );

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(xml.into())
                .unwrap()
        }
        Err(e) => error_response(&format!("Failed to generate repomap: {}", e)),
    }
}

fn error_response(message: &str) -> Response {
    let xml = format!(
        r#"<doc xmlns="https://hyperview.org/hyperview">
            <screen>
                <styles>
                    <style id="container" flex="1" backgroundColor="black" padding="16" />
                    <style id="error" color="red" fontSize="16" fontWeight="600" />
                </styles>
                <body style="container">
                    <text style="error">{}</text>
                </body>
            </screen>
        </doc>"#,
        message
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}