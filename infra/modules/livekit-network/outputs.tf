output "network_id" {
  description = "Dedicated LiveKit VPC resource ID."
  value       = google_compute_network.livekit.id
}

output "network_name" {
  description = "Dedicated LiveKit VPC name."
  value       = google_compute_network.livekit.name
}

output "subnetwork_id" {
  description = "LiveKit GKE node subnetwork resource ID."
  value       = google_compute_subnetwork.nodes.id
}

output "subnetwork_name" {
  description = "LiveKit GKE node subnetwork name."
  value       = google_compute_subnetwork.nodes.name
}

output "pod_range_name" {
  description = "Secondary range name allocated to GKE pods."
  value       = google_compute_subnetwork.nodes.secondary_ip_range[0].range_name
}

output "service_range_name" {
  description = "Secondary range name allocated to GKE services."
  value       = google_compute_subnetwork.nodes.secondary_ip_range[1].range_name
}

output "service_networking_connection" {
  description = "Private Service Access connection dependency."
  value       = try(google_service_networking_connection.service_networking[0].id, null)
}
