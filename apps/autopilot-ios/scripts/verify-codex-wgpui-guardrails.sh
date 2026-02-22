#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
IOS_APP_ROOT="apps/autopilot-ios/Autopilot/Autopilot"

# Emergency escape hatch for local debugging only.
if [[ "${OA_IOS_ALLOW_SWIFT_CODEX_LOGIC_DIFF:-0}" == "1" ]]; then
  echo "Skipping iOS Codex WGPUI guardrails (OA_IOS_ALLOW_SWIFT_CODEX_LOGIC_DIFF=1)."
  exit 0
fi

collect_changed_files() {
  local files
  files="$(git -C "${ROOT_DIR}" diff --cached --name-only)"
  if [[ -z "${files}" ]]; then
    files="$(git -C "${ROOT_DIR}" diff --name-only HEAD)"
  fi
  printf '%s\n' "${files}" | sed '/^$/d'
}

collect_name_status() {
  local files
  files="$(git -C "${ROOT_DIR}" diff --cached --name-status)"
  if [[ -z "${files}" ]]; then
    files="$(git -C "${ROOT_DIR}" diff --name-status HEAD)"
  fi
  printf '%s\n' "${files}" | sed '/^$/d'
}

diff_for_file() {
  local file="$1"
  local diff
  diff="$(git -C "${ROOT_DIR}" diff --cached --unified=0 -- "${file}")"
  if [[ -z "${diff}" ]]; then
    diff="$(git -C "${ROOT_DIR}" diff --unified=0 HEAD -- "${file}")"
  fi
  printf '%s\n' "${diff}"
}

print_violation() {
  local message="$1"
  local details="$2"
  echo "guardrail violation: ${message}" >&2
  if [[ -n "${details}" ]]; then
    printf '%s\n' "${details}" >&2
  fi
}

changed_files="$(collect_changed_files)"
if [[ -z "${changed_files}" ]]; then
  echo "No changed files detected; skipping iOS Codex WGPUI guardrails."
  exit 0
fi

ios_swift_files="$(printf '%s\n' "${changed_files}" | rg '^apps/autopilot-ios/Autopilot/Autopilot/.*\.swift$' || true)"
if [[ -z "${ios_swift_files}" ]]; then
  echo "No iOS Swift changes detected; iOS Codex WGPUI guardrails passed."
  exit 0
fi

failures=0

# Rule 1: Block new Codex/Khala Swift files in production iOS app lane.
name_status="$(collect_name_status)"
new_codex_files="$(
  printf '%s\n' "${name_status}" \
    | awk '$1 == "A" { print $2 }' \
    | rg "^${IOS_APP_ROOT}/.*(Codex|RuntimeCodex|Khala|Handshake|Chat).*\.swift$" \
    || true
)"
if [[ -n "${new_codex_files}" ]]; then
  print_violation \
    "new Codex/Khala Swift files are not allowed in the iOS app lane" \
    "${new_codex_files}"
  failures=$((failures + 1))
fi

# Rule 2: Block SwiftUI product-surface additions on Codex-related files.
swiftui_surface_pattern='^\+.*(VStack\(|HStack\(|ZStack\(|LazyVStack\(|LazyHStack\(|List\(|Form\(|NavigationStack\(|NavigationView\(|TabView\(|TextField\(|SecureField\(|TextEditor\(|ScrollView\(|ForEach\()'

# Rule 3: Block re-introduction of Swift-owned Codex state-machine/business logic.
state_decl_pattern='^\+\s*(struct|enum|final class|class)\s+(CodexAuthFlowState|CodexLifecycleResumeState|CodexResumeCheckpoint(Store)?|KhalaReconnectPolicy|KhalaReconnectClassifier|CodexStreamingTextAssembler|CodexAssistantDelta(Source|Decision|Policy)|CodexChatEventDisplayPolicy|RuntimeCodexControlCoordinator)\b'
state_method_pattern='^\+\s*private func\s+(preferredWorker|isDesktopWorker|freshnessRank|sharedWorkerRank|processKhalaUpdateBatch|handleIncoming|applyCodexEvent)\b'

while IFS= read -r file; do
  [[ -z "${file}" ]] && continue

  if [[ "${file}" != ${IOS_APP_ROOT}/* ]]; then
    continue
  fi

  if ! printf '%s\n' "${file}" | rg -q '(Codex|RuntimeCodex|Khala|Handshake|Chat)'; then
    continue
  fi

  diff_text="$(diff_for_file "${file}")"
  if [[ -z "${diff_text}" ]]; then
    continue
  fi

  added_lines="$(printf '%s\n' "${diff_text}" | rg '^\+' | rg -v '^\+\+\+' || true)"
  if [[ -z "${added_lines}" ]]; then
    continue
  fi

  swiftui_hits="$(printf '%s\n' "${added_lines}" | rg -n "${swiftui_surface_pattern}" || true)"
  if [[ -n "${swiftui_hits}" ]]; then
    print_violation \
      "forbidden SwiftUI Codex product UI additions in ${file}" \
      "${swiftui_hits}"
    failures=$((failures + 1))
  fi

  state_decl_hits="$(printf '%s\n' "${added_lines}" | rg -n "${state_decl_pattern}" || true)"
  if [[ -n "${state_decl_hits}" ]]; then
    print_violation \
      "forbidden Swift-owned Codex state/business declaration additions in ${file}" \
      "${state_decl_hits}"
    failures=$((failures + 1))
  fi

  state_method_hits="$(printf '%s\n' "${added_lines}" | rg -n "${state_method_pattern}" || true)"
  if [[ -n "${state_method_hits}" ]]; then
    print_violation \
      "forbidden Swift-owned Codex state/business method additions in ${file}" \
      "${state_method_hits}"
    failures=$((failures + 1))
  fi

done <<< "${ios_swift_files}"

if [[ "${failures}" -gt 0 ]]; then
  echo "iOS Codex WGPUI guardrails failed with ${failures} violation(s)." >&2
  exit 1
fi

echo "iOS Codex WGPUI guardrails passed."
