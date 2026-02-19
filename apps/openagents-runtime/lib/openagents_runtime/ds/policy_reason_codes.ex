defmodule OpenAgentsRuntime.DS.PolicyReasonCodes do
  @moduledoc """
  Canonical DS/runtime policy reason taxonomy.

  Contract source-of-truth:
  - docs/protocol/reasons/runtime-policy-reason-codes.v1.json
  """

  @version "runtime-policy-reasons.v1"

  @reason_codes [
    "policy_allowed.default",
    "policy_denied.explicit_deny",
    "policy_denied.consent_required",
    "policy_denied.suppressed_recipient",
    "policy_denied.authorization_missing",
    "policy_denied.authorization_expired",
    "policy_denied.authorization_revoked",
    "policy_denied.invalid_authorization_mode",
    "policy_denied.budget_exhausted",
    "loop_detected.no_progress",
    "ssrf_block.private_address",
    "ssrf_block.metadata_endpoint",
    "ssrf_block.host_not_allowed",
    "ssrf_block.invalid_url",
    "ssrf_block.dns_resolution_failed",
    "ssrf_block.dns_pin_mismatch",
    "ssrf_block.redirect_limit_exceeded",
    "ssrf_block.redirect_missing_location",
    "ssrf_block.redirect_invalid_location",
    "manifest_validation.invalid_schema",
    "comms_failed.provider_error",
    "comms_failed.provider_circuit_open",
    "comms_failed.fallback_exhausted",
    "coding_failed.provider_error",
    "coding_failed.provider_circuit_open"
  ]

  @spec version() :: String.t()
  def version, do: @version

  @spec all() :: [String.t()]
  def all, do: @reason_codes

  @spec valid?(String.t()) :: boolean()
  def valid?(reason_code) when is_binary(reason_code), do: reason_code in @reason_codes
  def valid?(_), do: false
end
