resource "google_service_account" "this" {
  project      = var.project
  account_id   = var.account_id
  display_name = var.display_name
  description  = var.description
}

# Non-authoritative role grants (google_project_iam_member never removes
# other members from a role).
resource "google_project_iam_member" "roles" {
  for_each = toset(var.project_roles)

  project = var.project
  role    = each.value
  member  = "serviceAccount:${google_service_account.this.email}"
}
