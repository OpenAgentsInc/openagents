# The Cowork virtual machine

Cowork tasks that need Linux run a guest. The desktop downloads a bundle,
checks it against a content hash, copies a virtual disk into it, and asks the
Swift addon to start the machine. The guest network stack embedded in that
addon is the Go `usernet` archive described in [native modules](native.md).
This chapter is the controller in the main process. The machine was not
started.

## What "ready" means

The bundle file is `claudevm.bundle`. Its identity is `RU.sha`, a content
hash the controller compares, not the desktop app's git commit. Status logs:

- `origin missing`
- `version mismatch: have=<hash>, want=<RU.sha>`
- a read error of the origin
- a stale cache dropped when the cached hash does not match, logged by
  `downloadVM` with the first 12 hex characters

The download URL template is:

```text
https://downloads.claude.ai/vms/linux/${arch}/${RU.sha}
```

`arch` comes from `RKr()`. The platform segment in the URL is `linux` even
on macOS, because the guest is Linux. A sweep (`Bundle:sweep`) removes
leftovers and stale partials, logs reclaimed gigabytes, and emits
`lam_vm_bundle_sweep`. It skips itself when a download or a start is in
flight, and a flag `disableStartupBundleSweep` turns the startup sweep off.
Partial files use a `.partial` suffix. In-progress files use `.wvm-tmp-`.
An adopted tree uses `.cowork-adopted`.

Before a download, free space is compared with the requirement. The warning
names both sizes in gigabytes and, when deleting the old image would free
enough, says so. The old image is deleted to make room unless a running VM
holds it (`locked by running VM, keeping stale bundle`). A warm bundle can be
promoted into place. If promotion throws, the controller falls back to the
cache or a download. The cache is decompressed (`lam_vm_cache_decompress_completed`
or `_failed`; `ZSTD_error_memory_allocation` is a string in the same
function). A later update can be a binary delta (`Updated … via binary
delta`). Success emits `lam_vm_bundle_download_completed` with byte count,
reason, and `resumed_from_byte`.

`deleteVMBundle` removes reinstall files. The log says sessiondata and the
compressed cache are preserved. A locked bundle is skipped. The reason string
for a lock includes the instance id.

## Start

`ClaudeVM.startVM` on the bridge enters the controller. The steps that have
their own names:

| Step | Failure the controller records |
| --- | --- |
| Support check | `VM not supported (<platform>/<arch>, <reason>)`, state set to offline. |
| Virtualization missing | `Virtualization not available`, state offline. |
| `download_and_sdk_prepare` | `SDK preparation failed`. |
| `load_swift_api` | `Swift VM addon not available` / `swift_addon_unavailable`. |
| Disk copy | `smol-bin.<arch>.vhdx not found`, or locked by the running VM. |
| `configure` then `startVM` | `VM boot failed: <message>`. A native connect error is rethrown with `isNativeConnectError`. |
| Guest connection | Poll until `connectDeadline`. Timeout names the last completed step. |

The disk step copies `smol-bin.<arch>.vhdx` from `process.resourcesPath` into
the bundle. On Windows the same shutdown path also copies that file, with a
short wait, and tolerates `EBUSY`, `EACCES`, and `EPERM`. The macOS resources
directory contains `smol-bin.arm64.img` and `smol-bin.x64.img`, about 23 MB
and 24 MB, each starting with an exFAT boot sector. The controller's copy
source is the `.vhdx` name. Both can be true: the images that ship beside the
app are the exFAT files, and the runtime disk the guest boots is a vhdx the
controller expects to find by the other name. This pass did not find a
`.vhdx` in the inspected bundle listing, and it did not mount the exFAT
images. `efivars.fd` is a filename in the same startup block. Whether it is
written into the bundle or only expected there was not separated from the
filename.

`startVM` on the Swift side is called with the instance, a memory size in
gigabytes (`[VM:start] memory=<n>GB`), the reachability probe target, an
IPv6 value derived from the host configuration, and a flag that is true when
that IPv6 value is present. The default probe target in the log format is
`api.anthropic.com`. `G().vmApiReachabilityProbeTarget()` can replace it.

After the native call returns, the state is `Booting`. The controller polls.
Each iteration checks a guest error channel and the abort signal. A guest
error becomes `VM stopped unexpectedly: …`. Abort becomes `VM start aborted`.
Otherwise it runs the connect step with `{vmIpv6}`. Progress logs
`Still waiting for guest connection… <ms> elapsed, <n> polls`. Success logs
`Startup complete, total time: <ms>` and emits `lam_vm_startup_completed`
with instance id, bundle hash, duration, whether the download was fresh,
retry reason, `vm_ipv6`, and `vm_memory_gb`.

Timeout builds `VM connection timeout after <seconds>`, may append VPN
wording when `vpnActive` is set, and throws a `VMConnectionTimeoutError` that
carries `guestReady`. The last completed step defaults to the name `vm_boot`.
Other failure tags: `lam_vm_startup_failed`, `vm_pre_boot_error`,
`host_share_not_mounted`, `VM_LOGON_RIGHT_DENIED`, `VM_DISK_SPACE_INSUFFICIENT`.
`ENOSPC` during start logs `disk full` and leaves the VM offline. An
installation the native layer rejects uses the string `Claude's installation
appears to be invalid or has been modified. Reinstall Claude from
claude.ai/download to use this feature.`

