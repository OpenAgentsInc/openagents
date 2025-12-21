//! View templates for the unified GUI

mod layout;

use actix_web::HttpResponse;
pub use layout::base_layout;

/// Home page with tabbed navigation
pub async fn home() -> HttpResponse {
    let content = r#"
        <div class="dashboard">
            <h1>OpenAgents</h1>
            <p>Unified desktop application for autonomous agents.</p>
            <div class="quick-links">
                <a href="/wallet" class="card">
                    <h3>Wallet</h3>
                    <p>Identity and payments</p>
                </a>
                <a href="/marketplace" class="card">
                    <h3>Marketplace</h3>
                    <p>Compute, skills, data</p>
                </a>
                <a href="/autopilot" class="card">
                    <h3>Autopilot</h3>
                    <p>Autonomous tasks</p>
                </a>
                <a href="/git" class="card">
                    <h3>AgentGit</h3>
                    <p>Nostr-native git</p>
                </a>
            </div>
        </div>
    "#;

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout("Dashboard", content))
}
