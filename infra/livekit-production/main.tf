locals {
  environment = "production"
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
  network_name = "oa-livekit-prod"

  node_subnet_cidr = "10.80.0.0/20"
  pod_cidr         = "10.81.0.0/16"
  service_cidr     = "10.82.0.0/20"

  sfu_network_tag = "oa-livekit-prod-sfu"
  enable_turn_udp = var.enable_turn_udp
  labels          = local.labels
}

module "platform" {
  source = "../modules/livekit-gke"

  project_id   = var.project_id
  region       = var.region
  zones        = var.zones
  environment  = local.environment
  cluster_name = "oa-livekit-prod"

  network_id         = module.network.network_id
  subnetwork_id      = module.network.subnetwork_id
  pod_range_name     = module.network.pod_range_name
  service_range_name = module.network.service_range_name

  master_ipv4_cidr_block     = "172.16.0.0/28"
  master_authorized_networks = var.master_authorized_networks
  deletion_protection        = true

  sfu_machine_type = "c2-standard-8"
  sfu_min_nodes    = 3
  sfu_max_nodes    = 7
  sfu_network_tag  = "oa-livekit-prod-sfu"

  app_machine_type = "e2-standard-8"
  app_min_nodes    = 3
  app_max_nodes    = 4

  redis_name           = "oa-livekit-redis"
  redis_memory_size_gb = 5
  redis_version        = "REDIS_7_2"

  namespace                              = "livekit-system"
  node_service_account_id                = "oa-livekit-prod-nodes"
  server_service_account_id              = "oa-livekit-server"
  agent_service_account_id               = "oa-livekit-agent"
  secret_reader_service_account_id       = "livekit-secret-reader"
  dns_secret_reader_service_account_id   = "oa-livekit-cert-manager-reader"
  sarah_secret_reader_service_account_id = "oa-livekit-sarah-secret-reader"

  labels = local.labels

  depends_on = [module.network]
}

module "observability" {
  source = "../modules/livekit-observability"

  project_id        = var.project_id
  project_number    = var.project_number
  environment       = local.environment
  cluster_name      = module.platform.cluster_name
  cluster_location  = module.platform.cluster_location
  redis_instance_id = "oa-livekit-redis"
  signal_hostname   = var.signal_hostname

  notification_channel_ids = var.notification_channel_ids
  billing_account_id       = var.billing_account_id
  monthly_budget_usd       = var.monthly_budget_usd
  max_rooms                = 20
  max_participants         = 60
  labels                   = local.labels
}

check "turn_udp_rollout" {
  assert {
    condition     = !var.enable_turn_udp
    error_message = "TURN/UDP 443 is deferred. Admit its connectivity gate before changing this production root."
  }
}
