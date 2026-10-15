# Enterprise configuration and third-party setup

Managed configuration arrives on the command line as
`--desktop-managed-config=<json>`. The main view publishes it as four
objects: `desktopManagedConfig`, `desktopEnterpriseConfig`,
`desktopTelemetryConfig`, and `desktopDeviceClass` (the JSON's
`deviceClass`). `desktopPreferredLanguages` is `preferredSystemLanguages`
from that same JSON. Buddy reads one field, `disableEssentialTelemetry`, and
publishes a boolean. The file that supplied the JSON was not opened. The
schema the app will accept is in the main chunk, as a table of setting
records.

A record carries a flat key, a title, a short description, a category, a
group, an `availableInVersion`, sometimes a `failClosedValue`, and sometimes
`appBehaviorOnly` or `remotePolicy`. Titles below are the record's
`defaultMessage`. Versions are the record's `availableInVersion` when one
was next to the key that was read. This is the schema compiled into
2.37337.1. It is not the policy of any one organization.

## Switches the boot path actually consults

| Key | What the desktop does with it |
| --- | --- |
| `disableDeepLinks` / `disableDeepLinkRegistration` | Deep links are dropped except magic-link and SSO. Fail closed, default false. Copy: "Disable claude:// deep-link handling". Version `1.6889.0`. |
| `disableEssentialTelemetry` | Buddy hides essential telemetry when this is exactly true. The main process also logs `disableNonessentialTelemetry is set` on a separate path. |
| `disableRemoteControl` | `SettingsResolver` logs that it is ignoring the value when the payload is not the shape it wants. An array of hosts is kept only when every entry is a non-empty string. An array with no host is `unreadable`. |
| `disableConnectorSignInInApp` | Copy: "Keep connector sign-in in the browser". Fail closed, `appBehaviorOnly`. Version `2.26454.0`. |
| `importThirdPartyHistoryIntoOrgUUID` | Copy: "Import earlier third-party sessions into this organization". The import service's `registerDesktop3pBundle` is the matching method. |

`allowedMcpServers`, `deniedMcpServers`, `strictKnownMarketplaces`,
`blockedMarketplaces`, `enabledPlugins`, `strictPluginOnlyCustomization`,
`deniedModels`, `availableModels`, `modelPricing`, `forceLoginOrgUUID`,
`gatewayInternalNetworks`, and `wslInheritsWindowsSettings` are keys in the
same policy objects. They are names. The branch that enforces each one was
not walked. `sandbox.credentials`, `sandbox.credentials.sigv4`,
`sandbox.credentials.allowPlaintextInject`, and `sandbox.credentials.awsPairs`
are key paths in that schema family. No value was present, and no function
that writes a credential into a request was read. `e2ePlaintextTokens` is a
name in the same neighborhood. It is not a token.

## Organization appearance

`deploymentDisplayName` and `deploymentDisplaySubtitle` are trimmed strings,
length 1 to 60, hot-applied, and the subtitle is shown only when the name is
non-empty. An organization banner has `enabled`, `text` (1 to 200
characters), `backgroundColor` and `textColor` as `#RRGGBB`, and an optional
`linkUrl` that must parse as a URL and must not be one of the schemes the
`STe` check rejects. A banner that fails those predicates does not render.
The record's title is "Organization banner". "Show banner" is the enabled
field's label.

`disableConfigDeprecationWarnings` hides deprecation warnings. Copy: "Hide
configuration deprecation warnings".

## Third-party and hybrid setup

`Custom3pSetup` is the deployment window's service, 37 methods. Config
lifecycle: `listConfigs`, `readConfig`, `createConfig`, `writeConfig`,
`deleteConfig`, `duplicateConfig`, `renameConfig`, `exportConfig`,
`revealConfig`, `getMergedConfig`, `setAppliedConfig`, `getConfigHealth`,
`recheckConfigHealth`. Discovery: `retryModelDiscovery`, `scanOrgPlugins`,
`listSwitchableProfiles`, `switchProfile`, `setHybridOrgList`,
`switchOrgAndRelaunch`. Mode: `setDeploymentMode`. Auth:
`signInWithAnthropicApi`, `applyAnthropicApiShortcut`,
`triggerBootstrapAuth`, `getLoginDesktop3pStatus`, `forgetMcpOAuth`,
`probeMcpServer`, `authorizeAndProbeMcpServer`, `probeEgressHosts`.
Relaunch: `relaunchApp`, `relaunchAppWithSessionGuard`. The hybrid
transition is `commitHybridTransition`, `cancelHybridTransition`,
`resolverLandedTainted`, with events `bootstrapState_` and
`hybridTransition_`. `openSetupWindow` and `openDeviceCodeWindowForE2e` open
the windows. `Custom3pHelperRun`, which is refused outside the setup window,
is `discoverModels`, `probeDeviceCode`, `probeInference`,
`runCredentialHelper`.

