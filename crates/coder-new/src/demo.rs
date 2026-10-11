//! Native presentation projection into the portable, effect-free demo renderer.
use coder_ui::demo as view;

pub(crate) fn draft(draft: &crate::Draft) -> view::Draft {
    view::Draft {
        text: draft.text.clone(),
        cursor: draft.cursor,
    }
}
pub(crate) fn options(value: &crate::models::GenerationOptions) -> view::models::GenerationOptions {
    view::models::GenerationOptions {
        reasoning: value.reasoning.clone(),
        max_tokens: value.max_tokens,
    }
}
pub(crate) fn focus(value: crate::plugins::SettingsFocus) -> view::plugins::SettingsFocus {
    use crate::plugins::SettingsFocus as F;
    use view::plugins::SettingsFocus as V;
    match value {
        F::ApiKey => V::ApiKey,
        F::Gateway => V::Gateway,
        F::Endpoint => V::Endpoint,
        F::Model => V::Model,
        F::TestKey => V::TestKey,
        F::Save => V::Save,
        F::RemoveKey => V::RemoveKey,
        F::Cancel => V::Cancel,
    }
}
pub(crate) fn connection(value: &crate::plugins::Connection) -> view::plugins::Connection {
    use crate::plugins::Connection as C;
    use view::plugins::Connection as V;
    match value {
        C::Unchecked => V::Unchecked,
        C::Checking => V::Checking,
        C::Verified => V::Verified,
        C::Failed(e) => V::Failed(e.clone()),
    }
}
pub(crate) fn cloud(
    value: &crate::cloud_settings::Configuration,
) -> view::cloud_settings::Configuration {
    view::cloud_settings::Configuration {
        enabled: value.enabled,
        mode: match value.mode {
            coder_cloud::Mode::Coder => view::cloud_settings::Mode::Coder,
            coder_cloud::Mode::Integrated => view::cloud_settings::Mode::Integrated,
        },
        size: value.size.clone(),
        template: value.template.clone(),
        credential_names: value.credential_names.clone(),
        workspace_paths: value.workspace_paths.clone(),
    }
}
pub(crate) fn editor(value: &crate::cloud_settings::Editor) -> view::cloud_settings::Editor {
    view::cloud_settings::Editor {
        placement: match value.placement {
            coder_cloud::Placement::Boat => view::cloud_settings::Placement::Boat,
            coder_cloud::Placement::Gce => view::cloud_settings::Placement::Gce,
        },
        config: cloud(&value.config),
        focus: value.focus,
        template: draft(&value.template),
        credentials: draft(&value.credentials),
        paths: draft(&value.paths),
        error: value.error.clone(),
    }
}
fn model(value: &crate::models::Model) -> view::models::Model {
    view::models::Model {
        plugin: value.plugin.clone(),
        provider: value.provider.clone(),
        id: value.id.clone(),
        name: value.name.clone(),
        description: value.description.clone(),
        context_length: value.context_length,
        max_output_tokens: value.max_output_tokens,
        efforts: value.efforts.clone(),
        default_effort: value.default_effort.clone(),
        supports_output_limit: value.supports_output_limit,
    }
}
fn picker(value: &crate::models::Picker) -> view::models::Picker {
    view::models::Picker {
        models: value.models.iter().map(model).collect(),
        query: draft(&value.query),
        selected: value.selected,
        stage: match value.stage {
            crate::models::Stage::Models => view::models::Stage::Models,
            crate::models::Stage::Reasoning => view::models::Stage::Reasoning,
            crate::models::Stage::Output => view::models::Stage::Output,
        },
        pending: value.pending.as_ref().map(model),
        options: options(&value.options),
        active_plugin: value.active_plugin.clone(),
        active_model: value.active_model.clone(),
        active_options: options(&value.active_options),
        loading: value.loading,
        refresh_requested: value.refresh_requested,
        error: value.error.clone(),
    }
}
pub(crate) fn screen(value: crate::Screen) -> view::Screen {
    match value {
        crate::Screen::Conversation => view::Screen::Conversation,
        crate::Screen::Plugins => view::Screen::Plugins,
        crate::Screen::PluginSettings => view::Screen::PluginSettings,
        crate::Screen::Appearance => view::Screen::Conversation,
    }
}
impl crate::App {
    /// Copy displayed demo values without carrying runtime, storage, or provider authority.
    pub fn demo_view(&self) -> view::DemoState {
        let mut value = view::DemoState::default();
        value.mode = if self.mode == crate::Mode::Demo {
            view::Mode::Demo
        } else {
            view::Mode::Live
        };
        value.screen = screen(self.screen);
        value.draft = draft(&self.draft);
        value.messages = self.messages.clone();
        value.scroll = self.scroll;
        value.selected_agent = self.selected_agent;
        value.animation_frame = self.animation_frame;
        value.elapsed_seconds = self.elapsed_seconds;
        value.plugins = self.plugins.demo_view();
        value.cwd = self.cwd.clone();
        value.branch = self.branch.clone();
        value.model_picker = self.model_picker.as_ref().map(picker);
        value.slash_selected = self.slash_selected;
        value.slash_hidden = self.slash_hidden;
        value.notice = self.notice.clone();
        value.other_draft = draft(&self.other_draft);
        value.return_screen = screen(self.return_screen);
        value.saved_chats = std::array::from_fn(|i| view::Chat {
            draft: draft(&self.saved_chats[i].draft),
            messages: self.saved_chats[i].messages.clone(),
            scroll: self.saved_chats[i].scroll,
        });
        value
    }
}
