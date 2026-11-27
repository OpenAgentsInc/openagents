# 6. A document tries to become the system prompt

> **Status: partly implemented.** OpenAgents Core enforces workspace trust
> boundaries, managed-only hook precedence, `DENY > ASK > ALLOW` hook merging,
> untrusted `projectSettings` opt-in refusal, DNS-rebinding-safe SSRF IPv4/IPv6
> blocking, CRLF header rejection, and shell differential prechecks under
> `#authority`, alongside live read ceilings in `openagents/daemon/host.coax` (see
> [Chapter 20](ch020-an-agent-that-stays-online.md)). Full automated adversarial
> red-team suites across every live model provider remain open.

Prompt injection is not only adversarial prose inside a README; in a real
coding agent, an untrusted repository can try to escalate authority through
`.claude/settings.json` opt-ins, repository hooks running before workspace
trust is granted, environment variable overrides, SSRF via HTTP hooks, CRLF
header smuggling, or shell syntax differentials (`IFS=: read a`).

Because an LLM can always be tricked by untrusted text, OpenAgents refuses to
rely on prompt wording alone. Authority boundaries live in pure `#authority`
Coax code before untrusted repository files can influence execution:

- **Pre-trust RCE & enterprise hook gates (`openagents/core/hooks.coax`):**
  `flawed_hook_central_gate_allows` refuses interactive hooks until workspace
  trust is accepted; `flawed_hook_source_allowed` enforces 4-tier enterprise
  precedence so `policy` hooks cannot be overridden by repo `settings`; parallel
  hook results merge strictly as `DENY > ASK > ALLOW`; and `ConfigChange` hooks
  can never veto managed `policySettings`.
- **Untrusted repository config isolation (`openagents/core/config.coax`,
  `openagents/core/permissions.coax`):** `flawed_cfg_source_trusted_for_optin`
  and `flawed_auto_mode_opt_in_trusted` reject `projectSettings` for dangerous
  opt-ins (`skipDangerousModePermissionPrompt`, auto mode), and
  `flawed_env_apply_allowed` blocks untrusted repository environment overrides
  before trust acceptance.
- **HTTP hook SSRF & header hygiene (`openagents/core/hooks.coax`):**
  `flawed_ssrf_is_blocked_ipv6` catches IPv4-mapped cloud metadata addresses
  (`::ffff:169.254.169.254`) while `flawed_http_header_value_clean` rejects
  CRLF/NUL header smuggling.
- **Pre-AST shell differential prechecks (`openagents/core/shell_sandbox.coax`):**
  `flawed_bash_precheck_safe` catches `IFS` manipulation and control-character
  smuggling before command execution.

## Profile

This chapter's profile is `openagents-trust-boundaries-v1, bounded CPU`:

- Pre-trust interactive hooks are refused; managed-only policy allows `POLICY`
  hooks and rejects repository `SETTINGS` hooks.
- Parallel `ALLOW + DENY` resolves to `DENY`, and `ConfigChange` cannot block
  `policySettings`.
- Untrusted `projectSettings` cannot enable dangerous opt-ins or unsafe
  environment variables.
- IPv4-mapped metadata SSRF targets and CRLF headers are refused, and `IFS`
  shell smuggling fails `flawed_bash_precheck_safe`.

## Worked example

The runnable example [prompt-boundaries.coax](../examples/prompt-boundaries.coax)
exercises all five authority walls directly against `openagents/core/`.

