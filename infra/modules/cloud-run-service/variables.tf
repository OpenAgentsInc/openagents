variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "name" {
  description = "Cloud Run service name"
  type        = string
}

variable "region" {
  description = "Cloud Run region"
  type        = string
}

variable "ingress" {
  description = "Ingress setting"
  type        = string
  default     = "INGRESS_TRAFFIC_ALL"
}

variable "placeholder_image" {
  description = "Image used only on first create; live revisions are deployed via gcloud/CI and ignored by Terraform"
  type        = string
  default     = "us-docker.pkg.dev/cloudrun/container/hello"
}
