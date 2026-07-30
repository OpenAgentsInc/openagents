variable "project_id" {
  description = "Google Cloud project that owns the isolated LiveKit network."
  type        = string
}

variable "region" {
  description = "Google Cloud region for the LiveKit data plane."
  type        = string
}

variable "environment" {
  description = "Deployment environment name."
  type        = string

  validation {
    condition     = contains(["staging", "production"], var.environment)
    error_message = "The LiveKit environment must be staging or production."
  }
}

variable "network_name" {
  description = "Name of the dedicated custom-mode VPC."
  type        = string
}

variable "node_subnet_cidr" {
  description = "Primary CIDR for GKE nodes."
  type        = string
}

variable "pod_cidr" {
  description = "Secondary CIDR for GKE pods."
  type        = string
}

variable "service_cidr" {
  description = "Secondary CIDR for GKE services."
  type        = string
}

variable "service_networking_prefix_length" {
  description = "Prefix length reserved for Private Service Access."
  type        = number
  default     = 20

  validation {
    condition     = var.service_networking_prefix_length >= 16 && var.service_networking_prefix_length <= 24
    error_message = "The Private Service Access prefix must be between /16 and /24."
  }
}

variable "enable_private_service_access" {
  description = "Create the Private Service Access range and peering required by production Redis."
  type        = bool
  default     = true
}

variable "enable_sfu_firewalls" {
  description = "Create direct-media and Google health-check firewall rules for GKE SFU nodes."
  type        = bool
  default     = true
}

variable "media_udp_port_range" {
  description = "Direct WebRTC UDP port range exposed only on SFU nodes."
  type = object({
    start = number
    end   = number
  })
  default = {
    start = 50000
    end   = 60000
  }

  validation {
    condition = (
      var.media_udp_port_range.start >= 1024 &&
      var.media_udp_port_range.end <= 65535 &&
      var.media_udp_port_range.start < var.media_udp_port_range.end
    )
    error_message = "The media UDP range must be ordered and within ports 1024-65535."
  }
}

variable "tcp_fallback_port" {
  description = "LiveKit TCP fallback port exposed on SFU nodes."
  type        = number
  default     = 7881
}

variable "turn_tls_port" {
  description = "TURN/TLS port exposed on SFU nodes."
  type        = number
  default     = 5349
}

variable "turn_udp_port" {
  description = "TURN/UDP port exposed on SFU nodes."
  type        = number
  default     = 443
}

variable "enable_turn_udp" {
  description = "Open TURN/UDP after the separate rollout gate is admitted."
  type        = bool
  default     = false
}

variable "sfu_network_tag" {
  description = "Network tag applied only to LiveKit SFU GKE nodes."
  type        = string
  default     = "livekit-sfu"
}

variable "iap_ssh_target_tags" {
  description = "Optional target tags that may receive SSH from Google IAP."
  type        = set(string)
  default     = []
}

variable "labels" {
  description = "Public-safe labels applied to supported resources."
  type        = map(string)
  default     = {}
}
