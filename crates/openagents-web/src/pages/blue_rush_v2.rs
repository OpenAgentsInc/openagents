//! A separate landing page based on the October 2026 Blue Rush research.
use super::*;
use axum::Form;
use serde::Deserialize;

pub(super) const PATH: &str = "/studios/blue-rush-v2";
pub(super) const INTERVIEW: &str = "/studios/blue-rush-v2/interview";
pub(super) const CSS: &str = include_str!("../../static/bluerush/v2.css");

fn home(site: &Site) -> &'static str {
    if site.home == "/" { "/v2" } else { PATH }
}
fn interview_path(site: &Site) -> &'static str {
    if site.home == "/" {
        "/v2/interview"
    } else {
        INTERVIEW
    }
}
fn header(site: &Site, is_interview: bool) -> Markup {
    html! {
        a class="br-skip" href="#main" { "Skip to content" }
        header class="v2-nav v2-wrap" {
            a class="v2-brand" href=(home(site)) aria-label="Blue Rush Studios home" {
                span class="v2-symbol" aria-hidden="true" { "≈" }
                span { "blue rush" small { "STUDIOS" } }
            }
            nav aria-label="Main" {
                a href={(home(site)) "#games"} { "The games" }
                a href={(home(site)) "#families"} { "For families" }
                a href={(home(site)) "#garden"} { "Sand garden" }
            }
            a class="v2-button v2-small" href=(if is_interview { home(site) } else { interview_path(site) }) {
                @if is_interview { "Back to the shore ↗" } @else { "Parent interview ↗" }
            }
        }
    }
}
fn document_head(site: &Site, title: &str, description: &str, script: bool) -> Markup {
    let css = format!("{}v2.css?v={}", site.assets, version(CSS));
    html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (title) }
            meta name="description" content=(description);
            meta name="theme-color" content="#f6faff";
            link rel="stylesheet" href={(site.assets) "bluerush.css?v=" (css_version())};
            link rel="stylesheet" href=(css);
            @if script { script src={(site.assets) "bluerush.js?v=" (*JS_VERSION)} defer {} }
        }
    }
}
pub(super) async fn page(headers: HeaderMap) -> Response {
    with_policy(render(site(&headers)), POLICY)
}
fn render(site: &Site) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (document_head(site, "Blue Rush Studios — Play the future.", "Games for curious kids who love to explore and build. Come make something wonderful.", true))
            body class="br v2" {
                (header(site, false))
                main id="main" {
                    section class="v2-hero v2-wrap" aria-labelledby="hero-title" {
                        div class="v2-hero-copy" {
                            p class="v2-eyebrow" { span class="v2-dot" {} "A little wonder. A world of possibility." }
                            h1 id="hero-title" { "Play the" br; em { "future." } span class="v2-spark" aria-hidden="true" { "✳" } }
                            p class="v2-intro" { "Games for curious kids who love to explore and build." }
                            p class="v2-sub" { "Big ideas start with little experiments. A ship that holds. A garden that grows. A world they get to make their own." }
                            div class="v2-actions" {
                                a class="v2-button" href="#games" { "Find your next adventure" span aria-hidden="true" { "↗" } }
                                a class="v2-text-link" href="#garden" { "Or make a little beach ↓" }
                            }
                            p class="v2-note" { "Made for curious minds. And the families cheering them on." }
                        }
                        figure class="v2-world" {
                            img src={(site.assets) "maker-town.svg"} alt="A sunlit floating maker town with blue-roofed homes, a garden, a friendly robot, and children testing a little boat." width="900" height="850";
                            figcaption { span class="v2-dot" {} "The world we’re imagining" span { "Concept illustration" } }
                        }
                    }
                    div class="v2-ribbon" aria-label="Our approach" { span { "Follow a question" } span aria-hidden="true" { "✳" } span { "Try an idea" } span aria-hidden="true" { "✳" } span { "See what happens" } span aria-hidden="true" { "✳" } span { "Make it yours" } }
                    section class="v2-section v2-wrap" id="games" aria-labelledby="games-title" {
                        div class="v2-section-heading" { div { p class="v2-eyebrow" { "Small starts. Big adventures." } h2 id="games-title" { "Where will curiosity take you?" } } p { "A few worlds to jump into today." } }
                        div class="v2-game-grid" {
                            article class="v2-game" {
                                a href=(SNAP) class="v2-game-image" tabindex="-1" aria-hidden="true" { img src={(site.assets) "snap-spin.jpg"} alt="" width="960" height="600" loading="lazy"; span class="v2-badge" { "Play now · Ages 5–12" } }
                                div class="v2-game-body" { p class="v2-eyebrow" { "For the ‘what if?’ builders" } h3 { "Snap Station" } p { "Build a ship. Press Go. See if it holds. Snap magnetic pieces together with jobs from a friendly robot crew." }
                                    div class="v2-card-actions" { a class="v2-button" href=(SNAP) { "Start building ↗" } a href=(site.snap_guide) { "How to play" } }
                                }
                            }
                            article class="v2-game" {
                                a href=(BUNNY) class="v2-game-image" tabindex="-1" aria-hidden="true" { img src={(site.assets) "bunny-tour.jpg"} alt="" width="960" height="600" loading="lazy"; span class="v2-badge" { "Play now" } }
                                div class="v2-game-body" { p class="v2-eyebrow" { "For the little explorers" } h3 { "Grow Little Bunny" } p { "A tiny bunny. A very tempting garden. Hop, eat, and grow as you find your way around the farmer’s net." }
                                    div class="v2-card-actions" { a class="v2-button" href=(BUNNY) { "Hop into the garden ↗" } a href=(site.guide) { "How to play" } }
                                }
                            }
                        }
                    }
                    section class="v2-family" id="families" aria-labelledby="family-title" {
                        div class="v2-wrap v2-family-grid" {
                            div { p class="v2-eyebrow" { "For families making their own way" } h2 id="family-title" { "Their ideas." br; "Your kind of adventure." } p { "Whether you homeschool or just love a good afternoon project, there’s room here to be curious together." } a class="v2-text-link" href=(interview_path(site)) { "Tell us what matters to your family ↗" } }
                            div class="v2-values" {
                                article { span aria-hidden="true" { "01 /" } h3 { "Make something. Change something." } p { "Build, test, and try again. The fun is finding out what your next idea can do." } }
                                article { span aria-hidden="true" { "02 /" } h3 { "A future worth imagining." } p { "Ocean neighborhoods. Helpful robots. Gardens at sea. We’re drawn to worlds where people make life better together." } }
                                article { span aria-hidden="true" { "03 /" } h3 { "A conversation with parents." } p { "We’re exploring how AI could help children create. What should kids choose? What should parents approve? Your experience will help shape the answer." } }
                            }
                        }
                    }
                    section class="v2-section v2-wrap" aria-labelledby="horizon-title" {
                        div class="v2-section-heading" { div { p class="v2-eyebrow" { "On the horizon" } h2 id="horizon-title" { "What if home could float?" } } span class="v2-pill" { "Concept · Not yet playable" } }
                        div class="v2-concept" {
                            div class="v2-concept-art" { img src={(site.assets) "maker-town.svg"} alt="A floating neighborhood with a greenhouse, solar panels, and a boat workshop." width="900" height="850" loading="lazy"; }
                            div { p class="v2-eyebrow" { "An idea we’re exploring" } h3 { "A neighborhood built on ‘what if?’" } p { "Imagine designing a floating home with an AI helper. Choose its shape, test its balance, and change what doesn’t work. Add a garden. Make room for a neighbor." } p { "Would your child want to build here? We’d love to hear what you think." } a class="v2-text-link" href=(interview_path(site)) { "Join the parent interview ↗" } }
                        }
                    }
                    (garden())
                    section class="v2-invite v2-wrap" aria-labelledby="invite-title" {
                        span class="v2-invite-star" aria-hidden="true" { "✳" }
                        div { p class="v2-eyebrow" { "Calling curious families" } h2 id="invite-title" { "Help us make the next good thing." } p { "Homeschooling mom? Choosing games for your family? We’d love to hear what you look for, what you skip, and how you feel about AI." } }
                        a class="v2-button" href=(interview_path(site)) { "Take the parent interview ↗" }
                    }
                }
                (footer(site))
            }
        }
    }
}

