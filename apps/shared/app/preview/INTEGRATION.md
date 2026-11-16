# Integrating the preview loops

The loops name today's app in two places. When the app's root or its page
carrier changes, edit these and nothing else.

## `loop.coax`, the block under "the page"

| Name | Today | Change it when |
| --- | --- | --- |
| `LP_ROOT` | `apps/openagents-ios/app.coax` | the root `generate.sh` compiles moves |
| `LP_SHADER` | `components/backend/gpu-shader.coax` | the shader root moves |
| `LP_GENERATE` | `apps/openagents-ios/generate.sh` | generating moves |
| `LP_PAGE` | `-DCOMPONENTS_CHAT=1` | the page's macro in `host.c` changes |
| `LP_BUNDLE`, `LP_SCHEME`, `LP_PROJECT`, `LP_PRODUCT` | `com.openagents.app`, `OpenAgents` | the Xcode project changes |
| `LP_DEVICE` | `Coax Native Starter` | the simulator the iOS README names changes |

## `carrier.c`, the block under "the page"

The carrier calls the page through these macros, which map to the app's
`host.c` today:

| Macro | Today | What it must do |
| --- | --- | --- |
| `Page`, `PAGE_STATE` | `Chat`, `coax_t_ChatPage` | the page's carrier struct and its Coax state |
| `PAGE_FRAME`, `PAGE_POINTER`, `PAGE_DRIVE` | `oc_on_frame`, `oc_on_pointer`, `oc_on_drive` | the `UiApp` callbacks |
| `PAGE_ENV` | `oc_env` | the safe areas, keyboard height, focus and reduced motion |
| `PAGE_SET_TEXT`, `PAGE_ACTIVATE` | `oc_set_text`, `oc_activate` | the platform editor's text; a press on a semantic ID |
| `PAGE_MOVING`, `PAGE_REFUSED` | `oc_moving`, `oc_refused` | whether a frame is still animating; frames over the vertex capacity |

When the shared app (`apps/shared/app/`) becomes the root, its host gives the
same seven calls; point the macros at them. When the app needs native services
(HTTP, account, clock), the preview gives none: requests stay pending, which is
what a picture of a page waiting on the network shows. A case that needs an
answer uses the fake services and controlled clock from `apps/shared/app/cases/`
through `coax drive` operations in the case file.
