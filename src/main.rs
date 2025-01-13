use anyhow::Context;
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "with_axum_htmx_askama=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("initializing router...");

    let assets_path = std::env::current_dir().unwrap();
    let port = 8000_u16;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let router = Router::new()
        .route("/", get(hello))
        .route("/mobile-app", get(mobile_app))
        .route("/business", get(business))
        .route("/video-series", get(video_series))
        .route("/company", get(company))
        .route("/contact", get(contact))
        .nest_service(
            "/assets",
            ServeDir::new(format!("{}/assets", assets_path.to_str().unwrap())),
        );

    info!("router initialized, now listening on port {}", port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind TcpListener")?;

    info!("listening on {}", addr);
    axum::serve(listener, router)
        .await
        .context("error while starting server")?;

    Ok(())
}

#[derive(Template)]
#[template(path = "hello.html")]
struct PageTemplate {
    title: String,
}

async fn hello() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Home".to_string(),
    };
    HtmlTemplate(template)
}

async fn mobile_app() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Mobile App".to_string(),
    };
    HtmlTemplate(template)
}

async fn business() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Services".to_string(),
    };
    HtmlTemplate(template)
}

async fn video_series() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Video Series".to_string(),
    };
    HtmlTemplate(template)
}

async fn company() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Company".to_string(),
    };
    HtmlTemplate(template)
}

async fn contact() -> impl IntoResponse {
    let template = PageTemplate {
        title: "Contact".to_string(),
    };
    HtmlTemplate(template)
}

/// A wrapper type that we'll use to encapsulate HTML parsed by askama into valid HTML for axum to serve.
struct HtmlTemplate<T>(T);

/// Allows us to convert Askama HTML templates into valid HTML for axum to serve in the response.
impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        // Attempt to render the template with askama
        match self.0.render() {
            // If we're able to successfully parse and aggregate the template, serve it
            Ok(html) => Html(html).into_response(),
            // If we're not, return an error or some bit of fallback HTML
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}