<!-- source: coder/examples/prompt-boundaries.coax example -->
```jai
main :: () {
  // 1. Pre-trust workspace RCE block: project hooks cannot run before trust acceptance
  pre_trust_hook := flawed_hook_central_gate_allows(false, false, true, false);
  post_trust_hook := flawed_hook_central_gate_allows(false, false, true, true);
  // Managed-only enterprise mode allows POLICY hooks and blocks repo SETTINGS hooks
  ent_policy := flawed_hook_source_allowed(FLAWED_HSRC_POLICY, false, false, false, true);
  ent_repo := flawed_hook_source_allowed(FLAWED_HSRC_SETTINGS, false, false, false, true);
  print("hook trust gate: pre_trust=% post_trust=% managed_policy=% managed_repo=%\n",
    pre_trust_hook, post_trust_hook, ent_policy, ent_repo);

  // 2. Parallel hook merge is strictly DENY > ASK > ALLOW; ConfigChange cannot veto policySettings
  merged := flawed_hook_merge_permission(FLAWED_PBEH_ALLOW, FLAWED_PBEH_DENY);
  policy_vetoed := flawed_hook_config_change_blocked(true, true);
  print("hook merge: allow+deny_is_deny=% policy_config_change_vetoed=%\n",
    merged == FLAWED_PBEH_DENY, policy_vetoed);

  // 3. Untrusted repo settings (.claude/settings.json) cannot opt into bypass/auto mode or unsafe env vars
  optin_project := flawed_cfg_source_trusted_for_optin(FLAWED_CSRC_PROJECT);
  optin_user := flawed_cfg_source_trusted_for_optin(FLAWED_CSRC_USER);
  auto_project := flawed_auto_mode_opt_in_trusted(FLAWED_PSRC_PROJECT);
  env_untrusted := flawed_env_apply_allowed(FLAWED_CSRC_PROJECT, false, false, false, false, false, false, false, false);
  print("repo settings trust: optin_proj=% optin_user=% auto_proj=% env_pre_trust=%\n",
    optin_project, optin_user, auto_project, env_untrusted);

  // 4. HTTP hook SSRF & CRLF header injection guard (blocks IPv4-mapped metadata 169.254.169.254)
  v6_meta: [8] u16;
  v6_meta[5] = 0xffff;
  v6_meta[6] = 0xa9fe;
  v6_meta[7] = 0xa9fe;
  ssrf_meta := flawed_ssrf_is_blocked_ipv6(v6_meta[..]);
  ssrf_loopback := flawed_ssrf_is_blocked_ipv4(127, 0);
  hdr_crlf := flawed_http_header_value_clean("Bearer tok\r\nX-Injected: 1");
  hdr_clean := flawed_http_header_value_clean("Bearer tok_ok");
  print("http hook guard: ipv6_meta_blocked=% loopback_blocked=% crlf_clean=% normal_clean=%\n",
    ssrf_meta, ssrf_loopback, hdr_crlf, hdr_clean);

  // 5. Pre-AST shell differential precheck catches IFS / control-char smuggling before execution
  print("shell precheck: ifs_attack_safe=% git_status_safe=%\n",
    flawed_bash_precheck_safe("IFS=: read a"), flawed_bash_precheck_safe("git status"));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/prompt-boundaries.coax
```

```text
hook trust gate: pre_trust=false post_trust=true managed_policy=true managed_repo=false
hook merge: allow+deny_is_deny=true policy_config_change_vetoed=false
repo settings trust: optin_proj=false optin_user=true auto_proj=false env_pre_trust=false
http hook guard: ipv6_meta_blocked=true loopback_blocked=false crlf_clean=false normal_clean=true
shell precheck: ifs_attack_safe=false git_status_safe=true
```

The checked fixture is [prompt-boundaries.out](../examples/prompt-boundaries.out).

## What remains

- Continuous live red-team evaluation of untrusted repository documents and
  channel inputs across every supported model engine.

## Done means

- [x] Pre-trust hook blocking, enterprise policy precedence, untrusted
  `projectSettings` opt-in refusal, SSRF/CRLF guards, and shell prechecks pass
  `#authority` checks and are adopted by `openagents/runtime/oa-worker.coax`.
- [x] `coder/examples/prompt-boundaries.coax` matches
  `coder/examples/prompt-boundaries.out` in debug and release.
- [ ] Live multi-engine prompt-injection red-team scorecard runs routinely on
  every supported model provider.
