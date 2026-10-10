use super::*;

const LANG_REPLY: &str = "Install Coder, then sign in.\n\n```openui-lang\nroot = Steps([install, signin])\ninstall = Step(\"Install Coder\", [Command(\"curl -fsSL https://openagents.com/cli/install.sh | bash\")])\nsignin = Step(\"Sign in\", [CodeBlock(\"coder login\")])\n```\n";

const JSON_REPLY: &str = "Install Coder, then sign in.\n\n```ui-json\n{\"type\":\"Steps\",\"steps\":[{\"type\":\"Step\",\"title\":\"Install Coder\",\"children\":[{\"type\":\"Command\",\"unix\":\"curl -fsSL https://openagents.com/cli/install.sh | bash\"}]},{\"type\":\"Step\",\"title\":\"Sign in\",\"children\":[{\"type\":\"CodeBlock\",\"code\":\"coder login\"}]}]}\n```\n";

#[test]
fn the_checked_in_prompts_parse() {
    let prompts = Prompts::parse(PROMPTS).expect("the prompts parse");
    assert!(prompts.prompts.len() >= 10, "{}", prompts.prompts.len());
    let mut ids: Vec<&str> = prompts.prompts.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), prompts.prompts.len(), "ids are unique");
}

#[test]
fn both_prompts_describe_every_component() {
    let lang = Format::Lang.instructions();
    let json = Format::Json.instructions();
    for component in CATALOG {
        assert!(
            lang.contains(&format!("- {}(", component.name)),
            "{}",
            component.name
        );
        assert!(
            json.contains(&format!("{{\"type\": \"{}\"", component.name)),
            "{}",
            component.name
        );
    }
    assert!(json.contains("```ui-json"), "{json}");
    assert!(
        json.contains("\"style\"?: \"primary\"|\"secondary\""),
        "{json}"
    );
}

#[test]
fn the_same_interface_scores_the_same_in_both_formats() {
    let lang = score(Format::Lang, LANG_REPLY);
    let json = score(Format::Json, JSON_REPLY);
    assert!(lang.valid && json.valid, "{lang:?} {json:?}");
    assert_eq!((lang.diagnostics, json.diagnostics), (0, 0));
    // The same tree, whichever format wrote it.
    let (lang_source, _) = fenced(LANG_REPLY, openui_lang::LANG).unwrap();
    let (json_source, _) = fenced(JSON_REPLY, JSON_LANG).unwrap();
    let value: Value = serde_json::from_str(json_source.trim()).unwrap();
    assert_eq!(
        openui_lang::parse(lang_source).root,
        openui_lang::parse(&json_to_lang(&value).unwrap()).root
    );
}

#[test]
fn invalid_and_blank_replies_are_told_apart() {
    let bad = LANG_REPLY.replace("CodeBlock", "Bogus");
    let scored = score(Format::Lang, &bad);
    assert!(
        !scored.valid && !scored.blank && scored.diagnostics > 0,
        "{scored:?}"
    );
    assert!(score(Format::Lang, "Prose only.").blank);
    let broken_json = JSON_REPLY.replace("]}]}", "]}");
    assert!(score(Format::Json, &broken_json).blank);
    let unclosed = LANG_REPLY.trim_end().trim_end_matches("```");
    let scored = score(Format::Lang, unclosed);
    assert!(!scored.valid && !scored.blank, "{scored:?}");
}

#[test]
fn openui_lang_draws_before_the_block_closes_and_json_does_not() {
    let cut = |text: &str, before: &str| text[..text.find(before).unwrap()].to_owned();
    // Cut inside the second step: the first statements already draw.
    let lang = cut(LANG_REPLY, "signin = Step");
    assert!(renders(Format::Lang, &lang), "{lang}");
    let json = cut(JSON_REPLY, "{\"type\":\"Step\",\"title\":\"Sign in\"");
    assert!(!renders(Format::Json, &json), "{json}");
    assert!(renders(Format::Json, JSON_REPLY));
    assert!(!renders(Format::Lang, "Install Coder, then"));
}
