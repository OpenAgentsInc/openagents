variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "Cloud Run and network region"
  type        = string
}

variable "zone" {
  description = "GCE zone for the NFS host and repository disk"
  type        = string
}

variable "service_name" {
  description = "Cloud Run service name"
  type        = string
  default     = "forge-git"
}

variable "nfs_instance_name" {
  description = "Dedicated GCE NFS host name"
  type        = string
  default     = "forge-git-nfs"
}

variable "nfs_machine_type" {
  description = "Machine type for the dedicated NFS host"
  type        = string
  default     = "e2-small"
}

variable "nfs_service_account_id" {
  description = "Service account ID for the NFS host"
  type        = string
  default     = "forge-git-nfs"
}

variable "repository_disk_name" {
  description = "Persistent disk that is the sole bare-repository authority"
  type        = string
  default     = "forge-git-repositories"
}

variable "repository_disk_size_gb" {
  description = "Balanced persistent disk capacity in GiB"
  type        = number
  default     = 100

  validation {
    condition     = var.repository_disk_size_gb >= 50
    error_message = "The repository disk must have at least 50 GiB."
  }
}

variable "nfs_export_path" {
  description = "Authoritative repository export path on the NFS host"
  type        = string
  default     = "/srv/forge/repositories"
}

variable "network" {
  description = "Existing VPC network self-link or name"
  type        = string
  default     = "default"
}

variable "subnetwork_name" {
  description = "Dedicated Direct VPC subnetwork name"
  type        = string
  default     = "forge-git"
}

variable "subnetwork_cidr" {
  description = "Dedicated Direct VPC and NFS client range"
  type        = string
  default     = "10.42.24.0/26"

  validation {
    condition     = can(cidrhost(var.subnetwork_cidr, 62))
    error_message = "The Forge Git subnet must be /26 or larger."
  }
}

variable "nfs_network_tag" {
  description = "Network tag for the NFS host"
  type        = string
  default     = "forge-git-nfs"
}

variable "cloud_run_network_tag" {
  description = "Direct VPC tag for the Forge Git service"
  type        = string
  default     = "forge-git"
}

variable "runtime_service_account_id" {
  description = "Service account ID for the Forge Git runtime"
  type        = string
  default     = "forge-git-runtime"
}

variable "pack_evidence_bucket" {
  description = "GCS bucket that keeps pack evidence and mirrors"
  type        = string
}

variable "pack_evidence_prefix" {
  description = "Object prefix for Forge Git pack evidence and mirrors"
  type        = string
  default     = "forge/git-packs"
}

variable "database_secret_id" {
  description = "Secret Manager container for the Forge Git Postgres URL"
  type        = string
  default     = "openagents-monolith-database-url-prod"
}

variable "database_password_secret_id" {
  description = "Password-only Secret Manager container for the Forge Git Postgres role"
  type        = string
  default     = "openagents-monolith-pgpassword"
}

variable "policy_authority_secret_id" {
  description = "Secret Manager container for the Forge Git to worker policy bearer"
  type        = string
  default     = "openagents-forge-git-policy-authority-token"
}

variable "database_instance_connection_name" {
  description = "Cloud SQL instance connection name for Forge Git authentication"
  type        = string
  default     = "openagentsgemini:us-central1:khala-sync-pg"
}

variable "repository_mount_path" {
  description = "Container path for authoritative bare repositories"
  type        = string
  default     = "/var/lib/forge/repositories"
}

variable "snapshot_start_time" {
  description = "UTC start time for the daily repository snapshot"
  type        = string
  default     = "08:00"
}

variable "snapshot_retention_days" {
  description = "Days to keep automatic repository snapshots"
  type        = number
  default     = 3
}

variable "request_timeout" {
  description = "Cloud Run request timeout for Git transport requests"
  type        = string
  default     = "3600s"
}

variable "request_concurrency" {
  description = "Maximum requests on the one Cloud Run instance"
  type        = number
  default     = 80
}

variable "placeholder_image" {
  description = "Image used only for the first Terraform create"
  type        = string
  default     = "us-docker.pkg.dev/cloudrun/container/hello"
}

variable "labels" {
  description = "Labels for Forge Git resources"
  type        = map(string)
  default = {
    product   = "forge"
    component = "git"
    managedby = "terraform"
  }
}
