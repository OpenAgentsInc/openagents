variable "project_id" {
  type = string
}

variable "project_number" {
  type = string
}

variable "region" {
  type = string
}

variable "cluster_id" {
  type = string
}

variable "managed_secret_ids" {
  type = map(string)
}

variable "deployment_executor_image" {
  type = string
}

variable "labels" {
  type = map(string)
}
