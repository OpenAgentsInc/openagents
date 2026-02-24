use super::*;

pub(super) fn ensure_codex_chat_dom() -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "document is unavailable".to_string())?;
    let body = document
        .body()
        .ok_or_else(|| "document body is unavailable".to_string())?;

    if document.get_element_by_id(CODEX_CHAT_ROOT_ID).is_none() {
        let root = document
            .create_element("section")
            .map_err(|_| "failed to create codex chat root".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex chat root is not HtmlElement".to_string())?;
        root.set_id(CODEX_CHAT_ROOT_ID);
        root.style()
            .set_property("position", "fixed")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("inset", "0")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("display", "none")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("flex-direction", "column")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("justify-content", "space-between")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("padding", "72px 16px 108px")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("box-sizing", "border-box")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("z-index", "20")
            .map_err(|_| "failed to style codex chat root".to_string())?;
        root.style()
            .set_property("pointer-events", "none")
            .map_err(|_| "failed to style codex chat root".to_string())?;

        let header = document
            .create_element("div")
            .map_err(|_| "failed to create codex chat header".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex chat header is not HtmlElement".to_string())?;
        header.set_id(CODEX_CHAT_HEADER_ID);
        header
            .style()
            .set_property("color", "#cbd5e1")
            .map_err(|_| "failed to style codex chat header".to_string())?;
        header
            .style()
            .set_property("font-size", "12px")
            .map_err(|_| "failed to style codex chat header".to_string())?;
        header
            .style()
            .set_property(
                "font-family",
                "ui-monospace, SFMono-Regular, Menlo, monospace",
            )
            .map_err(|_| "failed to style codex chat header".to_string())?;
        header
            .style()
            .set_property("pointer-events", "none")
            .map_err(|_| "failed to style codex chat header".to_string())?;
        let _ = root.append_child(&header);

        let messages = document
            .create_element("div")
            .map_err(|_| "failed to create codex chat messages".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex chat messages is not HtmlElement".to_string())?;
        messages.set_id(CODEX_CHAT_MESSAGES_ID);
        messages
            .style()
            .set_property("display", "flex")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("flex-direction", "column")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("gap", "10px")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("max-width", "760px")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("width", "100%")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("margin", "0 auto")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("overflow-y", "auto")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("padding-right", "4px")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        messages
            .style()
            .set_property("pointer-events", "auto")
            .map_err(|_| "failed to style codex chat messages".to_string())?;
        let _ = root.append_child(&messages);

        let quick_prompts = document
            .create_element("div")
            .map_err(|_| "failed to create codex quick prompts".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex quick prompts is not HtmlElement".to_string())?;
        quick_prompts.set_id(CODEX_CHAT_QUICK_PROMPTS_ID);
        let _ = quick_prompts.set_attribute("role", "group");
        quick_prompts
            .style()
            .set_property("display", "none")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("flex-wrap", "wrap")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("gap", "8px")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("max-width", "760px")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("width", "100%")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("margin", "0 auto")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;
        quick_prompts
            .style()
            .set_property("pointer-events", "auto")
            .map_err(|_| "failed to style codex quick prompts".to_string())?;

        for (index, prompt) in CHAT_QUICK_PROMPTS.iter().enumerate() {
            let prompt_button = document
                .create_element("button")
                .map_err(|_| "failed to create codex quick prompt button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "codex quick prompt button is not HtmlElement".to_string())?;
            prompt_button.set_id(CHAT_QUICK_PROMPT_IDS[index]);
            let _ = prompt_button.set_attribute("type", "button");
            prompt_button.set_inner_text(prompt);
            let _ = prompt_button.style().set_property("padding", "6px 10px");
            let _ = prompt_button.style().set_property("border-radius", "999px");
            let _ = prompt_button
                .style()
                .set_property("border", "1px solid #1f2937");
            let _ = prompt_button.style().set_property("background", "#111827");
            let _ = prompt_button.style().set_property("color", "#cbd5e1");
            let _ = prompt_button.style().set_property("font-size", "13px");
            let _ = prompt_button.style().set_property("line-height", "1.2");
            let _ = prompt_button.style().set_property("cursor", "pointer");
            let _ = quick_prompts.append_child(&prompt_button);
        }
        let _ = root.append_child(&quick_prompts);

        let composer = document
            .create_element("div")
            .map_err(|_| "failed to create codex composer".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex composer is not HtmlElement".to_string())?;
        composer.set_id(CODEX_CHAT_COMPOSER_ID);
        composer
            .style()
            .set_property("display", "flex")
            .map_err(|_| "failed to style codex composer".to_string())?;
        composer
            .style()
            .set_property("gap", "8px")
            .map_err(|_| "failed to style codex composer".to_string())?;
        composer
            .style()
            .set_property("max-width", "760px")
            .map_err(|_| "failed to style codex composer".to_string())?;
        composer
            .style()
            .set_property("margin", "0 auto")
            .map_err(|_| "failed to style codex composer".to_string())?;
        composer
            .style()
            .set_property("width", "100%")
            .map_err(|_| "failed to style codex composer".to_string())?;
        composer
            .style()
            .set_property("pointer-events", "auto")
            .map_err(|_| "failed to style codex composer".to_string())?;

        let input = document
            .create_element("input")
            .map_err(|_| "failed to create codex input".to_string())?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| "codex input is not HtmlInputElement".to_string())?;
        input.set_id(CODEX_CHAT_INPUT_ID);
        input.set_placeholder("Message Codex");
        let _ = input.set_attribute("aria-label", "Message Codex");
        let _ = input.style().set_property("flex", "1");
        let _ = input.style().set_property("height", "40px");
        let _ = input.style().set_property("padding", "0 12px");
        let _ = input.style().set_property("border-radius", "10px");
        let _ = input.style().set_property("border", "1px solid #1f2937");
        let _ = input.style().set_property("background", "#0f172a");
        let _ = input.style().set_property("color", "#e2e8f0");
        let _ = input.style().set_property("font-size", "15px");
        let _ = input.style().set_property(
            "font-family",
            "-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif",
        );
        let _ = composer.append_child(&input);

        let send_button = document
            .create_element("button")
            .map_err(|_| "failed to create codex send button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "codex send button is not HtmlElement".to_string())?;
        send_button.set_id(CODEX_CHAT_SEND_ID);
        let _ = send_button.set_attribute("type", "button");
        send_button.set_inner_text("Send");
        let _ = send_button.style().set_property("height", "40px");
        let _ = send_button.style().set_property("padding", "0 16px");
        let _ = send_button.style().set_property("border-radius", "10px");
        let _ = send_button
            .style()
            .set_property("border", "1px solid #2563eb");
        let _ = send_button.style().set_property("background", "#2563eb");
        let _ = send_button.style().set_property("color", "#ffffff");
        let _ = send_button.style().set_property("font-weight", "600");
        let _ = composer.append_child(&send_button);

        let _ = root.append_child(&composer);

        let auth_panel = document
            .create_element("div")
            .map_err(|_| "failed to create auth panel".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth panel is not HtmlElement".to_string())?;
        auth_panel.set_id(AUTH_PANEL_ID);
        let _ = auth_panel.style().set_property("display", "none");
        let _ = auth_panel.style().set_property("flex-direction", "column");
        let _ = auth_panel.style().set_property("gap", "8px");
        let _ = auth_panel.style().set_property("max-width", "760px");
        let _ = auth_panel.style().set_property("margin", "0 auto");
        let _ = auth_panel.style().set_property("width", "100%");
        let _ = auth_panel.style().set_property("pointer-events", "auto");

        let auth_email_row = document
            .create_element("div")
            .map_err(|_| "failed to create auth email row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth email row is not HtmlElement".to_string())?;
        let _ = auth_email_row.style().set_property("display", "flex");
        let _ = auth_email_row.style().set_property("gap", "8px");

        let auth_email_input = document
            .create_element("input")
            .map_err(|_| "failed to create auth email input".to_string())?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| "auth email input is not HtmlInputElement".to_string())?;
        auth_email_input.set_id(AUTH_EMAIL_INPUT_ID);
        auth_email_input.set_placeholder("Email");
        let _ = auth_email_input.style().set_property("flex", "1");
        let _ = auth_email_input.style().set_property("height", "40px");
        let _ = auth_email_input.style().set_property("padding", "0 12px");
        let _ = auth_email_input
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_email_input
            .style()
            .set_property("border", "1px solid #1f2937");
        let _ = auth_email_input
            .style()
            .set_property("background", "#0f172a");
        let _ = auth_email_input.style().set_property("color", "#e2e8f0");
        let _ = auth_email_row.append_child(&auth_email_input);

        let auth_send_button = document
            .create_element("button")
            .map_err(|_| "failed to create auth send button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth send button is not HtmlElement".to_string())?;
        auth_send_button.set_id(AUTH_SEND_ID);
        let _ = auth_send_button.set_attribute("type", "button");
        auth_send_button.set_inner_text("Send code");
        let _ = auth_send_button.style().set_property("height", "40px");
        let _ = auth_send_button.style().set_property("padding", "0 14px");
        let _ = auth_send_button
            .style()
            .set_property("border", "1px solid #2563eb");
        let _ = auth_send_button
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_send_button
            .style()
            .set_property("background", "#2563eb");
        let _ = auth_send_button.style().set_property("color", "#ffffff");
        let _ = auth_email_row.append_child(&auth_send_button);
        let _ = auth_panel.append_child(&auth_email_row);

        let auth_code_row = document
            .create_element("div")
            .map_err(|_| "failed to create auth code row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth code row is not HtmlElement".to_string())?;
        let _ = auth_code_row.style().set_property("display", "flex");
        let _ = auth_code_row.style().set_property("gap", "8px");

        let auth_code_input = document
            .create_element("input")
            .map_err(|_| "failed to create auth code input".to_string())?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| "auth code input is not HtmlInputElement".to_string())?;
        auth_code_input.set_id(AUTH_CODE_INPUT_ID);
        auth_code_input.set_placeholder("Verification code");
        let _ = auth_code_input.style().set_property("flex", "1");
        let _ = auth_code_input.style().set_property("height", "40px");
        let _ = auth_code_input.style().set_property("padding", "0 12px");
        let _ = auth_code_input
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_code_input
            .style()
            .set_property("border", "1px solid #1f2937");
        let _ = auth_code_input
            .style()
            .set_property("background", "#0f172a");
        let _ = auth_code_input.style().set_property("color", "#e2e8f0");
        let _ = auth_code_row.append_child(&auth_code_input);

        let auth_verify_button = document
            .create_element("button")
            .map_err(|_| "failed to create auth verify button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth verify button is not HtmlElement".to_string())?;
        auth_verify_button.set_id(AUTH_VERIFY_ID);
        let _ = auth_verify_button.set_attribute("type", "button");
        auth_verify_button.set_inner_text("Verify");
        let _ = auth_verify_button.style().set_property("height", "40px");
        let _ = auth_verify_button.style().set_property("padding", "0 14px");
        let _ = auth_verify_button
            .style()
            .set_property("border", "1px solid #10b981");
        let _ = auth_verify_button
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_verify_button
            .style()
            .set_property("background", "#10b981");
        let _ = auth_verify_button.style().set_property("color", "#ffffff");
        let _ = auth_code_row.append_child(&auth_verify_button);
        let _ = auth_panel.append_child(&auth_code_row);

        let auth_action_row = document
            .create_element("div")
            .map_err(|_| "failed to create auth action row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth action row is not HtmlElement".to_string())?;
        let _ = auth_action_row.style().set_property("display", "flex");
        let _ = auth_action_row.style().set_property("gap", "8px");

        let auth_restore_button = document
            .create_element("button")
            .map_err(|_| "failed to create auth restore button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth restore button is not HtmlElement".to_string())?;
        auth_restore_button.set_id(AUTH_RESTORE_ID);
        let _ = auth_restore_button.set_attribute("type", "button");
        auth_restore_button.set_inner_text("Restore session");
        let _ = auth_restore_button.style().set_property("height", "36px");
        let _ = auth_restore_button
            .style()
            .set_property("padding", "0 12px");
        let _ = auth_restore_button
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_restore_button
            .style()
            .set_property("border", "1px solid #1f2937");
        let _ = auth_restore_button
            .style()
            .set_property("background", "#111827");
        let _ = auth_restore_button.style().set_property("color", "#cbd5e1");
        let _ = auth_action_row.append_child(&auth_restore_button);

        let auth_logout_button = document
            .create_element("button")
            .map_err(|_| "failed to create auth logout button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "auth logout button is not HtmlElement".to_string())?;
        auth_logout_button.set_id(AUTH_LOGOUT_ID);
        let _ = auth_logout_button.set_attribute("type", "button");
        auth_logout_button.set_inner_text("Sign out");
        let _ = auth_logout_button.style().set_property("height", "36px");
        let _ = auth_logout_button.style().set_property("padding", "0 12px");
        let _ = auth_logout_button
            .style()
            .set_property("border-radius", "10px");
        let _ = auth_logout_button
            .style()
            .set_property("border", "1px solid #7f1d1d");
        let _ = auth_logout_button
            .style()
            .set_property("background", "#7f1d1d");
        let _ = auth_logout_button.style().set_property("color", "#ffffff");
        let _ = auth_action_row.append_child(&auth_logout_button);
        let _ = auth_panel.append_child(&auth_action_row);

        let _ = root.append_child(&auth_panel);

        let settings_panel = document
            .create_element("div")
            .map_err(|_| "failed to create settings panel".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings panel is not HtmlElement".to_string())?;
        settings_panel.set_id(SETTINGS_PANEL_ID);
        let _ = settings_panel.style().set_property("display", "none");
        let _ = settings_panel
            .style()
            .set_property("flex-direction", "column");
        let _ = settings_panel.style().set_property("gap", "10px");
        let _ = settings_panel.style().set_property("max-width", "760px");
        let _ = settings_panel.style().set_property("margin", "0 auto");
        let _ = settings_panel.style().set_property("width", "100%");
        let _ = settings_panel
            .style()
            .set_property("pointer-events", "auto");
        let _ = settings_panel
            .style()
            .set_property("padding", "8px 0 4px 0");

        let settings_status = document
            .create_element("div")
            .map_err(|_| "failed to create settings status".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings status is not HtmlElement".to_string())?;
        settings_status.set_id(SETTINGS_STATUS_ID);
        let _ = settings_status.style().set_property("min-height", "20px");
        let _ = settings_status.style().set_property("font-size", "12px");
        let _ = settings_status.style().set_property("color", "#93c5fd");
        settings_status.set_inner_text("Settings ready.");
        let _ = settings_panel.append_child(&settings_status);

        let profile_row = document
            .create_element("div")
            .map_err(|_| "failed to create settings profile row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings profile row is not HtmlElement".to_string())?;
        let _ = profile_row.style().set_property("display", "flex");
        let _ = profile_row.style().set_property("gap", "8px");
        let _ = profile_row.style().set_property("align-items", "center");

        let profile_name_input = document
            .create_element("input")
            .map_err(|_| "failed to create settings profile input".to_string())?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| "settings profile input is not HtmlInputElement".to_string())?;
        profile_name_input.set_id(SETTINGS_PROFILE_NAME_ID);
        profile_name_input.set_placeholder("Profile name");
        let _ = profile_name_input.style().set_property("flex", "1");
        let _ = profile_name_input.style().set_property("height", "36px");
        let _ = profile_name_input.style().set_property("padding", "0 10px");
        let _ = profile_name_input
            .style()
            .set_property("border-radius", "8px");
        let _ = profile_name_input
            .style()
            .set_property("border", "1px solid #1f2937");
        let _ = profile_name_input
            .style()
            .set_property("background", "#0f172a");
        let _ = profile_name_input.style().set_property("color", "#e2e8f0");
        let _ = profile_row.append_child(&profile_name_input);

        let profile_save_button = document
            .create_element("button")
            .map_err(|_| "failed to create settings profile save button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings profile save button is not HtmlElement".to_string())?;
        profile_save_button.set_id(SETTINGS_PROFILE_SAVE_ID);
        profile_save_button.set_inner_text("Save Profile");
        let _ = profile_save_button.style().set_property("height", "36px");
        let _ = profile_save_button
            .style()
            .set_property("padding", "0 12px");
        let _ = profile_save_button
            .style()
            .set_property("border-radius", "8px");
        let _ = profile_save_button
            .style()
            .set_property("border", "1px solid #0f766e");
        let _ = profile_save_button
            .style()
            .set_property("background", "#0f766e");
        let _ = profile_save_button.style().set_property("color", "#ffffff");
        let _ = profile_row.append_child(&profile_save_button);

        let profile_delete_button = document
            .create_element("button")
            .map_err(|_| "failed to create settings profile delete button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings profile delete button is not HtmlElement".to_string())?;
        profile_delete_button.set_id(SETTINGS_PROFILE_DELETE_ID);
        profile_delete_button.set_inner_text("Delete Profile");
        let _ = profile_delete_button.style().set_property("height", "36px");
        let _ = profile_delete_button
            .style()
            .set_property("padding", "0 12px");
        let _ = profile_delete_button
            .style()
            .set_property("border-radius", "8px");
        let _ = profile_delete_button
            .style()
            .set_property("border", "1px solid #7f1d1d");
        let _ = profile_delete_button
            .style()
            .set_property("background", "#7f1d1d");
        let _ = profile_delete_button
            .style()
            .set_property("color", "#ffffff");
        let _ = profile_row.append_child(&profile_delete_button);
        let _ = settings_panel.append_child(&profile_row);

        let autopilot_row = document
            .create_element("div")
            .map_err(|_| "failed to create settings autopilot row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings autopilot row is not HtmlElement".to_string())?;
        let _ = autopilot_row.style().set_property("display", "grid");
        let _ = autopilot_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(220px, 1fr))",
        );
        let _ = autopilot_row.style().set_property("gap", "8px");

        for (id, placeholder) in [
            (SETTINGS_AUTOPILOT_DISPLAY_NAME_ID, "Autopilot display name"),
            (SETTINGS_AUTOPILOT_TAGLINE_ID, "Autopilot tagline"),
            (SETTINGS_AUTOPILOT_OWNER_ID, "Owner display name"),
            (SETTINGS_AUTOPILOT_PERSONA_ID, "Persona summary"),
            (SETTINGS_AUTOPILOT_VOICE_ID, "Autopilot voice"),
            (
                SETTINGS_AUTOPILOT_PRINCIPLES_ID,
                "Principles text (one per line)",
            ),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create settings autopilot input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "settings autopilot input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = autopilot_row.append_child(&input);
        }
        let _ = settings_panel.append_child(&autopilot_row);

        let autopilot_save_button = document
            .create_element("button")
            .map_err(|_| "failed to create settings autopilot save button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings autopilot save button is not HtmlElement".to_string())?;
        autopilot_save_button.set_id(SETTINGS_AUTOPILOT_SAVE_ID);
        autopilot_save_button.set_inner_text("Save Autopilot");
        let _ = autopilot_save_button.style().set_property("height", "34px");
        let _ = autopilot_save_button
            .style()
            .set_property("padding", "0 12px");
        let _ = autopilot_save_button
            .style()
            .set_property("border-radius", "8px");
        let _ = autopilot_save_button
            .style()
            .set_property("border", "1px solid #334155");
        let _ = autopilot_save_button
            .style()
            .set_property("background", "#334155");
        let _ = autopilot_save_button
            .style()
            .set_property("color", "#ffffff");
        let _ = settings_panel.append_child(&autopilot_save_button);

        let resend_row = document
            .create_element("div")
            .map_err(|_| "failed to create settings resend row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings resend row is not HtmlElement".to_string())?;
        let _ = resend_row.style().set_property("display", "grid");
        let _ = resend_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = resend_row.style().set_property("gap", "8px");
        for (id, placeholder) in [
            (SETTINGS_RESEND_KEY_ID, "Resend API key"),
            (SETTINGS_RESEND_EMAIL_ID, "Resend sender email"),
            (SETTINGS_RESEND_NAME_ID, "Resend sender name"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create settings resend input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "settings resend input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = resend_row.append_child(&input);
        }
        let _ = settings_panel.append_child(&resend_row);

        let resend_actions = document
            .create_element("div")
            .map_err(|_| "failed to create settings resend actions".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings resend actions is not HtmlElement".to_string())?;
        let _ = resend_actions.style().set_property("display", "flex");
        let _ = resend_actions.style().set_property("gap", "8px");
        for (id, label, bg, border) in [
            (
                SETTINGS_RESEND_CONNECT_ID,
                "Connect Resend",
                "#1d4ed8",
                "#1d4ed8",
            ),
            (
                SETTINGS_RESEND_DISCONNECT_ID,
                "Disconnect Resend",
                "#7f1d1d",
                "#7f1d1d",
            ),
            (
                SETTINGS_RESEND_TEST_ID,
                "Send Resend Test",
                "#374151",
                "#374151",
            ),
        ] {
            let button = document
                .create_element("button")
                .map_err(|_| "failed to create settings resend action button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "settings resend action button is not HtmlElement".to_string())?;
            button.set_id(id);
            button.set_inner_text(label);
            let _ = button.style().set_property("height", "34px");
            let _ = button.style().set_property("padding", "0 10px");
            let _ = button.style().set_property("border-radius", "8px");
            let _ = button
                .style()
                .set_property("border", &format!("1px solid {border}"));
            let _ = button.style().set_property("background", bg);
            let _ = button.style().set_property("color", "#ffffff");
            let _ = resend_actions.append_child(&button);
        }
        let _ = settings_panel.append_child(&resend_actions);

        let google_actions = document
            .create_element("div")
            .map_err(|_| "failed to create settings google actions".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "settings google actions is not HtmlElement".to_string())?;
        let _ = google_actions.style().set_property("display", "flex");
        let _ = google_actions.style().set_property("gap", "8px");
        for (id, label, bg, border) in [
            (
                SETTINGS_GOOGLE_CONNECT_ID,
                "Connect Google",
                "#166534",
                "#166534",
            ),
            (
                SETTINGS_GOOGLE_DISCONNECT_ID,
                "Disconnect Google",
                "#7f1d1d",
                "#7f1d1d",
            ),
        ] {
            let button = document
                .create_element("button")
                .map_err(|_| "failed to create settings google action button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "settings google action button is not HtmlElement".to_string())?;
            button.set_id(id);
            button.set_inner_text(label);
            let _ = button.style().set_property("height", "34px");
            let _ = button.style().set_property("padding", "0 10px");
            let _ = button.style().set_property("border-radius", "8px");
            let _ = button
                .style()
                .set_property("border", &format!("1px solid {border}"));
            let _ = button.style().set_property("background", bg);
            let _ = button.style().set_property("color", "#ffffff");
            let _ = google_actions.append_child(&button);
        }
        let _ = settings_panel.append_child(&google_actions);

        let _ = root.append_child(&settings_panel);

        let admin_panel = document
            .create_element("div")
            .map_err(|_| "failed to create admin panel".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin panel is not HtmlElement".to_string())?;
        admin_panel.set_id(ADMIN_PANEL_ID);
        let _ = admin_panel.style().set_property("display", "none");
        let _ = admin_panel.style().set_property("flex-direction", "column");
        let _ = admin_panel.style().set_property("gap", "8px");
        let _ = admin_panel.style().set_property("max-width", "760px");
        let _ = admin_panel.style().set_property("margin", "0 auto");
        let _ = admin_panel.style().set_property("width", "100%");
        let _ = admin_panel.style().set_property("pointer-events", "auto");
        let _ = admin_panel.style().set_property("padding", "8px 0 4px 0");

        let admin_status = document
            .create_element("div")
            .map_err(|_| "failed to create admin status".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin status is not HtmlElement".to_string())?;
        admin_status.set_id(ADMIN_STATUS_ID);
        let _ = admin_status.style().set_property("min-height", "20px");
        let _ = admin_status.style().set_property("font-size", "12px");
        let _ = admin_status.style().set_property("color", "#93c5fd");
        admin_status.set_inner_text("Admin worker controls ready.");
        let _ = admin_panel.append_child(&admin_status);

        let worker_row = document
            .create_element("div")
            .map_err(|_| "failed to create admin worker row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin worker row is not HtmlElement".to_string())?;
        let _ = worker_row.style().set_property("display", "grid");
        let _ = worker_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = worker_row.style().set_property("gap", "8px");

        for (id, placeholder) in [
            (ADMIN_WORKER_ID_ID, "worker_id (desktopw:local)"),
            (ADMIN_WORKSPACE_ID, "workspace_ref"),
            (ADMIN_ADAPTER_ID, "adapter"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create admin worker input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "admin worker input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = worker_row.append_child(&input);
        }

        for (id, label, bg, border) in [
            (ADMIN_CREATE_ID, "Create/Reattach", "#1d4ed8", "#1d4ed8"),
            (ADMIN_REFRESH_ID, "Refresh", "#334155", "#334155"),
        ] {
            let button = document
                .create_element("button")
                .map_err(|_| "failed to create admin worker action button".to_string())?
                .dyn_into::<HtmlElement>()
                .map_err(|_| "admin worker action button is not HtmlElement".to_string())?;
            button.set_id(id);
            button.set_inner_text(label);
            let _ = button.style().set_property("height", "34px");
            let _ = button.style().set_property("padding", "0 10px");
            let _ = button.style().set_property("border-radius", "8px");
            let _ = button
                .style()
                .set_property("border", &format!("1px solid {border}"));
            let _ = button.style().set_property("background", bg);
            let _ = button.style().set_property("color", "#ffffff");
            let _ = worker_row.append_child(&button);
        }
        let _ = admin_panel.append_child(&worker_row);

        let stop_row = document
            .create_element("div")
            .map_err(|_| "failed to create admin stop row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin stop row is not HtmlElement".to_string())?;
        let _ = stop_row.style().set_property("display", "grid");
        let _ = stop_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = stop_row.style().set_property("gap", "8px");

        for (id, placeholder) in [
            (ADMIN_STOP_REASON_ID, "stop reason"),
            (ADMIN_STOP_CONFIRM_ID, "type worker_id to confirm stop"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create admin stop input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "admin stop input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #7f1d1d");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#fecaca");
            let _ = stop_row.append_child(&input);
        }
        let stop_button = document
            .create_element("button")
            .map_err(|_| "failed to create admin stop button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin stop button is not HtmlElement".to_string())?;
        stop_button.set_id(ADMIN_STOP_ID);
        stop_button.set_inner_text("Stop Worker");
        let _ = stop_button.style().set_property("height", "34px");
        let _ = stop_button.style().set_property("padding", "0 10px");
        let _ = stop_button.style().set_property("border-radius", "8px");
        let _ = stop_button
            .style()
            .set_property("border", "1px solid #7f1d1d");
        let _ = stop_button.style().set_property("background", "#7f1d1d");
        let _ = stop_button.style().set_property("color", "#ffffff");
        let _ = stop_row.append_child(&stop_button);
        let _ = admin_panel.append_child(&stop_row);

        let request_row = document
            .create_element("div")
            .map_err(|_| "failed to create admin request row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin request row is not HtmlElement".to_string())?;
        let _ = request_row.style().set_property("display", "grid");
        let _ = request_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = request_row.style().set_property("gap", "8px");

        for (id, placeholder) in [
            (ADMIN_REQUEST_METHOD_ID, "method (thread/list)"),
            (ADMIN_REQUEST_ID_ID, "request_id (optional)"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create admin request input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "admin request input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = request_row.append_child(&input);
        }
        let request_send_button = document
            .create_element("button")
            .map_err(|_| "failed to create admin request send button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin request send button is not HtmlElement".to_string())?;
        request_send_button.set_id(ADMIN_REQUEST_SEND_ID);
        request_send_button.set_inner_text("Send Request");
        let _ = request_send_button.style().set_property("height", "34px");
        let _ = request_send_button
            .style()
            .set_property("padding", "0 10px");
        let _ = request_send_button
            .style()
            .set_property("border-radius", "8px");
        let _ = request_send_button
            .style()
            .set_property("border", "1px solid #166534");
        let _ = request_send_button
            .style()
            .set_property("background", "#166534");
        let _ = request_send_button.style().set_property("color", "#ffffff");
        let _ = request_row.append_child(&request_send_button);
        let _ = admin_panel.append_child(&request_row);

        let request_params_input = document
            .create_element("input")
            .map_err(|_| "failed to create admin request params input".to_string())?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| "admin request params input is not HtmlInputElement".to_string())?;
        request_params_input.set_id(ADMIN_REQUEST_PARAMS_ID);
        request_params_input.set_placeholder(
            "request params JSON object (for example: {\"thread_id\":\"thread_123\"})",
        );
        let _ = request_params_input.style().set_property("height", "34px");
        let _ = request_params_input
            .style()
            .set_property("padding", "0 10px");
        let _ = request_params_input
            .style()
            .set_property("border-radius", "8px");
        let _ = request_params_input
            .style()
            .set_property("border", "1px solid #1f2937");
        let _ = request_params_input
            .style()
            .set_property("background", "#0f172a");
        let _ = request_params_input
            .style()
            .set_property("color", "#cbd5e1");
        let _ = admin_panel.append_child(&request_params_input);

        let event_row = document
            .create_element("div")
            .map_err(|_| "failed to create admin event row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin event row is not HtmlElement".to_string())?;
        let _ = event_row.style().set_property("display", "grid");
        let _ = event_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = event_row.style().set_property("gap", "8px");

        for (id, placeholder) in [
            (ADMIN_EVENT_TYPE_ID, "event_type (worker.event)"),
            (ADMIN_EVENT_PAYLOAD_ID, "event payload JSON object"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create admin event input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "admin event input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = event_row.append_child(&input);
        }
        let event_send_button = document
            .create_element("button")
            .map_err(|_| "failed to create admin event send button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin event send button is not HtmlElement".to_string())?;
        event_send_button.set_id(ADMIN_EVENT_SEND_ID);
        event_send_button.set_inner_text("Send Event");
        let _ = event_send_button.style().set_property("height", "34px");
        let _ = event_send_button.style().set_property("padding", "0 10px");
        let _ = event_send_button
            .style()
            .set_property("border-radius", "8px");
        let _ = event_send_button
            .style()
            .set_property("border", "1px solid #334155");
        let _ = event_send_button
            .style()
            .set_property("background", "#334155");
        let _ = event_send_button.style().set_property("color", "#ffffff");
        let _ = event_row.append_child(&event_send_button);
        let _ = admin_panel.append_child(&event_row);

        let stream_row = document
            .create_element("div")
            .map_err(|_| "failed to create admin stream row".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin stream row is not HtmlElement".to_string())?;
        let _ = stream_row.style().set_property("display", "grid");
        let _ = stream_row.style().set_property(
            "grid-template-columns",
            "repeat(auto-fit, minmax(180px, 1fr))",
        );
        let _ = stream_row.style().set_property("gap", "8px");
        for (id, placeholder) in [
            (ADMIN_STREAM_CURSOR_ID, "cursor (default 0)"),
            (ADMIN_STREAM_TAIL_ID, "tail_ms (default 15000)"),
        ] {
            let input = document
                .create_element("input")
                .map_err(|_| "failed to create admin stream input".to_string())?
                .dyn_into::<HtmlInputElement>()
                .map_err(|_| "admin stream input is not HtmlInputElement".to_string())?;
            input.set_id(id);
            input.set_placeholder(placeholder);
            let _ = input.style().set_property("height", "34px");
            let _ = input.style().set_property("padding", "0 10px");
            let _ = input.style().set_property("border-radius", "8px");
            let _ = input.style().set_property("border", "1px solid #1f2937");
            let _ = input.style().set_property("background", "#0f172a");
            let _ = input.style().set_property("color", "#cbd5e1");
            let _ = stream_row.append_child(&input);
        }
        let stream_fetch_button = document
            .create_element("button")
            .map_err(|_| "failed to create admin stream fetch button".to_string())?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "admin stream fetch button is not HtmlElement".to_string())?;
        stream_fetch_button.set_id(ADMIN_STREAM_FETCH_ID);
        stream_fetch_button.set_inner_text("Fetch Stream");
        let _ = stream_fetch_button.style().set_property("height", "34px");
        let _ = stream_fetch_button
            .style()
            .set_property("padding", "0 10px");
        let _ = stream_fetch_button
            .style()
            .set_property("border-radius", "8px");
        let _ = stream_fetch_button
            .style()
            .set_property("border", "1px solid #334155");
        let _ = stream_fetch_button
            .style()
            .set_property("background", "#334155");
        let _ = stream_fetch_button.style().set_property("color", "#ffffff");
        let _ = stream_row.append_child(&stream_fetch_button);
        let _ = admin_panel.append_child(&stream_row);

        let _ = root.append_child(&admin_panel);
        body.append_child(&root)
            .map_err(|_| "failed to append codex chat root".to_string())?;
    }

    let send_button = document
        .get_element_by_id(CODEX_CHAT_SEND_ID)
        .ok_or_else(|| "missing codex send button".to_string())?;
    let input = document
        .get_element_by_id(CODEX_CHAT_INPUT_ID)
        .ok_or_else(|| "missing codex input".to_string())?;

    CODEX_SEND_CLICK_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_codex_message_from_input();
        }));
        let _ = send_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    CODEX_INPUT_KEYDOWN_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::wrap(Box::new(
            move |event: web_sys::KeyboardEvent| {
                if event.key() == "Enter" && !event.shift_key() {
                    event.prevent_default();
                    submit_codex_message_from_input();
                }
            },
        ));
        let _ =
            input.add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    GLOBAL_SHORTCUT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::wrap(Box::new(
            move |event: web_sys::KeyboardEvent| {
                handle_global_shortcut(event);
            },
        ));
        let _ =
            document.add_event_listener_with_callback("keydown", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    CODEX_QUICK_PROMPT_CLICK_HANDLERS.with(|slot| {
        let mut handlers = slot.borrow_mut();
        if !handlers.is_empty() {
            return;
        }
        for (index, prompt) in CHAT_QUICK_PROMPTS.iter().enumerate() {
            let Some(button) = document.get_element_by_id(CHAT_QUICK_PROMPT_IDS[index]) else {
                continue;
            };
            let prompt = prompt.to_string();
            let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
                start_codex_thread_with_prompt(prompt.clone());
            }));
            let _ =
                button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
            handlers.push(callback);
        }
    });

    let auth_send_button = document
        .get_element_by_id(AUTH_SEND_ID)
        .ok_or_else(|| "missing auth send button".to_string())?;
    let auth_verify_button = document
        .get_element_by_id(AUTH_VERIFY_ID)
        .ok_or_else(|| "missing auth verify button".to_string())?;
    let auth_restore_button = document
        .get_element_by_id(AUTH_RESTORE_ID)
        .ok_or_else(|| "missing auth restore button".to_string())?;
    let auth_logout_button = document
        .get_element_by_id(AUTH_LOGOUT_ID)
        .ok_or_else(|| "missing auth logout button".to_string())?;

    AUTH_SEND_CLICK_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_auth_send_from_input();
        }));
        let _ = auth_send_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    AUTH_VERIFY_CLICK_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_auth_verify_from_input();
        }));
        let _ = auth_verify_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    AUTH_RESTORE_CLICK_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            queue_intent(CommandIntent::RestoreSession);
        }));
        let _ = auth_restore_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    AUTH_LOGOUT_CLICK_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            queue_intent(CommandIntent::LogoutSession);
        }));
        let _ = auth_logout_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    let settings_profile_save_button = document
        .get_element_by_id(SETTINGS_PROFILE_SAVE_ID)
        .ok_or_else(|| "missing settings profile save button".to_string())?;
    let settings_profile_delete_button = document
        .get_element_by_id(SETTINGS_PROFILE_DELETE_ID)
        .ok_or_else(|| "missing settings profile delete button".to_string())?;
    let settings_autopilot_save_button = document
        .get_element_by_id(SETTINGS_AUTOPILOT_SAVE_ID)
        .ok_or_else(|| "missing settings autopilot save button".to_string())?;
    let settings_resend_connect_button = document
        .get_element_by_id(SETTINGS_RESEND_CONNECT_ID)
        .ok_or_else(|| "missing settings resend connect button".to_string())?;
    let settings_resend_disconnect_button = document
        .get_element_by_id(SETTINGS_RESEND_DISCONNECT_ID)
        .ok_or_else(|| "missing settings resend disconnect button".to_string())?;
    let settings_resend_test_button = document
        .get_element_by_id(SETTINGS_RESEND_TEST_ID)
        .ok_or_else(|| "missing settings resend test button".to_string())?;
    let settings_google_connect_button = document
        .get_element_by_id(SETTINGS_GOOGLE_CONNECT_ID)
        .ok_or_else(|| "missing settings google connect button".to_string())?;
    let settings_google_disconnect_button = document
        .get_element_by_id(SETTINGS_GOOGLE_DISCONNECT_ID)
        .ok_or_else(|| "missing settings google disconnect button".to_string())?;

    SETTINGS_PROFILE_SAVE_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_profile_update_from_inputs();
        }));
        let _ = settings_profile_save_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_PROFILE_DELETE_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_profile_delete();
        }));
        let _ = settings_profile_delete_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_AUTOPILOT_SAVE_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_autopilot_update_from_inputs();
        }));
        let _ = settings_autopilot_save_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_RESEND_CONNECT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_resend_connect_from_inputs();
        }));
        let _ = settings_resend_connect_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_RESEND_DISCONNECT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_resend_disconnect();
        }));
        let _ = settings_resend_disconnect_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_RESEND_TEST_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_resend_test();
        }));
        let _ = settings_resend_test_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_GOOGLE_CONNECT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            if let Some(window) = web_sys::window() {
                let _ = window
                    .location()
                    .set_href("/settings/integrations/google/redirect");
            }
        }));
        let _ = settings_google_connect_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    SETTINGS_GOOGLE_DISCONNECT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_settings_google_disconnect();
        }));
        let _ = settings_google_disconnect_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    let admin_create_button = document
        .get_element_by_id(ADMIN_CREATE_ID)
        .ok_or_else(|| "missing admin create button".to_string())?;
    let admin_refresh_button = document
        .get_element_by_id(ADMIN_REFRESH_ID)
        .ok_or_else(|| "missing admin refresh button".to_string())?;
    let admin_stop_button = document
        .get_element_by_id(ADMIN_STOP_ID)
        .ok_or_else(|| "missing admin stop button".to_string())?;
    let admin_request_button = document
        .get_element_by_id(ADMIN_REQUEST_SEND_ID)
        .ok_or_else(|| "missing admin request send button".to_string())?;
    let admin_event_button = document
        .get_element_by_id(ADMIN_EVENT_SEND_ID)
        .ok_or_else(|| "missing admin event send button".to_string())?;
    let admin_stream_button = document
        .get_element_by_id(ADMIN_STREAM_FETCH_ID)
        .ok_or_else(|| "missing admin stream fetch button".to_string())?;

    ADMIN_WORKER_CREATE_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_create_from_inputs();
        }));
        let _ = admin_create_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    ADMIN_WORKER_REFRESH_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_refresh_from_inputs();
        }));
        let _ = admin_refresh_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    ADMIN_WORKER_STOP_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_stop_from_inputs();
        }));
        let _ = admin_stop_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    ADMIN_WORKER_REQUEST_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_request_from_inputs();
        }));
        let _ = admin_request_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    ADMIN_WORKER_EVENT_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_event_from_inputs();
        }));
        let _ = admin_event_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    ADMIN_WORKER_STREAM_HANDLER.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let callback = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_event| {
            submit_admin_worker_stream_fetch_from_inputs();
        }));
        let _ = admin_stream_button
            .add_event_listener_with_callback("click", callback.as_ref().unchecked_ref());
        *slot.borrow_mut() = Some(callback);
    });

    Ok(())
}