Device-code sign-in does not save on browser approval alone. The main
process logs `device-code sign-in approved in the browser; awaiting account
confirmation`. The dialog copy is "Continue as {account}?" and "Did your
browser just confirm that you approved this sign-in?". A second string covers
the case where the sign-in returned no address: continue only if you typed
the code yourself. Dismissing the dialog logs `account prompt declined;
nothing saved`. `AbortError` is the cancellation. Nothing is written before
that confirmation returns.

Requests the setup proxy forwards to the Anthropic host, and which the host
refuses or does not answer, are turned into HTTP 503. A stub that fails is
also a 503. The log prefix is `[custom-3p]`. Neighbor tags: `custom-3p:sso`,
`custom-3p:entra`, `custom-3p:idp`, `custom-3p:health`,
`custom-3p:org-plugins`, `custom-3p:configured-marketplaces`,
`custom-3p:sharing-endpoint`, `custom-3p:wif`, `custom-3p:console`,
`egress-probe`, `egress-proxy`. `Custom3pUsage` is the world member for usage
of that deployment. Its method list was not extracted.

An API the desktop calls for org records is built as
`/v1/desktop/orgs/<org>/<suffix>`, with `Authorization: Bearer <token>` or
the token placed in the header the request calls `xAuthorization`.
`bootstrapUrl` is accepted only when it parses as one of the URLs the `lze`
check allows. The check's allow list was not extracted.

Self-hosted startup has its own failure counter. A start-up step that fails
increments it. The log is `[self-hosted] a start-up step of the failure
counts failed`. Categories beside that counter: `sign_in_failure`,
`tool_connection_failure`, `update_failure`, `install_failed`,
`config-error`.

## Microsoft 365 labels

The Office add-in can be turned off per file by a sensitivity-label
statement. A statement has an effect, `allow` or `deny`, and deny wins when
both match. The copy in the schema says deny turns the named actions off for
matching files, and allow turns them off for every other file of that type.

Actions the statement can name: `slide`, `doc`, `mail`, `skills.authoring`,
`thumbs`, `addin.access`, `file.upload`, `web_search`. The labels shown to an
administrator are Skill authoring, Response feedback, "The add-in itself",
File upload, and Web search. "The add-in itself" covers the whole add-in.

A label identifier is either a GUID (`mip_label_guid`) or a name
(`mip_label_name`). Name matching is `equals`, `startsWith`, `endsWith`, or
`exists`. The copy says GUIDs keep matching after a rename, and names must
match case. `exists` with "any label" matches every labeled file. "No label"
matches unlabeled files and, for uploads, files that cannot have a label,
such as images. Scope is `open_file` (the document the add-in is running in)
or `uploaded_file` (each attachment). A description field is stored and "has
no effect".

Entra records on the same schema: `clientId` (a GUID, redacted),
`azureCloud`, and `scope` split on spaces and commas. The scope field warns
when it is set and `entra.clientId` is empty. `readers` is a list on the
client id. `remotePolicy` for these is `requiresUserConsent`.

`OfficeAddinFiles` is how a live document shows up in the desktop:
`getConnectedFiles`, `focusFile`, `selectFile`, `isFeatureEnabled`, the
connected-files state event, and the Porter handoff that moves a file across.
The label statement is the policy that can make `isFeatureEnabled` false for
one of those actions. The comparison function was not stepped through with a
sample label.

## What this chapter does not contain

No organization config, no tenant id, and no token was read from this
machine. `~/Library/Application Support/Claude` was not opened. The schema
above is what the binary will parse. A key that is absent, or that fails the
parser, stays at the record's `failClosedValue` when the record defines one.
`disableDeepLinks` and `disableConnectorSignInInApp` both fail closed to
true, which means a broken managed config disables the feature rather than
leaving it on.
