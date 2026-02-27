use super::*;

impl Storybook {
    pub(crate) fn paint_marketplace_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let provider_height = panel_height(260.0);
        let skills_height = panel_height(280.0);
        let data_height = panel_height(280.0);
        let ref_height = panel_height(180.0);

        let panels = panel_stack(
            bounds,
            &[provider_height, skills_height, data_height, ref_height],
        );

        // ========== Panel 1: Compute Providers ==========
        let provider_bounds = panels[0];
        draw_panel("Compute Providers", provider_bounds, cx, |inner, cx| {
            let providers = [
                ProviderInfo::new(
                    "p1",
                    "FastCompute Pro",
                    ProviderSpecs::new(32, 128, 2000).gpu("NVIDIA A100"),
                )
                .status(ProviderStatus::Online)
                .price(15000)
                .rating(4.9)
                .jobs(1250)
                .location("US-East"),
                ProviderInfo::new("p2", "Budget Runner", ProviderSpecs::new(8, 32, 500))
                    .status(ProviderStatus::Busy)
                    .price(2000)
                    .rating(4.5)
                    .jobs(340)
                    .location("EU-West"),
            ];

            for (i, provider) in providers.iter().enumerate() {
                let mut card = ProviderCard::new(provider.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: Skills Marketplace ==========
        let skills_bounds = panels[1];
        draw_panel("Skills Marketplace", skills_bounds, cx, |inner, cx| {
            let skills = [
                SkillInfo::new(
                    "s1",
                    "Code Review Pro",
                    "AI-powered code review with security analysis",
                )
                .category(SkillCategory::CodeGeneration)
                .author("openagents")
                .version("2.1.0")
                .status(SkillInstallStatus::Installed)
                .downloads(45000)
                .rating(4.8),
                SkillInfo::new(
                    "s2",
                    "Data Transformer",
                    "Transform and clean datasets automatically",
                )
                .category(SkillCategory::DataAnalysis)
                .author("datacraft")
                .version("1.5.2")
                .status(SkillInstallStatus::Available)
                .price(5000)
                .downloads(12000)
                .rating(4.6),
            ];

            for (i, skill) in skills.iter().enumerate() {
                let mut card = SkillCard::new(skill.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 120.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: Data Marketplace ==========
        let data_bounds = panels[2];
        draw_panel("Data Marketplace", data_bounds, cx, |inner, cx| {
            let datasets = [
                DatasetInfo::new(
                    "d1",
                    "LLM Training Corpus",
                    "High-quality text corpus for language model training",
                )
                .format(DataFormat::Parquet)
                .license(DataLicense::OpenSource)
                .size(10_737_418_240) // 10 GB
                .rows(50_000_000)
                .author("opendata")
                .downloads(2500)
                .updated_at("2 days ago"),
                DatasetInfo::new(
                    "d2",
                    "Code Embeddings",
                    "Pre-computed embeddings for 100+ programming languages",
                )
                .format(DataFormat::Arrow)
                .license(DataLicense::Commercial)
                .size(5_368_709_120) // 5 GB
                .rows(25_000_000)
                .author("codebase")
                .price(25000)
                .downloads(850)
                .updated_at("1 week ago"),
            ];

            for (i, dataset) in datasets.iter().enumerate() {
                let mut card = DatasetCard::new(dataset.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(550.0),
                        105.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Categories & Formats Reference ==========
        let ref_bounds = panels[3];
        draw_panel("Categories & Formats", ref_bounds, cx, |inner, cx| {
            // Skill categories
            let mut cat_x = inner.origin.x;
            let categories = [
                SkillCategory::CodeGeneration,
                SkillCategory::DataAnalysis,
                SkillCategory::WebAutomation,
                SkillCategory::FileProcessing,
                SkillCategory::ApiIntegration,
            ];

            for cat in &categories {
                let cat_w = (cat.label().len() as f32 * 6.0) + 12.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cat_x, inner.origin.y, cat_w, 18.0))
                        .with_background(cat.color().with_alpha(0.2))
                        .with_border(cat.color(), 1.0),
                );
                let text = cx.text.layout(
                    cat.label(),
                    Point::new(cat_x + 4.0, inner.origin.y + 3.0),
                    theme::font_size::XS,
                    cat.color(),
                );
                cx.scene.draw_text(text);
                cat_x += cat_w + 8.0;
            }

            // Data formats
            let mut fmt_x = inner.origin.x;
            let formats = [
                DataFormat::Json,
                DataFormat::Csv,
                DataFormat::Parquet,
                DataFormat::Arrow,
                DataFormat::Sqlite,
            ];

            for fmt in &formats {
                let fmt_w = 60.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(fmt_x, inner.origin.y + 30.0, fmt_w, 18.0))
                        .with_background(fmt.color().with_alpha(0.2))
                        .with_border(fmt.color(), 1.0),
                );
                let text = cx.text.layout(
                    fmt.label(),
                    Point::new(fmt_x + 6.0, inner.origin.y + 33.0),
                    theme::font_size::XS,
                    fmt.color(),
                );
                cx.scene.draw_text(text);
                fmt_x += fmt_w + 8.0;
            }

            // Provider statuses
            let mut status_x = inner.origin.x;
            let statuses = [
                ProviderStatus::Online,
                ProviderStatus::Busy,
                ProviderStatus::Offline,
                ProviderStatus::Maintenance,
            ];

            for status in &statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y + 60.0, status_w, 18.0))
                        .with_background(status.color().with_alpha(0.2))
                        .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 6.0, inner.origin.y + 63.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                status_x += status_w + 8.0;
            }

            // Install statuses
            let mut install_x = inner.origin.x;
            let install_statuses = [
                SkillInstallStatus::Available,
                SkillInstallStatus::Installed,
                SkillInstallStatus::UpdateAvailable,
                SkillInstallStatus::Installing,
            ];

            for status in &install_statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        install_x,
                        inner.origin.y + 90.0,
                        status_w,
                        18.0,
                    ))
                    .with_background(status.color().with_alpha(0.2))
                    .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(install_x + 6.0, inner.origin.y + 93.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                install_x += status_w + 8.0;
            }
        });
    }
}
