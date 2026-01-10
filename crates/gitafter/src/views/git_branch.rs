/// Git branch creation form
pub fn git_branch_create_form_page(identifier: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Create Branch - " (identifier) " - GitAfter" }
                style {
                    (include_str!("../styles.css"))
                }
            }
            body {
                header {
                    h1 { "GitAfter" }
                    nav {
                        a href="/" { "Home" }
                        " | "
                        a href={"/repo/" (identifier)} { "Repository" }
                        " | "
                        span { "Create Branch" }
                    }
                }

                main {
                    h2 { "Create New Branch - " (identifier) }

                    form method="post" action={"/repo/" (identifier) "/git/branch"} {
                        div.form-group {
                            label for="branch_name" { "Branch Name" }
                            input type="text" id="branch_name" name="branch_name" required placeholder="feature/my-feature";
                        }

                        div.form-actions {
                            button type="submit" { "Create Branch" }
                            a.button.secondary href={"/repo/" (identifier)} { "Cancel" }
                        }
                    }
                }

                footer {
                    p { "GitAfter - Nostr-native GitHub alternative" }
                }
            }
        }
    }
}

