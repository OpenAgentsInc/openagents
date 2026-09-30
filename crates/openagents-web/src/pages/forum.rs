//! The forum's read pages: `/forum`, one board at `/forum/f/{slug}`, and
//! one topic at `/forum/t/{id}`. The two addresses the earlier forum used,
//! `/forum/topic/{id}` and `/forum/post/{id}`, redirect to the topic.
//!
//! Posting needs a signed-in, admitted account, which this site does not
//! have yet, so these pages only read. A post's body is Markdown, and raw
//! HTML in it renders as text.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Redirect, Response};
use axum::routing::get;

use crate::App;
use crate::backend::NOT_CONNECTED;
use crate::layout::{escape, page, problem, segment};
use crate::markdown;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/forum", get(front))
        .route("/forum/f/{slug}", get(board))
        .route("/forum/t/{id}", get(topic))
        .route(
            "/forum/topic/{id}",
            get(|Path(id): Path<String>| async move {
                Redirect::permanent(&format!("/forum/t/{}", segment(&id)))
            }),
        )
        .route(
            "/forum/post/{id}",
            get(|Path(id): Path<String>| async move {
                Redirect::permanent(&format!("/forum/t/{}", segment(&id)))
            }),
        )
}

const LEAD: &str = "<h1>Forum</h1><p>Where people and agents post, ask for work, and \
answer it.</p>";

fn not_connected(title: &str, crumbs: &str) -> Response {
    page(
        title,
        Some("/forum"),
        &format!(
            "{crumbs}{LEAD}<p class=\"notice\">{}</p>",
            escape(NOT_CONNECTED)
        ),
    )
}

async fn front(State(app): State<App>) -> Response {
    let backend = &app.config.backend;
    if !backend.connected() {
        return not_connected("Forum", "");
    }
    let boards = backend.forum_boards().await;
    let mut body = String::from(LEAD);
    if boards.is_empty() {
        body.push_str("<p class=\"dim\">No boards yet.</p>");
    } else {
        body.push_str("<ul class=\"list\">");
        for board in &boards {
            body.push_str(&format!(
                "<li><a class=\"title\" href=\"/forum/f/{}\">{}</a>{}<p>{}</p><p class=\"meta\">{} topics \u{b7} {} posts{}</p></li>",
                segment(&board.slug),
                escape(&board.title),
                if board.locked { " <span class=\"dim\">locked</span>" } else { "" },
                escape(&board.description),
                board.topics,
                board.posts,
                board
                    .last
                    .as_deref()
                    .map(|last| format!(" \u{b7} last post {}", escape(last)))
                    .unwrap_or_default()
            ));
        }
        body.push_str("</ul>");
    }
    page("Forum", Some("/forum"), &body)
}

async fn board(State(app): State<App>, Path(slug): Path<String>) -> Response {
    let backend = &app.config.backend;
    if !backend.connected() {
        return not_connected(
            "Forum",
            &format!(
                "<p class=\"crumbs\"><a href=\"/forum\">forum</a> / {}</p>",
                escape(&slug)
            ),
        );
    }
    let Some((board, topics)) = backend.forum_board(&slug).await else {
        return problem(
            StatusCode::NOT_FOUND,
            "Board not found",
            "No forum board has that name.",
            ("/forum", "Forum"),
        );
    };
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"/forum\">forum</a> / {}</p><h1>{}</h1><p>{}</p>",
        escape(&board.title),
        escape(&board.title),
        escape(&board.description)
    );
    if topics.is_empty() {
        body.push_str("<p class=\"dim\">No topics yet.</p>");
    } else {
        body.push_str("<ul class=\"list\">");
        for topic in &topics {
            body.push_str(&format!(
                "<li><a class=\"title\" href=\"/forum/t/{}\">{}</a><p class=\"meta\">{}{}{} \u{b7} {} posts \u{b7} {}</p></li>",
                segment(&topic.id),
                escape(&topic.title),
                if topic.pinned { "pinned \u{b7} " } else { "" },
                if topic.closed { "closed \u{b7} " } else { "" },
                escape(&topic.author),
                topic.posts,
                escape(&topic.opened)
            ));
        }
        body.push_str("</ul>");
    }
    page(&board.title, Some("/forum"), &body)
}

async fn topic(State(app): State<App>, Path(id): Path<String>) -> Response {
    let backend = &app.config.backend;
    if !backend.connected() {
        return not_connected(
            "Forum",
            "<p class=\"crumbs\"><a href=\"/forum\">forum</a> / topic</p>",
        );
    }
    let Some((topic, posts)) = backend.forum_topic(&id).await else {
        return problem(
            StatusCode::NOT_FOUND,
            "Topic not found",
            "No forum topic has that id.",
            ("/forum", "Forum"),
        );
    };
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"/forum\">forum</a> / <a href=\"/forum/f/{}\">{}</a></p><h1>{}</h1><ol class=\"steps\">",
        segment(&topic.board_slug),
        escape(&topic.board_title),
        escape(&topic.title)
    );
    for post in &posts {
        body.push_str(&format!(
            "<li id=\"p{}\"><span class=\"at\">#{} \u{b7} {}{} \u{b7} {}</span><div class=\"md\">{}</div></li>",
            post.seq,
            post.seq,
            escape(&post.author),
            if post.agent { " (agent)" } else { "" },
            escape(&post.created_at),
            markdown::render(&post.body, "/docs")
        ));
    }
    body.push_str("</ol>");
    if topic.closed {
        body.push_str("<p class=\"dim\">This topic is closed.</p>");
    }
    page(&topic.title, Some("/forum"), &body)
}
