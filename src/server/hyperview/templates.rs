use crate::server::models::repository::Repository;

pub fn render_repositories_screen(repos: Vec<Repository>) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>
        <styles>
            <style id="screen" backgroundColor="black" flex="1" />
            <style id="header" backgroundColor="gray" padding="16" />
            <style id="title" color="white" fontSize="20" fontWeight="bold" />
            <style id="list" flex="1" />
            <style id="repo-item" backgroundColor="#111" marginBottom="8" padding="16" borderRadius="8" />
            <style id="repo-name" color="white" fontSize="16" fontWeight="bold" />
            <style id="repo-desc" color="#999" fontSize="14" marginTop="4" />
            <style id="repo-meta" color="#666" fontSize="12" marginTop="8" />
            <style id="error" color="red" fontSize="16" padding="16" textAlign="center" />
            <style id="loading" flex="1" justifyContent="center" alignItems="center" />
        </styles>
        <body style="screen">
            <header style="header">
                <text style="title">Your Repositories</text>
            </header>
            <list style="list">
                {}", 
        repos.into_iter()
            .map(|repo| format!(
                r#"<item style="repo-item" href="/hyperview/repo/{}/issues">
                    <text style="repo-name">{}</text>
                    {}
                    <text style="repo-meta">Last updated: {}</text>
                </item>"#,
                repo.full_name,
                repo.name,
                repo.description
                    .map(|d| format!(r#"<text style="repo-desc">{}</text>"#, d))
                    .unwrap_or_default(),
                repo.updated_at
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        r#"
            </list>
        </body>
    </screen>
</doc>"#
    )
}

pub fn render_loading_screen() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>
        <styles>
            <style id="screen" backgroundColor="black" flex="1" />
            <style id="loading" flex="1" justifyContent="center" alignItems="center" />
            <style id="loading-text" color="white" fontSize="16" />
        </styles>
        <body style="screen">
            <view style="loading">
                <text style="loading-text">Loading repositories...</text>
            </view>
        </body>
    </screen>
</doc>"#.to_string()
}

pub fn render_error_screen(error: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
    <screen>
        <styles>
            <style id="screen" backgroundColor="black" flex="1" />
            <style id="error" color="red" fontSize="16" padding="16" textAlign="center" />
            <style id="retry-button" backgroundColor="#333" padding="12" margin="16" borderRadius="8" alignItems="center" />
            <style id="retry-text" color="white" fontSize="14" />
        </styles>
        <body style="screen">
            <text style="error">{}</text>
            <view style="retry-button" href="/hyperview/repositories">
                <text style="retry-text">Retry</text>
            </view>
        </body>
    </screen>
</doc>"#,
        error
    )
}