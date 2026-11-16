# GPU accessibility on Linux

`access.c` projects the host's committed `UiElement` source into retained ATK
objects. It creates no GTK controls and owns no application reducer, layout,
text parser or second semantic tree. Coax supplies the same visible bounds,
roles, public text, states and stable IDs used for GPU hit testing.

Rows are preflighted before SDK objects change: bounded count, unique IDs,
UTF-8, finite geometry, known roles and backward parent references. Refusal
keeps the previous generation. Existing IDs retain their ATK objects;
removed objects become defunct and cannot dispatch a stale action. Activation
calls the committed-row adapter and invalidates the display, unless activation
itself retired the host. Secure fields expose only their public semantic
value, which the Coax identity screen leaves empty.

The SDK tests use actual ATK queries for roles, checked/disabled states,
Unicode text, redacted private values, geometry, direct activation, stable
objects, duplicate/count refusal and retirement (including closing inside an
activation). Both ordinary and ASan/UBSan/LSan runs pass on a Linux build host.

```sh
OA_REMOTE=1 coax/remote/remote.sh run -- nix-shell -p gcc pkg-config at-spi2-core \
  --run 'coax/web/tool.sh apps/shared/host/linux/tests/access.coax'
```

This is the SDK foundation for #13645, not a shipped screen-reader exit.
AT-SPI application registration and event-loop integration, active-editor
text/selection, focus traversal, actual Vulkan app integration and an Orca
ceremony remain. Coordinates currently describe the window; a Wayland client
cannot infer a global desktop origin from these rows. The GTK native client
is retained until those gates pass.

The SDK contract follows GNOME's [AtkObject](https://docs.gtk.org/atk/class.Object.html),
[AtkComponent](https://docs.gtk.org/atk/iface.Component.html) and
[AtkAction](https://docs.gtk.org/atk/iface.Action.html) interfaces. No upstream
implementation is copied into the application.
