variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "name" {
  description = "Cloud SQL instance name"
  type        = string
}

variable "region" {
  description = "Instance region"
  type        = string
}

variable "database_version" {
  description = "Postgres major version, e.g. POSTGRES_17"
  type        = string
}

variable "tier" {
  description = "Machine tier, e.g. db-custom-8-53248 or db-f1-micro"
  type        = string
}

variable "edition" {
  description = "ENTERPRISE or ENTERPRISE_PLUS"
  type        = string
  default     = "ENTERPRISE"
}

variable "availability_type" {
  description = "ZONAL or REGIONAL"
  type        = string
  default     = "ZONAL"
}

variable "zone" {
  description = "Preferred primary zone"
  type        = string
  default     = null
}

variable "secondary_zone" {
  description = "Preferred secondary zone (REGIONAL only)"
  type        = string
  default     = null
}

variable "disk_size_gb" {
  description = "Data disk size in GB"
  type        = number
}

variable "disk_type" {
  description = "PD_SSD or PD_HDD"
  type        = string
  default     = "PD_SSD"
}

variable "backups_enabled" {
  description = "Enable automated backups"
  type        = bool
  default     = true
}

variable "backup_start_time" {
  description = "Backup window start (HH:MM, UTC)"
  type        = string
  default     = "08:00"
}

variable "pitr_enabled" {
  description = "Enable point-in-time recovery (WAL archiving)"
  type        = bool
  default     = false
}

variable "transaction_log_retention_days" {
  description = "Days of transaction logs retained for PITR"
  type        = number
  default     = 7
}

variable "retained_backups" {
  description = "Number of automated backups to retain"
  type        = number
  default     = 7
}

variable "database_flags" {
  description = "Map of database flag name -> value"
  type        = map(string)
  default     = {}
}

variable "authorized_networks" {
  description = "List of authorized networks (name may be empty string)"
  type = list(object({
    name  = string
    value = string
  }))
  default = []
}

variable "ssl_mode" {
  description = "ALLOW_UNENCRYPTED_AND_ENCRYPTED, ENCRYPTED_ONLY, or TRUSTED_CLIENT_CERTIFICATE_REQUIRED"
  type        = string
  default     = "ENCRYPTED_ONLY"
}

variable "gcp_deletion_protection" {
  description = "GCP-side deletionProtectionEnabled flag (mirrors live setting)"
  type        = bool
  default     = false
}

variable "enable_dataplex_integration" {
  description = "Dataplex integration flag (null = leave as-is on the instance)"
  type        = bool
  default     = null
}

variable "users" {
  description = "BUILT_IN Postgres user names to manage (passwords managed out of band)"
  type        = list(string)
  default     = []
}
