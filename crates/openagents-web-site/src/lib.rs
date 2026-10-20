//! The `openagents-web` binary over the site's library, and the pages
//! nothing else in the site uses: the screens gallery (`/ui/screens`),
//! the agent card and lab pages, and the run commands
//! (#12184). They are a crate of their own so an edit to one recompiles
//! this crate, not the whole site. [`routes`] adds them to
//! [`openagents_web::router_with`].

pub use openagents_web::*;

mod agent_card_page;
mod agent_lab_page;
mod run_commands;
mod screens;

/// The whole site: the library's routes and this crate's. It shadows
/// [`openagents_web::router`], which has only the library's.
pub fn router(config: Config) -> axum::Router {
    let site = openagents_web::router_with(config, routes);
    // In-process test requests have no socket; the library treats its own
    // tests as on this computer, and this crate's the same way.
    #[cfg(test)]
    let site = site.layer(axum::middleware::map_request(
        |mut request: axum::extract::Request| async move {
            if request
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .is_none()
            {
                request.extensions_mut().insert(axum::extract::ConnectInfo(
                    std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
                ));
            }
            request
        },
    ));
    site
}

/// This crate's routes, added to the site's.
pub fn routes(site: axum::Router<App>) -> axum::Router<App> {
    site.merge(screens::routes())
        .merge(agent_card_page::routes())
        .merge(run_commands::routes())
        .merge(agent_lab_page::routes())
}
