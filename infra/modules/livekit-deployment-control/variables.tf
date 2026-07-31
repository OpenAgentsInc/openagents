variable "project_id" {
  type = string
}

variable "project_number" {
  type = string
}

variable "organization_id" {
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

variable "github_app_installation_id" {
  type = number

  validation {
    condition = (
      floor(var.github_app_installation_id) == var.github_app_installation_id &&
      var.github_app_installation_id > 0
    )
    error_message = "The Cloud Build GitHub App installation ID must be a positive integer."
  }
}

variable "github_authorizer_token_secret_version" {
  type = string

  validation {
    condition = can(regex(
      "^projects/[a-z][a-z0-9-]{4,28}[a-z0-9]/secrets/[A-Za-z0-9_-]{1,255}/versions/[1-9][0-9]*$",
      var.github_authorizer_token_secret_version,
    ))
    error_message = "The GitHub authorizer token must use an immutable numbered Secret Manager version."
  }
}

variable "labels" {
  type = map(string)
}
