# Single-env (prod) today. To add staging later: copy this root module to
# infra/staging/, change the backend prefix + tfvars, and reuse the same
# modules — everything environment-specific already flows through variables.

variable "project_id" {
  description = "GCP project ID"
  type        = string
  default     = "openagentsgemini"
}

variable "region" {
  description = "Default region"
  type        = string
  default     = "us-central1"
}

variable "portable_checkpoint_kms_crypto_key_resource" {
  description = "Existing full CryptoKey resource for portable checkpoint DEK wrap and unwrap. Null keeps the IAM grant absent."
  type        = string
  default     = null
  nullable    = true

  validation {
    condition = var.portable_checkpoint_kms_crypto_key_resource == null || can(regex(
      "^projects/[a-z][a-z0-9-]{4,62}/locations/[a-z0-9-]+/keyRings/[A-Za-z0-9_-]{1,63}/cryptoKeys/[A-Za-z0-9_-]{1,63}$",
      var.portable_checkpoint_kms_crypto_key_resource,
    ))
    error_message = "The portable checkpoint KMS value must be a full CryptoKey resource."
  }
}

variable "portable_checkpoint_kms_key_ref" {
  description = "Public-safe key reference for v3 portable checkpoint envelopes. Deploy the same value as PORTABLE_CHECKPOINT_KMS_KEY_REF."
  type        = string
  default     = "key.portable-checkpoint.production.v1"

  validation {
    condition     = can(regex("^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$", var.portable_checkpoint_kms_key_ref))
    error_message = "The portable checkpoint KMS key reference must be public-safe."
  }
}

variable "portable_checkpoint_kms_runtime_service_account_email" {
  description = "Cloud Run workload identity that can encrypt and decrypt portable checkpoint DEKs."
  type        = string
  default     = "157437760789-compute@developer.gserviceaccount.com"
}
