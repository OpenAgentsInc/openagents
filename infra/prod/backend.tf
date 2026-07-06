# Remote state lives in a versioned GCS bucket in the same project.
# The bucket itself is created once out of band (see infra/README.md) and
# then imported into this state (module.terraform_state_bucket).
terraform {
  backend "gcs" {
    bucket = "openagentsgemini-terraform-state"
    prefix = "prod"
  }
}
