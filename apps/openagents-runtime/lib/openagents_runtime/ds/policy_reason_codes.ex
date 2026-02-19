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
    "manifest_validation.invalid_schema",
    "comms_failed.provider_error"
  ]

  @spec version() :: String.t()
  def version, do: @version

  @spec all() :: [String.t()]
  def all, do: @reason_codes

  @spec valid?(String.t()) :: boolean()
  def valid?(reason_code) when is_binary(reason_code), do: reason_code in @reason_codes
  def valid?(_), do: false
end
