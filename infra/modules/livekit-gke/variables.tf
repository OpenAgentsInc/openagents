variable "project_id" {
  description = "Google Cloud project that owns the LiveKit cluster."
  type        = string
}

variable "region" {
  description = "Google Cloud region for the regional cluster and Redis."
  type        = string
}

variable "zones" {
  description = "Zones used by the regional cluster."
  type        = list(string)

  validation {
    condition     = length(var.zones) >= 2
    error_message = "The LiveKit cluster must span at least two zones."
  }
}

variable "environment" {
  description = "Deployment environment name."
  type        = string

  validation {
    condition     = contains(["staging", "production"], var.environment)
    error_message = "The LiveKit environment must be staging or production."
  }
}

variable "cluster_name" {
  description = "Regional GKE Standard cluster name."
  type        = string
}

variable "network_id" {
  description = "Dedicated LiveKit VPC resource ID."
  type        = string
}

variable "subnetwork_id" {
  description = "Dedicated LiveKit node subnetwork resource ID."
  type        = string
}

variable "pod_range_name" {
  description = "Secondary range name for GKE pods."
  type        = string
}

variable "service_range_name" {
  description = "Secondary range name for GKE services."
  type        = string
}

variable "master_authorized_networks" {
  description = "CIDRs allowed to reach the public GKE control-plane endpoint."
  type = list(object({
    cidr_block   = string
    display_name = string
  }))

  validation {
    condition = (
      length(var.master_authorized_networks) > 0 &&
      alltrue([
        for network in var.master_authorized_networks :
        network.cidr_block != "0.0.0.0/0"
      ])
    )
    error_message = "At least one bounded master authorized network is required; 0.0.0.0/0 is forbidden."
  }
}

variable "release_channel" {
  description = "GKE release channel."
  type        = string
  default     = "STABLE"

  validation {
    condition     = contains(["REGULAR", "STABLE"], var.release_channel)
    error_message = "The LiveKit cluster release channel must be REGULAR or STABLE."
  }
}

variable "deletion_protection" {
  description = "Protect the regional cluster from deletion."
  type        = bool
  default     = true
}

variable "secret_deletion_protection" {
  description = "Protect production Secret Manager containers from deletion."
  type        = bool
  default     = true
}

variable "sfu_machine_type" {
  description = "Compute-optimized machine type for public SFU nodes."
  type        = string
  default     = "c2-standard-8"
}

variable "sfu_disk_size_gb" {
  description = "Boot disk size for SFU nodes."
  type        = number
  default     = 100
}

variable "sfu_min_nodes" {
  description = "Minimum total SFU node count across the regional pool."
  type        = number
  default     = 3
}

variable "sfu_max_nodes" {
  description = "Maximum total SFU node count across the regional pool."
  type        = number
  default     = 7
}

variable "app_machine_type" {
  description = "Machine type for signaling, agent-worker, and observability workloads."
  type        = string
  default     = "e2-standard-8"
}

variable "app_disk_size_gb" {
  description = "Boot disk size for application nodes."
  type        = number
  default     = 100
}

variable "app_min_nodes" {
  description = "Minimum total application node count across the regional pool."
  type        = number
  default     = 3
}

variable "app_max_nodes" {
  description = "Maximum total application node count across the regional pool."
  type        = number
  default     = 4
}

variable "sfu_network_tag" {
  description = "Network tag that receives direct WebRTC and TURN traffic."
  type        = string
  default     = "livekit-sfu"
}

variable "redis_name" {
  description = "HA Memorystore instance name."
  type        = string
}

variable "redis_memory_size_gb" {
  description = "Memory allocated to the HA Redis instance."
  type        = number
  default     = 5

  validation {
    condition     = var.redis_memory_size_gb >= 5
    error_message = "The HA Redis instance must have at least 5 GiB."
  }
}

variable "redis_version" {
  description = "Pinned Memorystore Redis major/minor version."
  type        = string
  default     = "REDIS_7_2"
}

variable "namespace" {
  description = "Kubernetes namespace used by LiveKit workloads."
  type        = string
  default     = "livekit"
}

variable "node_service_account_id" {
  description = "Stable Google service account ID for GKE nodes."
  type        = string
}

variable "server_service_account_id" {
  description = "Stable Google service account ID mapped to the LiveKit server KSA."
  type        = string
}

variable "agent_service_account_id" {
  description = "Stable Google service account ID mapped to the Sarah agent KSA."
  type        = string
}

variable "server_ksa_name" {
  description = "Kubernetes service account used by LiveKit servers."
  type        = string
  default     = "livekit-server"
}

variable "agent_ksa_name" {
  description = "Kubernetes service account used by Sarah agent workers."
  type        = string
  default     = "sarah-agent"
}

variable "secret_reader_service_account_id" {
  description = "Stable Google service account ID for the LiveKit External Secrets reader."
  type        = string
}

variable "secret_reader_ksa_name" {
  description = "Kubernetes service account used by External Secrets for server and Redis credentials."
  type        = string
  default     = "livekit-secret-reader"
}

variable "dns_secret_reader_service_account_id" {
  description = "Stable Google service account ID for the cert-manager External Secrets reader."
  type        = string
}

variable "dns_secret_reader_ksa_name" {
  description = "Kubernetes service account used by External Secrets only for the Cloudflare DNS token."
  type        = string
  default     = "livekit-cert-manager-secret-reader"
}

variable "sarah_secret_reader_service_account_id" {
  description = "Stable Google service account ID for the Sarah worker External Secrets reader."
  type        = string
}

variable "sarah_secret_reader_ksa_name" {
  description = "Kubernetes service account used by External Secrets only for Sarah worker credentials."
  type        = string
  default     = "oa-livekit-sarah-secret-reader"
}

variable "labels" {
  description = "Public-safe labels applied to resources."
  type        = map(string)
  default     = {}
}
