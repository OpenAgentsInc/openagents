import { describe, expect, test } from "vite-plus/test";

import {
  acpProviderSettingsView,
  availableAcpProviderActions,
  buildAcpSupportBundle,
  decodeAcpProviderActionPayload,
  decodeAcpProviderSettings,
  type AcpProviderProjection,
} from "./acp-provider-settings.ts";

type AnyNode = Readonly<Record<string, unknown>>;
const nodes = (root: unknown): AnyNode[] => {
  const result: AnyNode[] = [];
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) return void value.forEach(visit);
    if (value === null || typeof value !== "object") return;
    const record = value as AnyNode;
    if (typeof record._tag === "string") result.push(record);
    Object.entries(record).forEach(([key, child]) => {
      if (!["style", "a11y", "_tag"].includes(key)) visit(child);
    });
  };
  visit(root);
  return result;
};
const byKey = (root: unknown, key: string): AnyNode | undefined =>
  nodes(root).find((node) => node.key === key);

const fixture = (
  provider: "grok" | "cursor",
  patch: Partial<AcpProviderProjection> = {},
): AcpProviderProjection => ({
  provider,
  displayName: provider === "grok" ? "Grok CLI" : "Cursor Agent CLI",
  profileRef: provider === "grok" ? "grok-cli" : "cursor-agent",
  protocol: "Agent Client Protocol",
  install: "detected",
  executable: {
    source: "trusted-path",
    displayPath: provider === "grok" ? "/opt/xai/grok" : "/opt/cursor/cursor-agent",
  },
  version: provider === "grok" ? "0.2.101" : "2026.6.24",
  profileState: provider === "grok" ? "supported" : "experimental",
  auth: {
    state: "required",
    advertisedMethods: provider === "grok" ? ["cached_token", "xai.api_key"] : ["cursor_login"],
    safeLogout: false,
  },
  probe: { state: "passed", code: null, observedAt: "2026-07-16T12:00:00.000Z" },
  session: {
    state: "none",
    sessionRef: null,
    processRef: null,
    canNew: true,
    canList: provider === "grok",
    canLoad: provider === "grok",
    canResume: false,
    canCancel: false,
  },
  capabilities: { filesystem: false, terminal: false, permissions: true, questions: true },
  configuration: [],
  diagnosticCodes: [],
  conformanceRef:
    provider === "grok" ? "acp.grok.0.2.101.darwin-arm64" : "acp.cursor.2026.6.24.darwin-arm64",
  ...patch,
});

const response = (grok = fixture("grok"), cursor = fixture("cursor")): unknown => ({
  state: "ok",
  providers: [cursor, grok],
});

