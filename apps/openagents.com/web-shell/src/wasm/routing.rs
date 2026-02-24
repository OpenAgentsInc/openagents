use super::*;

    pub(super) fn sync_thread_route_from_state(state: &AppState) {
        CODEX_THREAD_STATE.with(|chat| {
            chat.borrow_mut()
                .set_thread_id(thread_id_from_route(&state.route));
        });
    }

    pub(super) fn apply_route_transition(route: AppRoute, push_history: bool) {
        remember_route_scroll_position();
        APP_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let _ = apply_action(&mut state, AppAction::Navigate { route });
            update_diagnostics_from_state(state.route.to_path(), state.intent_queue.len());
        });
        let state = snapshot_state();
        if push_history {
            push_route_to_browser_history(&state.route);
        }
        sync_thread_route_from_state(&state);
        schedule_codex_history_refresh();
        schedule_management_surface_refresh();
        schedule_settings_surface_refresh();
        schedule_l402_surface_refresh();
        schedule_admin_worker_surface_refresh();
        render_codex_chat_dom();
    }

    pub(super) fn remember_route_scroll_position() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        let Some(messages_container) = document.get_element_by_id(CODEX_CHAT_MESSAGES_ID) else {
            return;
        };
        let Ok(messages_container) = messages_container.dyn_into::<HtmlElement>() else {
            return;
        };
        let route = APP_STATE.with(|state| state.borrow().route.clone());
        remember_scroll_position_for_route(&messages_container, &route);
    }

    pub(super) fn remember_scroll_position_for_route(messages_container: &HtmlElement, route: &AppRoute) {
        let route_path = route.to_path();
        ROUTE_SCROLL_POSITIONS.with(|positions| {
            let mut positions = positions.borrow_mut();
            positions.insert(route_path, messages_container.scroll_top());
            while positions.len() > ROUTE_SCROLL_POSITION_CACHE_LIMIT {
                let Some(key_to_remove) = positions.keys().next().cloned() else {
                    break;
                };
                positions.remove(&key_to_remove);
            }
        });
    }

    pub(super) fn restore_route_scroll_position(
        messages_container: &HtmlElement,
        route: &AppRoute,
        fallback: i32,
    ) {
        let route_path = route.to_path();
        let scroll_top =
            ROUTE_SCROLL_POSITIONS.with(|positions| positions.borrow().get(&route_path).copied());
        messages_container.set_scroll_top(scroll_top.unwrap_or(fallback));
    }

    pub(super) fn install_browser_navigation_handlers() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };

        ROUTE_POPSTATE_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                let route = AppRoute::from_path(&current_pathname());
                apply_route_transition(route, false);
            }));
            let _ = window
                .add_event_listener_with_callback("popstate", callback.as_ref().unchecked_ref());
            *slot.borrow_mut() = Some(callback);
        });

        ROUTE_LINK_CLICK_HANDLER.with(|slot| {
            if slot.borrow().is_some() {
                return;
            }
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |event| {
                intercept_internal_link_click(event);
            }));
            let _ = document.add_event_listener_with_callback_and_bool(
                "click",
                callback.as_ref().unchecked_ref(),
                true,
            );
            *slot.borrow_mut() = Some(callback);
        });
    }

    pub(super) fn intercept_internal_link_click(event: web_sys::Event) {
        if event.default_prevented() {
            return;
        }
        let Some(mouse_event) = event.dyn_ref::<MouseEvent>() else {
            return;
        };
        if mouse_event.button() != 0
            || mouse_event.meta_key()
            || mouse_event.ctrl_key()
            || mouse_event.shift_key()
            || mouse_event.alt_key()
        {
            return;
        }

        let Some(anchor) = anchor_from_event(&event) else {
            return;
        };
        let href_attribute = anchor.get_attribute("href").unwrap_or_default();
        if href_attribute.trim().is_empty() || href_attribute.starts_with('#') {
            return;
        }
        if anchor.has_attribute("download") {
            return;
        }
        let target = anchor.target();
        if !target.is_empty() && target != "_self" {
            return;
        }

        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(origin) = window.location().origin() else {
            return;
        };

        let href = anchor.href();
        if href.is_empty() {
            return;
        }

        let path_with_query_and_hash = if href.starts_with(&origin) {
            href.strip_prefix(&origin).unwrap_or_default().to_string()
        } else if href.starts_with('/') {
            href
        } else {
            return;
        };

        let path_before_query = path_with_query_and_hash
            .split('?')
            .next()
            .unwrap_or_default();
        let path = path_before_query
            .split('#')
            .next()
            .unwrap_or(path_before_query);
        if path.is_empty() || !is_internal_shell_route_path(path) {
            return;
        }

        event.prevent_default();
        let route = AppRoute::from_path(path);
        apply_route_transition(route, true);
    }

    pub(super) fn anchor_from_event(event: &web_sys::Event) -> Option<HtmlAnchorElement> {
        let composed_path = event.composed_path();
        for index in 0..composed_path.length() {
            let value = composed_path.get(index);
            if let Ok(anchor) = value.dyn_into::<HtmlAnchorElement>() {
                return Some(anchor);
            }
        }
        None
    }

    pub(super) fn is_internal_shell_route_path(path: &str) -> bool {
        path == "/"
            || path == "/feed"
            || path == "/chat"
            || path.starts_with("/chat/")
            || path == "/login"
            || path == "/register"
            || path == "/authenticate"
            || path == "/workers"
            || path == "/debug"
            || path == "/onboarding"
            || path.starts_with("/onboarding/")
            || path == "/account"
            || path.starts_with("/account/")
            || path == "/settings"
            || path.starts_with("/settings/")
            || path == "/l402"
            || path.starts_with("/l402/")
            || path == "/billing"
            || path.starts_with("/billing/")
            || path == "/admin"
            || path.starts_with("/admin/")
    }

    pub(super) fn push_route_to_browser_history(route: &AppRoute) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let route_path = route.to_path();
        if current_pathname() == route_path {
            return;
        }
        let _ = history.push_state_with_url(&JsValue::NULL, "", Some(&route_path));
    }

    pub(super) fn replace_route_in_browser_history(route: &AppRoute) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(history) = window.history() else {
            return;
        };
        let route_path = route.to_path();
        if current_pathname() == route_path {
            return;
        }
        let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&route_path));
    }

    pub(super) fn thread_id_from_route(route: &AppRoute) -> Option<String> {
        match route {
            AppRoute::Chat { thread_id } => thread_id.clone(),
            AppRoute::Home
            | AppRoute::Feed
            | AppRoute::Login
            | AppRoute::Register
            | AppRoute::Authenticate
            | AppRoute::Onboarding { .. }
            | AppRoute::Workers
            | AppRoute::Account { .. }
            | AppRoute::Settings { .. }
            | AppRoute::Billing { .. }
            | AppRoute::Admin { .. }
            | AppRoute::Debug => None,
        }
    }

    pub(super) fn route_is_management_surface(route: &AppRoute) -> bool {
        matches!(
            route,
            AppRoute::Account { .. }
                | AppRoute::Settings { .. }
                | AppRoute::Billing { .. }
                | AppRoute::Admin { .. }
        )
    }

    pub(super) fn route_is_settings_surface(route: &AppRoute) -> bool {
        matches!(route, AppRoute::Settings { .. })
    }

    pub(super) fn route_is_l402_surface(route: &AppRoute) -> bool {
        matches!(route, AppRoute::Billing { .. })
    }

    pub(super) fn route_is_admin_surface(route: &AppRoute) -> bool {
        matches!(route, AppRoute::Admin { .. })
    }

    pub(super) fn route_is_codex_chat_surface(route: &AppRoute) -> bool {
        matches!(
            route,
            AppRoute::Home | AppRoute::Feed | AppRoute::Chat { .. }
        )
    }

    pub(super) fn route_is_auth_surface(route: &AppRoute) -> bool {
        matches!(
            route,
            AppRoute::Login
                | AppRoute::Register
                | AppRoute::Authenticate
                | AppRoute::Onboarding { .. }
        )
    }

