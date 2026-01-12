use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use tokio::sync::mpsc;
use web_time::Instant;
use wgpui::components::hud::CommandPalette;
use wgpui::{TextInput, TextSystem};
use winit::keyboard::ModifiersState;
use winit::window::Window;

use crate::app::agents::AgentBackendsState;
use crate::app::agents::AgentRegistry;
use crate::app::autopilot::AutopilotState;
use crate::app::autopilot_issues::AutopilotIssuesState;
use crate::app::catalog::CatalogState;
use crate::app::chat::ChatState;
use crate::app::config::AgentSelection;
use crate::app::config::SettingsState;
use crate::app::dspy::DspyState;
use crate::app::dvm::DvmState;
use crate::app::events::ModalState;
use crate::app::gateway::GatewayState;
use crate::app::git::GitState;
use crate::app::chainviz::ChainVizState;
use crate::app::lm_router::LmRouterState;
use crate::app::nexus::NexusState;
use crate::app::nip28::Nip28State;
use crate::app::nip90::Nip90State;
use crate::app::permissions::PermissionState;
use crate::app::pylon_earnings::PylonEarningsState;
use crate::app::pylon_jobs::PylonJobsState;
use crate::app::rlm::{RlmState, RlmTraceState};
use crate::app::session::SessionState;
use crate::app::spark_wallet::SparkWalletState;
use crate::app::tools::ToolsState;
use crate::app::ui::{ThemeSetting, resolve_theme};
use crate::app::wallet::WalletState;
use crate::app::workspaces::WorkspaceState;
use crate::panels::PanelLayout;
use wgpui::components::EventContext;
use wgpui::renderer::Renderer;

pub(crate) struct AppState {
    pub(crate) window: Arc<Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) renderer: Renderer,
    pub(crate) text_system: TextSystem,
    pub(crate) event_context: EventContext,
    #[allow(dead_code)]
    pub(crate) clipboard: Rc<RefCell<Option<arboard::Clipboard>>>,
    pub(crate) command_palette: CommandPalette,
    pub(crate) command_palette_action_rx: Option<mpsc::UnboundedReceiver<String>>,
    pub(crate) input: TextInput,
    pub(crate) mouse_pos: (f32, f32),
    pub(crate) modifiers: ModifiersState,
    #[allow(dead_code)]
    pub(crate) last_tick: Instant,
    pub(crate) modal_state: ModalState,
    #[allow(dead_code)]
    pub(crate) panel_layout: PanelLayout,
    pub(crate) left_sidebar_open: bool,
    pub(crate) right_sidebar_open: bool,
    pub(crate) new_session_button_hovered: bool,
    pub(crate) chat: ChatState,
    pub(crate) tools: ToolsState,
    pub(crate) git: GitState,
    pub(crate) session: SessionState,
    pub(crate) catalogs: CatalogState,
    pub(crate) agent_backends: AgentBackendsState,
    pub(crate) workspaces: WorkspaceState,
    pub(crate) settings: SettingsState,
    pub(crate) permissions: PermissionState,
    pub(crate) autopilot: AutopilotState,
    pub(crate) autopilot_issues: AutopilotIssuesState,
    pub(crate) rlm: RlmState,
    pub(crate) rlm_trace: RlmTraceState,
    pub(crate) pylon_earnings: PylonEarningsState,
    pub(crate) pylon_jobs: PylonJobsState,
    pub(crate) wallet: WalletState,
    pub(crate) dspy: DspyState,
    pub(crate) dvm: DvmState,
    pub(crate) gateway: GatewayState,
    pub(crate) lm_router: LmRouterState,
    pub(crate) chainviz: ChainVizState,
    pub(crate) nexus: NexusState,
    pub(crate) spark_wallet: SparkWalletState,
    pub(crate) nip28: Nip28State,
    pub(crate) nip90: Nip90State,
    pub(crate) system_theme: Option<ThemeSetting>,
    pub(crate) show_kitchen_sink: bool,
    pub(crate) kitchen_sink_scroll: f32,
    pub(crate) help_scroll_offset: f32,
    /// Selected agent backend (Codex)
    pub(crate) agent_selection: AgentSelection,
    /// Agent availability registry
    #[allow(dead_code)]
    pub(crate) agent_registry: AgentRegistry,
}

impl AppState {
    pub(crate) fn resolved_theme(&self) -> ThemeSetting {
        resolve_theme(self.settings.coder_settings.theme, self.system_theme)
    }
}
