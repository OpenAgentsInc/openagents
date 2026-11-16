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
objects, duplicate/count refusal, state/child notifications and retirement
(including closing inside an activation). Both ordinary and ASan/UBSan/LSan
runs pass on a Linux build host.

```sh
OA_REMOTE=1 coax/remote/remote.sh run -- nix-shell -p gcc pkg-config at-spi2-core \
  --run 'coax/web/tool.sh apps/shared/host/linux/tests/access.coax'
```

`access-bus.c` registers the application through the existing AT-SPI bridge.
A separate actual AT-SPI client discovers its application and window, reads
the redacted field and activates a row over D-Bus. It is an isolated session
with no compositor, account credentials, network login or sound. State, text
and structural events follow the committed generation; removed SDK objects
are retained as defunct if a client still holds them.

```sh
OA_REMOTE=1 coax/remote/remote.sh run -- nix-shell -p gcc pkg-config at-spi2-core dbus \
  --run 'export XDG_DATA_DIRS="$(pkg-config --variable=prefix atspi-2)/share"; coax/web/tool.sh apps/shared/host/linux/tests/access.coax bus --bus-config "$(pkg-config --variable=prefix dbus-1)/share/dbus-1/session.conf"'
```

The shared Vulkan host's `UI_ATSPI` path synchronizes after a frame, services
GLib messages on the UI thread and retires accessibility before freeing the
surface. It compiles against the actual generated Wayland input/text headers
and Vulkan/ATK SDKs (`tests/access.coax host`). Consumers enabling it link
`access.c`, `access-bus.c` and `atk`/`atk-bridge-2.0`.

This is the SDK foundation for #13645, not a shipped screen-reader exit.
Active-editor text/selection, focus traversal, the compiled OpenAgents Linux
consumer and an Orca ceremony remain. Coordinates currently describe the window; a Wayland client
cannot infer a global desktop origin from these rows. The GTK native client
is retained until those gates pass.

The SDK contract follows GNOME's [AtkObject](https://docs.gtk.org/atk/class.Object.html),
[AtkComponent](https://docs.gtk.org/atk/iface.Component.html) and
[AtkAction](https://docs.gtk.org/atk/iface.Action.html) interfaces. No upstream
implementation is copied into the application.

The older generic gallery cannot currently generate its C: two hit-count
assertions in `components/gallery/page.coax` fail unchanged on earlier commit
`5ba21554b7` as well as this tree. They were retained. This does not prevent
the direct ATK, separate-process AT-SPI or real Vulkan-host SDK compiler gates.