const QUESTIONS: [&str; 5] = [
    "Tell us about the last learning game you chose. What convinced you? What made you stop using one?",
    "What does a good day of learning and play look like for your family?",
    "What would you want your child to do with AI? What would you want to approve yourself?",
    "Which would your child want to try: an ocean explorer, a floating neighborhood builder, or a family invention lab? Why?",
    "What would you need to see before welcoming a new game into your home?",
];
pub(super) async fn interview(headers: HeaderMap) -> Response {
    with_policy(render_interview(site(&headers)), GUIDE_POLICY)
}
fn render_interview(site: &Site) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (document_head(site, "A conversation with curious families — Blue Rush Studios", "Help shape games for curious kids. A short interview for parents choosing learning games and exploring AI.", false))
            body class="br v2" {
                (header(site, true))
                main id="main" class="v2-interview v2-wrap" {
                    p class="v2-eyebrow" { "Your family. Your perspective." }
                    h1 { "The best ideas start" br; em { "with listening." } }
                    p class="v2-intro" { "Help us imagine what play could be." }
                    p class="v2-interview-intro" { "We’re listening to homeschooling mothers and other parents who choose games for their families. Whether you’re excited about AI or still deciding how it fits, we’d love to hear your experience." }
                    div class="v2-interview-details" { span { "5 questions" } span { "About 10 minutes" } span { "No right answers" } }
                    form method="post" action=(interview_path(site)) {
                        p class="v2-form-note" { "Save your answers, then share the file with the person who invited you. We don’t keep a copy. Leave out names and other private details." }
                        @for (i, question) in QUESTIONS.iter().enumerate() {
                            div class="v2-question" {
                                label for=(format!("q{i}")) { span { (format!("0{} /", i + 1)) } (question) }
                                textarea id=(format!("q{i}")) name=(format!("q{i}")) rows="4" maxlength="4000" required {}
                            }
                        }
                        button class="v2-button" type="submit" { "Save my interview answers ↓" }
                    }
                }
                (footer(site))
            }
        }
    }
}
#[derive(Deserialize)]
pub(super) struct Answers {
    q0: String,
    q1: String,
    q2: String,
    q3: String,
    q4: String,
}
pub(super) async fn answers(Form(answers): Form<Answers>) -> Response {
    let values = [answers.q0, answers.q1, answers.q2, answers.q3, answers.q4];
    if values
        .iter()
        .any(|v| v.trim().is_empty() || v.chars().count() > 4000)
    {
        return (
            StatusCode::BAD_REQUEST,
            "Answer each question using up to 4,000 characters, then try again.",
        )
            .into_response();
    }
    let mut text = String::from(
        "Blue Rush Studios — Parent interview\n\nShare this file with the person who invited you.\n\n",
    );
    for (question, answer) in QUESTIONS.iter().zip(values) {
        text.push_str(&format!("{question}\n\n{}\n\n", answer.trim()));
    }
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=blue-rush-interview.txt",
            ),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        text,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn v2_works_on_both_hosts_without_changing_the_original() {
        assert_eq!(studio_path("/"), super::super::PATH);
        assert_eq!(studio_path("/v2"), PATH);
        assert_eq!(studio_path("/v2/interview"), INTERVIEW);
        assert_eq!(studio_path(PATH), PATH);
        assert_eq!(studio_path(INTERVIEW), INTERVIEW);
        for site in [&ON_OPENAGENTS, &ON_STUDIO] {
            let page = render(site).into_string();
            assert!(page.contains(&format!("href=\"{}\"", interview_path(site))));
            assert!(page.contains("id=\"br-sand\""));
            assert!(page.contains("Concept · Not yet playable"));
            assert!(page.contains("snap-spin.jpg"));
            assert_eq!(oa_copy::violations(&page, &[]), vec![]);
            let interview = render_interview(site).into_string();
            assert!(interview.contains(&format!("action=\"{}\"", interview_path(site))));
            assert!(interview.contains("We don’t keep a copy."));
            assert_eq!(oa_copy::violations(&interview, &[]), vec![]);
            for question in QUESTIONS {
                assert!(interview.contains(&maud::html! { (question) }.into_string()));
            }
        }
    }

    #[tokio::test]
    async fn answers_download_without_becoming_public_html() {
        let response = answers(Form(Answers {
            q0: "<script>private answer</script>".into(),
            q1: "2".into(),
            q2: "3".into(),
            q3: "4".into(),
            q4: "5".into(),
        }))
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::CONTENT_DISPOSITION],
            "attachment; filename=blue-rush-interview.txt"
        );
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        assert!(
            String::from_utf8(body.to_vec())
                .unwrap()
                .contains("<script>private answer</script>")
        );
        let invalid = answers(Form(Answers {
            q0: " ".into(),
            q1: "2".into(),
            q2: "3".into(),
            q3: "4".into(),
            q4: "5".into(),
        }))
        .await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
}
