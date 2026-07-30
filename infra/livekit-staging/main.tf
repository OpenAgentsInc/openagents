locals {
  environment = "staging"
  labels = {
    environment = local.environment
    program     = "ep263-livekit"
    service     = "livekit"
  }
}

module "network" {
  source = "../modules/livekit-network"

  project_id   = var.project_id
  region       = var.region
  environment  = local.environment
  network_name = "oa-livekit-staging"

  node_subnet_cidr = "10.84.0.0/24"
  pod_cidr         = "10.85.0.0/20"
  service_cidr     = "10.86.0.0/24"

  enable_private_service_access = false
  enable_sfu_firewalls          = false
  sfu_network_tag               = "oa-livekit-staging-sfu"
  enable_turn_udp               = false
  labels                        = local.labels
}

module "canary" {
  source = "../modules/livekit-gce-canary"

  project_id    = var.project_id
  region        = var.region
  zone          = var.zone
  name          = "oa-livekit-canary"
  network_id    = module.network.network_id
  subnetwork_id = module.network.subnetwork_id

  enable_instance = var.enable_canary_instance
  expires_at_unix = var.canary_expires_at_unix

  boot_image               = var.canary_boot_image
  livekit_server_image     = var.livekit_server_image
  reverse_proxy_image      = var.reverse_proxy_image
  turn_domain              = var.turn_hostname
  enable_turn_udp          = var.enable_turn_udp
  max_run_duration_seconds = 14400

  labels = local.labels
}

check "turn_udp_rollout" {
  assert {
    condition     = !var.enable_turn_udp
    error_message = "TURN/UDP 443 is deferred. Admit its connectivity gate before changing this staging root."
  }
}