describe("ACP provider settings projection", () => {
  test("decodes exactly the distinct Grok and Cursor peer profiles in deterministic order", () => {
    const decoded = decodeAcpProviderSettings(response());
    expect(decoded.state).toBe("loaded");
    if (decoded.state !== "loaded") return;
    expect(
      decoded.providers.map((provider) => [
        provider.provider,
        provider.profileRef,
        provider.protocol,
      ]),
    ).toEqual([
      ["grok", "grok-cli", "Agent Client Protocol"],
      ["cursor", "cursor-agent", "Agent Client Protocol"],
    ]);
    expect(JSON.stringify(decoded)).not.toContain("Agent Communication Protocol");
    expect(JSON.stringify(decoded)).not.toContain("A2A");
  });

  test("fails closed on incomplete, swapped, duplicate, or malformed provider identity", () => {
    expect(decodeAcpProviderSettings({ state: "ok", providers: [fixture("grok")] }).state).toBe(
      "unavailable",
    );
    expect(
      decodeAcpProviderSettings({ state: "ok", providers: [fixture("grok"), fixture("grok")] })
        .state,
    ).toBe("unavailable");
    expect(
      decodeAcpProviderSettings(
        response(fixture("grok", { displayName: "Cursor Agent CLI" })) as never,
      ).state,
    ).toBe("unavailable");
    expect(
      decodeAcpProviderSettings({ state: "ok", providers: [{ provider: "all-acp-agents" }] }).state,
    ).toBe("unavailable");
  });

  test("derives authentication actions only from advertised runtime state", () => {
    const required = fixture("grok");
    expect(availableAcpProviderActions(required)).toEqual([
      "probe",
      "select_alternate",
      "authenticate",
    ]);
    const authenticated = fixture("grok", {
      auth: { ...required.auth, state: "authenticated", safeLogout: true },
    });
    expect(availableAcpProviderActions(authenticated)).toEqual(["logout", "new_session"]);
    const expired = fixture("cursor", { auth: { ...fixture("cursor").auth, state: "expired" } });
    expect(availableAcpProviderActions(expired)).toEqual([
      "probe",
      "select_alternate",
      "reauthenticate",
    ]);
    expect(availableAcpProviderActions(fixture("grok", { install: "not_installed" }))).toEqual([
      "probe",
      "select_alternate",
    ]);
  });

  test("represents Cursor login cancel/deny/expiry and Grok cached/API-key methods without secrets", () => {
    for (const state of ["pending", "cancelled", "denied", "expired"] as const) {
      const cursor = fixture("cursor", {
        auth: { state, advertisedMethods: ["cursor_login"], safeLogout: false },
      });
      const decoded = decodeAcpProviderSettings(response(fixture("grok"), cursor));
      expect(decoded.state === "loaded" && decoded.providers[1]?.auth.state).toBe(state);
    }
    const serialized = JSON.stringify(decodeAcpProviderSettings(response()));
    expect(serialized).toContain("cached_token");
    expect(serialized).toContain("xai.api_key");
    expect(serialized).not.toContain("XAI_API_KEY=");
    expect(serialized).not.toContain("Bearer ");
  });

  test("renders non-color status, dynamic provenance, authority withholding, and accessible typed actions", () => {
    const cursor = fixture("cursor", {
      auth: { state: "authenticated", advertisedMethods: ["cursor_login"], safeLogout: false },
      configuration: [
        {
          id: "model",
          label: "Model",
          value: "composer-2",
          provenance: "stable",
          state: "reconciled",
        },
        {
          id: "mode",
          label: "Mode",
          value: "agent",
          provenance: "peer-extension",
          state: "pending",
        },
      ],
    });
    const decoded = decodeAcpProviderSettings(response(fixture("grok"), cursor));
    const view = acpProviderSettingsView(decoded);
    expect(byKey(view, "settings-acp-copy")?.content).toContain(
      "does not imply support for every ACP agent",
    );
    expect(byKey(view, "settings-acp-cursor-profile")?.label).toBe("experimental");
    expect(byKey(view, "settings-acp-cursor-authority")?.content).toContain("not active");
    expect(byKey(view, "settings-acp-cursor-config-mode")?.content).toContain(
      "peer-extension · experimental · pending",
    );
    expect((byKey(view, "settings-acp-cursor-new_session")?.a11y as { label?: string }).label).toBe(
      "New session for Cursor Agent CLI",
    );
  });

  test("distinguishes cancelling from escalated process termination and offers recovery", () => {
    const cancelling = fixture("grok", {
      session: { ...fixture("grok").session, state: "cancelling", canCancel: true },
    });
    const escalated = fixture("cursor", {
      session: { ...fixture("cursor").session, state: "cancel_escalated" },
      diagnosticCodes: ["cancel_escalated"],
    });
    const view = acpProviderSettingsView(
      decodeAcpProviderSettings(response(cancelling, escalated)),
    );
    expect(byKey(view, "settings-acp-grok-session")?.content).toBe("Session cancelling");
    expect(byKey(view, "settings-acp-cursor-session")?.content).toBe("Session cancel_escalated");
    expect(byKey(view, "settings-acp-cursor-diagnostics")?.content).toContain("cancel_escalated");
  });

  test("exports a structural redacted support bundle with no path, auth payload, prompt, or configuration values", () => {
    const decoded = decodeAcpProviderSettings(response());
    const bundle = buildAcpSupportBundle({
      generatedAt: "2026-07-16T12:00:00.000Z",
      providers:
        decoded.state === "loaded"
          ? decoded.providers.map((projection) => ({
              projection,
              receiptRefs: ["receipt.safe.1"],
              evidenceRefs: ["evidence.safe.1"],
            }))
          : [],
    });
    const serialized = JSON.stringify(bundle);
    expect(bundle.providers).toHaveLength(2);
    expect(serialized).toContain("acp.grok.0.2.101.darwin-arm64");
    expect(serialized).not.toContain("/opt/");
    expect(serialized).not.toContain("cached_token");
    expect(serialized).not.toContain("xai.api_key");
    expect(serialized).not.toContain("cursor_login");
    expect(serialized).not.toContain("composer-2");
  });

  test("validates action payloads as a closed provider/action pair", () => {
    expect(decodeAcpProviderActionPayload("cursor:reauthenticate")).toEqual({
      provider: "cursor",
      action: "reauthenticate",
    });
    expect(decodeAcpProviderActionPayload("grok:cancel:extra")).toBeNull();
    expect(decodeAcpProviderActionPayload("all:probe")).toBeNull();
    expect(decodeAcpProviderActionPayload("grok:install-shell-string")).toBeNull();
  });
});
