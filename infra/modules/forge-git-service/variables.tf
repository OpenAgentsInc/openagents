variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "Cloud Run region"
  type        = string
}

variable "filestore_zone" {
  description = "Zone for the repository Filestore instance"
  type        = string
}

variable "service_name" {
  description = "Cloud Run service name"
  type        = string
  default     = "forge-git"
}

variable "filestore_name" {
  description = "Filestore instance name"
  type        = string
  default     = "forge-git-repositories"
}

variable "file_share_name" {
  description = "Filestore NFS share name"
  type        = string
  default     = "forge_repositories"
}

variable "capacity_gb" {
  description = "Filestore BASIC_HDD capacity in GiB"
  type        = number
  default     = 1024

  validation {
    condition     = var.capacity_gb >= 1024
    error_message = "A BASIC_HDD Filestore share must have at least 1024 GiB."
  }
}

variable "network" {
  description = "VPC network for Filestore and Direct VPC egress"
  type        = string
  default     = "default"
}

variable "subnetwork" {
  description = "VPC subnetwork for Direct VPC egress"
  type        = string
  default     = "default"
}

variable "network_tag" {
  description = "Direct VPC network tag for the Filestore egress rule"
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

variable "repository_mount_path" {
  description = "Container path for authoritative bare repositories"
  type        = string
  default     = "/var/lib/forge/repositories"
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