If the guest is already connected, start takes a fast path
(`VM already connected`) and can still fail in post-connect setup. Auto
reinstall runs at most once (`lam_vm_auto_reinstall_triggered`). It is
skipped when the host drive share is the thing that failed, when a marker
cannot be written, or when it already ran. The VM stays offline and the next
launch retries. `restartVM` joins a restart already in progress rather than
starting a second one. It stops, records `trigger`, and starts again.

`CLAUDE_E2E_VM_SMOKE_COMMAND` is read from the environment at the end of a
successful start and, when set, runs a named process. That is a test hook in
the production script.

## Network and the host

The Swift addon links Virtualization and `vmnet`, and it embeds the Go module
`usernet` built on `gvisor-tap-vsock`. The controller's own words for that
stack: log file `vzgvisor.log`, a probe step whose label is `gvisor`, and a
path class `home_mnt_alias` that rewrites a guest `~/mnt/` onto `/mnt`.
Windows guest paths in the same classifier include `/appdata/local/temp`.
The host log directory on Windows is `C:\ProgramData\Claude\Logs`, with
`cowork-service.log`, `coworkd.log`, and `cowork_vm_swift.log`. On the
machine this build was read from, those are names in the script. The macOS
log files next to them in the same string table are `swift.log`,
`chrome-native-host.log`, `app-cu-helper.log`, and `coworkd-user.log`.

Reachability is polled. `networkStatus` values include `CONNECTED` and
`NOT_CONNECTED`. Losing the network starts a 30 second timer (`NKr = 30000`)
and, if it is still down, sets `network_connection_failed`. The log is
`API reachability: <state> (probe target: …)`. Local names the same function
treats as non-public: `localhost`, `127.0.0.1`, `::1`, `[::1]`, `.internal`,
`.local`, `.lan`, `.invalid`, `.localhost`, plus the labels `unconfigured`,
`private`, and `other`.

Host loop is the mode in which the desktop talks to a Linux agent without
making the guest the only place code runs. `ClaudeVM` exposes
`isHostLoopModeEnabled`, `isHostLoopDevOverrideActive`,
`setForceDisableHostLoop`, and `hostIPv6Notice_`. The IPv6 notice is also a
field the start path already passes downward (`hostIPv6Notice` in the
controller). `setYukonSilverConfig` writes a store named `yukon-silver-config`.
Values observed next to that store name are `auto`, `bypassPermissions`, and
`skip_all_permission_checks`. The scheduled-task dispatcher reads
`yukonSilver.status` and will not dispatch a local task when the status is
not the ready value it checks. So Yukon Silver is the host-loop configuration
record, and `bypassPermissions` is one of its stored modes. What the guest
does with that mode was not executed. It is a dangerous-looking name stored
as a config value. This pass did not find a second implementation that turns
it into a blanket permission grant, and it did not find a token.

A proxy log (`VM:proxy`, `[proxy] <<< UPSTREAM ERROR`) sits on the same
network path. `PartitionNetwork` is the log tag for the Electron session
side of a partition's traffic. `VMDiskJanitor` is the log tag for the
leftover sweep. `ClaudeCodeManager-VM` and `VMCLIRunner` are the tags for
running a Claude Code process against the guest. `Spawn:vm` is the tag for
the spawn itself. `CLAUDE_CODE_SDK_HAS_HOST_AUTH_REFRESH` and
`CLAUDE_CODE_HOST_CREDS_FILE` are environment names read when that runner
prepares the guest-side CLI. The credential file was not opened.

## Shutdown and the bridge

On app quit the controller logs `App quit, stopping VM (instance: …)`, calls
`stopVM(true)`, and logs the duration. Telemetry is `lam_vm_shutdown_completed`
or `lam_vm_shutdown_failed`. The Windows vhdx copy described above happens
after that stop.

The renderer service `ClaudeVM` is the only bridge into this controller:

`download`, `startVM`, `restartVM`, `getDownloadStatus`, `getRunningStatus`,
`checkVirtualMachinePlatform`, `enableVirtualMachinePlatform`,
`restartAfterVMPInstall`, `setForceDisableHostLoop`, `setYukonSilverConfig`,
`isHostLoopModeEnabled`, `isHostLoopDevOverrideActive`, and the events
`downloadProgress`, `downloadStatusChanged`, `runningStatusChanged`,
`startupError`, `apiReachability_`, `hostIPv6Notice_`.

`enableVirtualMachinePlatform` and `restartAfterVMPInstall` are the Windows
"Virtual Machine Platform" feature path, compiled into this macOS build the
same way the Squirrel flags are. `getRunningStatus` is what
`CoworkFilePreview.isVmReady` has to agree with before a preview that needs
the guest will show.
