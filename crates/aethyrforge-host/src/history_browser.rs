//! Read saved changes and existing credits without replacing the running world.
use super::*;
#[derive(Default)]
pub(super) struct View {
    rows: Vec<Contribution>,
    shown: usize,
    loading: bool,
    next: f64,
    turn: u64,
    credits: BTreeMap<u64, ContributionEarnings>,
    credit_busy: bool,
    inspection: VersionInspection,
    inspected: Option<ProjectDocument>,
    session: String,
}
pub(super) fn clear() {
    element("project-history-rows").set_text_content(None);
    element("project-history-changes").set_text_content(None);
    element("project-history-version").set_text_content(None);
    element("project-history-change-limit").set_text_content(None);
    hide("project-history-download", true);
    hide("project-history-detail", true);
    hide("project-history-more", true);
    element("project-history-status")
        .set_text_content(Some("Choose a project to read its saved changes."));
}
fn child(parent: &web_sys::Element, tag: &str, text: &str) -> Result<web_sys::Element, JsValue> {
    let e = window().document().unwrap().create_element(tag)?;
    e.set_text_content(Some(text));
    parent.append_child(&e)?;
    Ok(e)
}
fn amount(c: &RecordedCredit) -> String {
    if c.unit != "msat" {
        return "Amount unavailable".into();
    }
    let Ok(value) = c.amount.parse::<u64>() else {
        return "Amount unavailable".into();
    };
    let remainder = value % 1000;
    let fraction = if remainder == 0 {
        String::new()
    } else {
        format!(".{remainder:03}").trim_end_matches('0').to_string()
    };
    format!("{}{fraction} sats", value / 1000)
}
fn credit_text(value: &ContributionEarnings) -> String {
    match value {
        ContributionEarnings::NotRecorded => "Earnings: Not recorded".into(),
        ContributionEarnings::Lookup => "Checking recorded earnings…".into(),
        ContributionEarnings::Unavailable => {
            "Earnings couldn't be reached. Refresh to try again.".into()
        }
        ContributionEarnings::Recorded { credits } => credits
            .iter()
            .map(|c| {
                format!(
                    "{}{} · {}",
                    if c.test_ledger { "Test ledger · " } else { "" },
                    amount(c),
                    match c.state {
                        CreditState::Pending => "Pending credit",
                        CreditState::Confirmed => "Confirmed credit",
                        CreditState::Paid => "Paid",
                        CreditState::Reversed => "Reversed",
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
fn render(s: &State) -> Result<(), JsValue> {
    let view = &s.projects.history;
    let list = element("project-history-rows");
    list.set_text_content(None);
    for row in view.rows.iter().rev().take(view.shown) {
        let li = child(&list, "li", "")?;
        child(&li, "strong", &row.actor.name)?;
        if let Some(profile) = &row.actor.profile {
            let a = child(&li, "a", "Profile")?;
            a.set_attribute("href", profile)?;
            a.set_attribute("target", "_blank")?;
            a.set_attribute("rel", "noopener noreferrer")?;
        }
        let date = js_sys::Date::new(&JsValue::from_f64(row.created_unix as f64 * 1000.))
            .to_iso_string()
            .as_string()
            .unwrap_or_default()
            .replace('T', " ")
            .replace('Z', " UTC");
        child(
            &li,
            "p",
            &format!("{} · Version {} · {}", row.summary, row.version, date),
        )?;
        child(
            &li,
            "p",
            if row.source_state == "draft" {
                "Draft saved · Source changes still need compiling"
            } else {
                "Saved content · Editor result not recorded here"
            },
        )?;
        let button = child(&li, "button", "Inspect saved version")?;
        button.set_attribute("type", "button")?;
        button.set_attribute("data-inspect-version", &row.version.to_string())?;
        let earnings = view.credits.get(&row.version).unwrap_or(&row.earnings);
        let p = child(&li, "p", &credit_text(earnings))?;
        p.set_attribute("id", &format!("contribution-credit-{}", row.id))?;
        if let ContributionEarnings::Recorded { credits } = earnings {
            for c in credits {
                let a = child(&li, "a", "Recorded receipt")?;
                a.set_attribute("href", &format!("/event/{}", c.receipt))?;
                a.set_attribute("target", "_blank")?;
                a.set_attribute("rel", "noopener noreferrer")?;
            }
        }
    }
    hide("project-history-more", view.shown >= view.rows.len());
    Ok(())
}
async fn refresh(weak: Weak<RefCell<State>>) {
    let Some(rc) = weak.upgrade() else { return };
    let (scope, project, turn) = {
        let mut s = rc.borrow_mut();
        if s.projects.history.loading {
            return;
        }
        if s.account.is_none() || s.projects.project.is_empty() {
            clear();
            return;
        }
        let Some(scope) = confirmation_scope(&s) else {
            return;
        };
        s.projects.history.loading = true;
        s.projects.history.turn = s.projects.history.turn.wrapping_add(1);
        s.projects.history.next = js_sys::Date::now() + 15000.;
        (scope, s.projects.project.clone(), s.projects.history.turn)
    };
    element("project-history-status").set_text_content(Some("Loading saved changes…"));
    let result = project_api(&weak, &format!("{}/history", project_path(&project)), None).await;
    let mut s = rc.borrow_mut();
    if !scope.accepts(confirmation_scope(&s).as_ref()) || s.projects.history.turn != turn {
        return;
    }
    s.projects.history.loading = false;
    s.projects.history.credit_busy = false;
    s.projects.history.credits.clear();
    match result.and_then(|v| {
        serde_json::from_value::<Vec<Contribution>>(v["data"].clone()).map_err(|_| JsValue::NULL)
    }) {
        Ok(rows) if rows.len() <= 128 => {
            s.projects.history.rows = rows;
            s.projects.history.shown = s.projects.history.shown.max(20);
            element("project-history-status").set_text_content(Some(if s.projects.history.rows.is_empty(){"No saved changes yet. Save your project to record a version."}else{"Saved changes are shared with this project's members. This visit shows editor results below."}));
            let _ = render(&s);
        }
        _ => {
            s.projects.history.rows.clear();
            element("project-history-rows").set_text_content(None);
            element("project-history-status").set_text_content(Some(
                "Saved changes couldn't be reached. Your world and drafts are still here.",
            ));
        }
    }
}
async fn inspect(weak: Weak<RefCell<State>>, version: u64) {
    let Some(rc) = weak.upgrade() else { return };
    let (scope, project, request) = {
        let mut s = rc.borrow_mut();
        let Some(scope) = confirmation_scope(&s) else {
            return;
        };
        let request = s.projects.history.inspection.request(version);
        s.projects.history.inspected = None;
        (scope, s.projects.project.clone(), request)
    };
    hide("project-history-detail", false);
    hide("project-history-download", true);
    element("project-history-version").set_text_content(Some(&format!("Version {version}")));
    element("project-history-changes").set_text_content(None);
    element("project-history-change-limit").set_text_content(Some("Loading changes…"));
    let result = project_api(
        &weak,
        &format!("{}/history/{version}", project_path(&project)),
        None,
    )
    .await;
    let mut s = rc.borrow_mut();
    if !scope.accepts(confirmation_scope(&s).as_ref())
        || !s.projects.history.inspection.accepts(request, version)
    {
        return;
    }
    let parsed = result.and_then(|value| {
        let document: ProjectDocument =
            serde_json::from_value(value["document"].clone()).map_err(|_| JsValue::NULL)?;
        document.validate().map_err(|_| JsValue::NULL)?;
        Ok((document, value["changes"].clone()))
    });
    match parsed {
        Ok((document, changes)) => {
            let rows = review_changes(&document, &changes);
            let list = element("project-history-changes");
            for row in &rows {
                if let Ok(li) = child(&list, "li", "") {
                    let _ = child(&li, "strong", &row.label);
                    let _ = child(&li, "span", &format!(": {} → {}", row.before, row.after));
                }
            }
            let extra = changes.as_array().is_some_and(|a| a.len() > rows.len());
            element("project-history-change-limit").set_text_content(Some(if extra {
                "Showing the first changes. Download the editable version to review all content and source."
            } else if rows.is_empty() { "No content changes in this version." } else { "Download the editable version to review its full source." }));
            s.projects.history.inspected = Some(document);
            hide("project-history-download", false);
        }
        Err(_) => element("project-history-change-limit")
            .set_text_content(Some("This saved version couldn't be reached. Try again.")),
    }
}

pub(super) fn mount(
    rc: &Rc<RefCell<State>>,
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
) -> Result<(), JsValue> {
    hide("contribution-tools", false);
    hide("visit-earnings", true);
    clear();
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-history-download", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        let Some(document) = rc.borrow().projects.history.inspected.clone() else {
            return;
        };
        let project = rc.borrow().projects.project.clone();
        let turn = rc.borrow().projects.history.turn;
        let weak = Rc::downgrade(&rc);
        spawn_local(async move {
            match files::saved_archive(&weak, &project, document).await {
                Ok(archive) => {
                    let _ = files::download(&archive);
                }
                Err(_) => {
                    if let Some(rc) = weak.upgrade() {
                        if rc.borrow().projects.history.turn == turn {
                            element("project-history-status").set_text_content(Some("The version's source images could not open. Try again before leaving this world."));
                        }
                    }
                }
            }
        });
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-history-refresh", "click", move |_| {
        spawn_local(refresh(weak.clone()));
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-history-more", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            s.projects.history.shown = (s.projects.history.shown + 20).min(128);
            let _ = render(&s);
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-history-rows", "click", move |event| {
        let Some(target) = event
            .target()
            .and_then(|v| v.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let Some(button) = target.closest("[data-inspect-version]").ok().flatten() else {
            return;
        };
        let Some(version) = button
            .get_attribute("data-inspect-version")
            .and_then(|v| v.parse::<u64>().ok())
        else {
            return;
        };
        spawn_local(inspect(weak.clone(), version));
    })?;
    Ok(())
}
pub(super) fn poll(rc: &Rc<RefCell<State>>) {
    let mut s = rc.borrow_mut();
    let session = s.snapshot()["session"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if s.projects.history.session != session {
        s.projects.history = View {
            session,
            ..Default::default()
        };
        clear();
    }
    if !s.visit.open
        || !s.visit.panels.iter().any(|p| p == "history")
        || s.account.is_none()
        || s.projects.project.is_empty()
    {
        return;
    }
    if !s.projects.history.loading && js_sys::Date::now() >= s.projects.history.next {
        drop(s);
        spawn_local(refresh(Rc::downgrade(rc)));
        return;
    }
    if s.projects.history.credit_busy {
        return;
    }
    let row = s
        .projects
        .history
        .rows
        .iter()
        .rev()
        .take(s.projects.history.shown)
        .find(|r| {
            matches!(r.earnings, ContributionEarnings::Lookup)
                && !s.projects.history.credits.contains_key(&r.version)
        })
        .cloned();
    let Some(row) = row else { return };
    let Some(scope) = confirmation_scope(&s) else {
        return;
    };
    let project = s.projects.project.clone();
    let turn = s.projects.history.turn;
    drop(s);
    rc.borrow_mut().projects.history.credit_busy = true;
    let weak = Rc::downgrade(rc);
    spawn_local(async move {
        let result = project_api(
            &weak,
            &format!(
                "{}/history/{}/earnings",
                project_path(&project),
                row.version
            ),
            None,
        )
        .await;
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        if !scope.accepts(confirmation_scope(&s).as_ref()) || s.projects.history.turn != turn {
            return;
        }
        s.projects.history.credit_busy = false;
        let earnings = result
            .ok()
            .filter(|v| v["contribution"] == row.id)
            .and_then(|v| {
                serde_json::from_value::<ContributionEarnings>(v["earnings"].clone()).ok()
            })
            .unwrap_or(ContributionEarnings::Unavailable);
        s.projects.history.credits.insert(row.version, earnings);
        let _ = render(&s);
    });
}
