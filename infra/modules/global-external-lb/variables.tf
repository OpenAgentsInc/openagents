variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "name" {
  description = "Prefix for every LB resource name"
  type        = string
}

variable "region" {
  description = "Region of the Cloud Run service behind the serverless NEG"
  type        = string
}

variable "domains" {
  description = "Hostnames served by this LB (cert SANs + URL-map host rule)"
  type        = list(string)
}

variable "cloud_run_service" {
  description = "Cloud Run service name the serverless NEG points at"
  type        = string
}

variable "monolith_hosts" {
  description = "Hostnames that route every path to the monolith"
  type        = list(string)
}

variable "components_host" {
  description = "Hostname for the separately managed Effect Native component gallery"
  type        = string
}

variable "components_backend_service" {
  description = "Existing global backend service name for the Effect Native component gallery"
  type        = string
}
