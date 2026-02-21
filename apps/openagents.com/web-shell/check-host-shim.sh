#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOST_DIR="${SCRIPT_DIR}/host"

if [[ ! -d "${HOST_DIR}" ]]; then
  echo "error: host directory not found at ${HOST_DIR}" >&2
  exit 1
fi

host_js_files="$(find "${HOST_DIR}" -type f \( -name "*.js" -o -name "*.mjs" \) | sort)"
if [[ -z "${host_js_files}" ]]; then
  echo "error: no host shim JS files found under ${HOST_DIR}" >&2
  exit 1
fi

while IFS= read -r file; do
  filename="$(basename "${file}")"

  if [[ "${filename}" == "host-shim.js" ]]; then
    if rg -n -i '\b(xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >/dev/null; then
      echo "error: host shim contains prohibited state/network primitives (${file})" >&2
      rg -n -i '\b(xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >&2
      exit 1
    fi

    if rg -n -i '\bfetch\s*\(' "${file}" >/dev/null; then
      if ! rg -n '/manifest\.json' "${file}" >/dev/null; then
        echo "error: host shim fetch usage must be limited to static manifest polling (${file})" >&2
        rg -n -i '\bfetch\s*\(' "${file}" >&2
        exit 1
      fi
      if rg -n '/api/' "${file}" >/dev/null; then
        echo "error: host shim must not fetch product API routes (${file})" >&2
        rg -n '/api/' "${file}" >&2
        exit 1
      fi
    fi
  elif [[ "${filename}" == "sw-template.js" ]]; then
    if rg -n -i '\b(xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >/dev/null; then
      echo "error: service-worker template contains prohibited state/network primitives (${file})" >&2
      rg -n -i '\b(xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >&2
      exit 1
    fi
    if rg -n '/api/' "${file}" >/dev/null; then
      echo "error: service-worker template must not reference product API routes (${file})" >&2
      rg -n '/api/' "${file}" >&2
      exit 1
    fi
  else
    if rg -n -i '\b(fetch|xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >/dev/null; then
      echo "error: host helper must not contain product state/network primitives (${file})" >&2
      rg -n -i '\b(fetch|xmlhttprequest|axios|localstorage|sessionstorage|document\.cookie)\b' "${file}" >&2
      exit 1
    fi
  fi

  if rg -n -i '\b(auth|session|token|route(state|r)?|feature(flag)?|business)\b' "${file}" >/dev/null; then
    echo "error: host shim contains prohibited product-logic keywords (${file})" >&2
    rg -n -i '\b(auth|session|token|route(state|r)?|feature(flag)?|business)\b' "${file}" >&2
    exit 1
  fi
done <<< "${host_js_files}"

echo "web-shell host shim boundary check passed"
