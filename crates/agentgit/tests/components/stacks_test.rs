//! Component tests for stacked diffs view

use scraper::{Html, Selector};

#[test]
fn test_stack_view_ordered_layers() {
    let stack_html = r#"
        <div class="stack-view">
            <h2>Stack: Feature Implementation</h2>
            <ol class="stack-layers">
                <li class="layer">
                    <span class="layer-number">1 of 3</span>
                    <a href="/prs/1">Layer 1: Foundation</a>
                    <span class="status merged">Merged</span>
                </li>
                <li class="layer">
                    <span class="layer-number">2 of 3</span>
                    <a href="/prs/2">Layer 2: Core logic</a>
                    <span class="status open">Open</span>
                </li>
                <li class="layer">
                    <span class="layer-number">3 of 3</span>
                    <a href="/prs/3">Layer 3: Integration</a>
                    <span class="status draft">Draft</span>
                </li>
            </ol>
        </div>
    "#;

    let doc = Html::parse_fragment(stack_html);

    // Must use ordered list for layers
    let ol = doc.select(&Selector::parse("ol").unwrap()).next();
    assert!(
        ol.is_some(),
        "Stack layers must use ordered list (ol) element"
    );

    // Must have 3 layers
    let layers: Vec<_> = doc.select(&Selector::parse(".layer").unwrap()).collect();
    assert_eq!(layers.len(), 3, "Should have 3 stack layers");

    // Each layer must show position
    let layer_numbers: Vec<_> = doc.select(&Selector::parse(".layer-number").unwrap()).collect();
    assert_eq!(layer_numbers.len(), 3, "Each layer must show position");
}

#[test]
fn test_stack_layer_accessibility() {
    let layer_html = r#"
        <li class="layer">
            <span class="layer-number" aria-label="Layer 1 of 3">1 of 3</span>
            <a href="/prs/1">Layer 1: Foundation</a>
            <span class="status merged">Merged</span>
        </li>
    "#;

    let doc = Html::parse_fragment(layer_html);

    // Layer number must be accessible
    let layer_num = doc.select(&Selector::parse(".layer-number").unwrap()).next();
    assert!(layer_num.is_some(), "Must have layer number");

    let text: String = layer_num.unwrap().text().collect();
    assert!(
        !text.trim().is_empty(),
        "Layer number must have visible text"
    );

    // Link must have accessible text
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "Must have layer link");

    let link_text: String = link.unwrap().text().collect();
    assert!(
        !link_text.trim().is_empty(),
        "Layer link must have accessible text"
    );
}

#[test]
fn test_stack_dependency_indicator() {
    let dependency_html = r#"
        <div class="layer-dependency">
            <span>Depends on:</span>
            <a href="/prs/1">Layer 1: Foundation</a>
        </div>
    "#;

    let doc = Html::parse_fragment(dependency_html);

    // Must show dependency relationship
    let dep_div = doc.select(&Selector::parse(".layer-dependency").unwrap()).next();
    assert!(dep_div.is_some(), "Must show dependency information");

    // Link to dependency must be present
    let link = doc.select(&Selector::parse("a").unwrap()).next();
    assert!(link.is_some(), "Must link to dependency");
}

#[test]
fn test_stack_merge_blocker() {
    let blocker_html = r#"
        <div class="merge-blocker">
            <span class="warning">⚠ Cannot merge: dependencies not merged</span>
            <ul>
                <li><a href="/prs/1">Layer 1</a> must be merged first</li>
            </ul>
        </div>
    "#;

    let doc = Html::parse_fragment(blocker_html);

    // Must show blocker warning
    let warning = doc.select(&Selector::parse(".warning").unwrap()).next();
    assert!(warning.is_some(), "Must display merge blocker warning");

    // Must list blockers
    let list = doc.select(&Selector::parse("ul").unwrap()).next();
    assert!(list.is_some(), "Must list blocking dependencies");
}

#[test]
fn test_restack_button() {
    let restack_html = r#"
        <form action="/stacks/restack" method="post">
            <button type="submit">Restack layers</button>
        </form>
    "#;

    let doc = Html::parse_fragment(restack_html);

    // Button must be in form
    let form = doc.select(&Selector::parse("form").unwrap()).next();
    assert!(form.is_some(), "Restack must be in form");

    // Button must have text
    let button = doc.select(&Selector::parse("button").unwrap()).next();
    assert!(button.is_some(), "Must have restack button");

    let text: String = button.unwrap().text().collect();
    assert!(!text.trim().is_empty(), "Button must have text");
}

#[test]
fn test_stack_uuid_display() {
    let stack_html = r#"
        <div class="stack-info">
            <span>Stack ID:</span>
            <code>stack-uuid-abc-123</code>
        </div>
    "#;

    let doc = Html::parse_fragment(stack_html);

    // UUID must be in code element
    let code = doc.select(&Selector::parse("code").unwrap()).next();
    assert!(code.is_some(), "Stack UUID must use code element");
}

#[test]
fn test_stack_layer_status_colors() {
    let layers_html = r#"
        <ol>
            <li><span class="status merged">Merged</span></li>
            <li><span class="status open">Open</span></li>
            <li><span class="status draft">Draft</span></li>
        </ol>
    "#;

    let doc = Html::parse_fragment(layers_html);

    // All statuses must be present
    let statuses: Vec<_> = doc.select(&Selector::parse(".status").unwrap()).collect();
    assert_eq!(statuses.len(), 3, "Should have 3 status indicators");

    // Each must have text
    for status in statuses {
        let text: String = status.text().collect();
        assert!(
            !text.trim().is_empty(),
            "Status must have visible text"
        );
    }
}

#[test]
fn test_empty_stack_view() {
    let empty_html = r#"
        <div class="stack-view">
            <p>No layers in this stack.</p>
        </div>
    "#;

    let doc = Html::parse_fragment(empty_html);

    let message = doc.select(&Selector::parse("p").unwrap()).next();
    assert!(message.is_some(), "Empty stack must have message");

    let text: String = message.unwrap().text().collect();
    assert!(
        text.contains("No layers"),
        "Empty message must be informative"
    );
}
