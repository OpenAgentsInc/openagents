import Api from './Api'
import GuestChatSessionController from './GuestChatSessionController'
import ChatApiController from './ChatApiController'
import ChatPageController from './ChatPageController'
import OpenApiSpecController from './OpenApiSpecController'
import FeedPageController from './FeedPageController'
import L402PageController from './L402PageController'
import Settings from './Settings'
import Auth from './Auth'

const Controllers = {
    Api: Object.assign(Api, Api),
    GuestChatSessionController: Object.assign(GuestChatSessionController, GuestChatSessionController),
    ChatApiController: Object.assign(ChatApiController, ChatApiController),
    ChatPageController: Object.assign(ChatPageController, ChatPageController),
    OpenApiSpecController: Object.assign(OpenApiSpecController, OpenApiSpecController),
    FeedPageController: Object.assign(FeedPageController, FeedPageController),
    L402PageController: Object.assign(L402PageController, L402PageController),
    Settings: Object.assign(Settings, Settings),
    Auth: Object.assign(Auth, Auth),
}

export default Controllers