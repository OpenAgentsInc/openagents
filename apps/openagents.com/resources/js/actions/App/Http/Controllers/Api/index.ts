import ShoutsController from './ShoutsController'
import AuthRegisterController from './AuthRegisterController'
import Webhooks from './Webhooks'
import Internal from './Internal'
import MeController from './MeController'
import TokenController from './TokenController'
import SpacetimeTokenController from './SpacetimeTokenController'
import ChatController from './ChatController'
import RuntimeToolsController from './RuntimeToolsController'
import RuntimeSkillRegistryController from './RuntimeSkillRegistryController'
import RuntimeCodexWorkersController from './RuntimeCodexWorkersController'
import AutopilotController from './AutopilotController'
import AutopilotStreamController from './AutopilotStreamController'
import ProfileController from './ProfileController'
import WhispersController from './WhispersController'
import AgentPaymentsController from './AgentPaymentsController'
import L402Controller from './L402Controller'
import L402PaywallController from './L402PaywallController'

const Api = {
    ShoutsController: Object.assign(ShoutsController, ShoutsController),
    AuthRegisterController: Object.assign(AuthRegisterController, AuthRegisterController),
    Webhooks: Object.assign(Webhooks, Webhooks),
    Internal: Object.assign(Internal, Internal),
    MeController: Object.assign(MeController, MeController),
    TokenController: Object.assign(TokenController, TokenController),
    SpacetimeTokenController: Object.assign(SpacetimeTokenController, SpacetimeTokenController),
    ChatController: Object.assign(ChatController, ChatController),
    RuntimeToolsController: Object.assign(RuntimeToolsController, RuntimeToolsController),
    RuntimeSkillRegistryController: Object.assign(RuntimeSkillRegistryController, RuntimeSkillRegistryController),
    RuntimeCodexWorkersController: Object.assign(RuntimeCodexWorkersController, RuntimeCodexWorkersController),
    AutopilotController: Object.assign(AutopilotController, AutopilotController),
    AutopilotStreamController: Object.assign(AutopilotStreamController, AutopilotStreamController),
    ProfileController: Object.assign(ProfileController, ProfileController),
    WhispersController: Object.assign(WhispersController, WhispersController),
    AgentPaymentsController: Object.assign(AgentPaymentsController, AgentPaymentsController),
    L402Controller: Object.assign(L402Controller, L402Controller),
    L402PaywallController: Object.assign(L402PaywallController, L402PaywallController),
}

export default Api
