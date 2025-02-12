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
    info!("🗺️ REPOMAP: Endpoint hit for {}/{}", owner, repo);
    info!("🗺️ REPOMAP: Full request params: {:?}", params);
    info!("🗺️ REPOMAP: Starting repomap generation request");

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

    info!("Generating repomap for {}/{}", owner, repo);

    // Generate repomap
    match repomap_service.generate_repomap(&owner, &repo).await {
        Ok(repomap) => {
            info!("Successfully generated repomap");
            info!("🗺️ REPOMAP CONTENT: \n{}", repomap);

            let xml = format!(
                r#"<view xmlns="https://hyperview.org/hyperview" id="repos_list" backgroundColor="black" flex="1" padding="16">
                    <text color="white" fontSize="24" marginBottom="16">Repository Map: {}/{}</text>
                    <text color="white" fontFamily="monospace" fontSize="14" whiteSpace="pre" marginBottom="16">{}</text>
                    <text color="white" backgroundColor="gray" padding="8" borderRadius="4">
                        <behavior
                            trigger="press"
                            action="replace"
                            href="/hyperview/fragments/github-repos?github_id={}"
                            target="repos_list"
                        />
                        Back to Repos
                    </text>
                </view>"#,
                owner, repo, repomap, github_id
            );

            info!("🗺️ GENERATED XML: \n{}", xml);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(xml.into())
                .unwrap()
        }
        Err(e) => {
            error!("Failed to generate repomap: {}", e);
            error_response(&format!("Failed to generate repomap: {}", e))
        }
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
