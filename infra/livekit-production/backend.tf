terraform {
  backend "gcs" {
    bucket = "openagentsgemini-terraform-state"
    prefix = "livekit/production"
  }
}
