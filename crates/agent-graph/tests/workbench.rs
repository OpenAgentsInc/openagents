//! Graph edits go through Agent Lab's own form: each planned step is a
//! control the form shows, and applying it changes the draft the form and
//! the graph both read.

use agent_graph::edit::{Edit, Op, editable, plan};
use agent_lab::view::Workbench;
use rust_native::Element;

fn keys(workbench: &Workbench) -> Vec<String> {
    let view = workbench.view();
    let Element::Stack { children, .. } = &view.root.element else {
        return vec![];
    };
    children.iter().map(|n| n.key.clone()).collect()
}

fn apply(workbench: &mut Workbench, edit: Edit) {
    for op in plan(&workbench.draft, &edit).unwrap() {
        let present = keys(workbench);
        match op {
            Op::Field { key, value } => {
                assert!(present.contains(&key), "the form has no field {key}");
                workbench.field(&key, value).unwrap();
            }
            Op::Action { key } => {
                assert!(present.contains(&key), "the form has no control {key}");
                workbench.action(&key).unwrap();
            }
        }
    }
}

fn foreman() -> Workbench {
    let mut workbench = Workbench::new("test".into());
    workbench.draft = agent_lab::templates::foreman();
    workbench
}

#[test]
fn graph_edits_change_the_draft_the_form_shows() {
    let mut w = foreman();
    apply(&mut w, Edit::ToggleAuthority);
    assert!(!editable(&w.draft).authority);
    assert!(keys(&w).contains(&"authority".to_owned()));
    apply(&mut w, Edit::SetEveryMinutes { minutes: Some(30) });
    assert_eq!(editable(&w.draft).every_minutes, Some(30));
    apply(&mut w, Edit::SetStallMinutes { minutes: Some(45) });
    assert_eq!(editable(&w.draft).stall_minutes, Some(45));
    apply(&mut w, Edit::ToggleCheckIns);
    assert!(!editable(&w.draft).check_ins);
    assert!(!keys(&w).contains(&"every-minutes".to_owned()));
    apply(&mut w, Edit::RemoveTool { name: "gh".into() });
    assert!(!editable(&w.draft).named.contains(&"gh".to_owned()));
    apply(&mut w, Edit::RemoveSkill { number: 8 });
    assert_eq!(w.draft.draft.components.len(), 7);
    apply(&mut w, Edit::AddSkill);
    assert_eq!(w.draft.draft.components.len(), 8);
    // A new skill starts empty; its text is written in the form.
    w.field("component:7", "Keep each note short.".into())
        .unwrap();
    apply(&mut w, Edit::AddTool { name: "gh".into() });
    assert!(editable(&w.draft).named.contains(&"gh".to_owned()));
    assert!(w.draft_valid(), "{:?}", w.draft.validate());
    // The graph reads the edited draft.
    let graph = agent_graph::lab::of(&w.draft, "foreman", &Default::default());
    graph.validate().unwrap();
    assert!(graph.unit("check-ins").is_none());
}

#[test]
fn out_of_range_minutes_keep_the_draft_from_saving() {
    let mut w = foreman();
    apply(&mut w, Edit::SetEveryMinutes { minutes: Some(2) });
    assert!(!w.draft_valid());
    assert!(w.action("save").is_err());
    apply(&mut w, Edit::SetEveryMinutes { minutes: None });
    assert!(w.draft_valid());
}
