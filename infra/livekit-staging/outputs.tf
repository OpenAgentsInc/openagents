output "canary_instance_name" {
  description = "Exact disposable canary VM name, or null before the second-phase apply."
  value       = module.canary.instance_name
}

output "canary_external_ip" {
  description = "Reserved public address for both canary DNS names."
  value       = module.canary.external_ip
}

output "signal_hostname" {
  description = "Trusted canary WSS hostname."
  value       = var.signal_hostname
}

output "turn_hostname" {
  description = "Trusted canary TURN/TLS hostname."
  value       = var.turn_hostname
}

output "canary_service_account_email" {
  description = "Canary metadata identity."
  value       = module.canary.service_account_email
}

output "canary_secret_ids" {
  description = "Canary Secret Manager containers. Operators add versions before enabling the VM."
  value       = module.canary.secret_ids
}

output "canary_boot_image" {
  description = "Immutable COS image pin."
  value       = var.canary_boot_image
}

output "livekit_server_image" {
  description = "Released LiveKit server manifest pin."
  value       = var.livekit_server_image
}

output "reverse_proxy_image" {
  description = "Caddy manifest pin."
  value       = var.reverse_proxy_image
}
