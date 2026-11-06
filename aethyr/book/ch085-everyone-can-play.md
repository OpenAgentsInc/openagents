# 85. Everyone can play

> **Status: partly implemented.** The shared settings record and Everglade's
> SETTINGS pane implement key and controller remapping, text size, reduced
> motion and captions. The accessible control mirror works in the browser.
> Other-realm adoption and translation remain work.

[`aethyr/ui/settings.coax`](../ui/settings.coax) owns the versioned record,
its defaults and its codec. Each action has one key and one pad button.
Binding a code already in use swaps the two bindings, leaving every action
bound. Text size ranges from 100 to 200 percent in steps of 25. A malformed
or unsupported saved record returns the defaults whole, with a status.

## Everglade's SETTINGS pane

[`everglade-access.coax`](../../apps/openagents-web/everglade-access.coax)
uses that record for gameplay input and the pane above SOUND. The browser
persists it as `verse.settings`. Text size scales the HUD within the
screen's layout limit, which the pane names. Reduced motion stops camera
shake. Captions identify sounds that matter and their direction.

The page exposes controls as ordered, named records. The browser host
mirrors them into buttons and checkboxes, preserves keyboard focus and
reports status through a live region. MAP, SETTINGS, SOUND, INVITE,
CHARACTER, PARTY and the hotbar use the same actions as their canvas
controls. The made-realm editor has its own
[accessible layer](../../apps/openagents-web/myrealm-a11y.coax).

## Sessions and remaining scope

Accessibility settings do not grant authority or change the room's game
clock. [Chapter 95](ch095-offline-is-a-local-server.md) keeps offline and
room gameplay on one simulation; [chapter 98](ch098-versions-deploys-and-reconnect.md)
keeps reconnect distinct from held device input. The shared lifecycle
releases held input on blur and revokes old commands after a long suspension.

Everglade is English only. Its pane has stable text IDs and `?pseudo=1`
lengthens labels for layout checks. Pseudolocalization does not qualify
translation, script coverage, bidirectional layout or IME composition.
Applying remapping, captions and scaled layouts to every realm also remains
work; this chapter does not claim universal accessibility from one adoption.

## Run the checks

```shell
coax/bin/coax test aethyr/ui/settings.coax --library
coax/web/tool.sh coax/web/forge-access.coax
```

The second command exercises the made-realm browser layer and requires its
local page setup; see the tool's usage. Browser checks run headless and
muted. Keyboard navigation, focus, captions and actual layout need their
own observations in addition to the settings codec's unit checks.
