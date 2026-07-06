resource "google_storage_bucket" "this" {
  project  = var.project
  name     = var.name
  location = var.location

  storage_class               = var.storage_class
  uniform_bucket_level_access = true
  force_destroy               = false

  # Only emit the block when versioning is on: existing non-versioned buckets
  # have no versioning block in state, and adding `enabled = false` would
  # show up as a permanent (harmless but noisy) plan diff.
  dynamic "versioning" {
    for_each = var.versioning ? [true] : []
    content {
      enabled = true
    }
  }

  soft_delete_policy {
    retention_duration_seconds = var.soft_delete_retention_seconds
  }

  dynamic "lifecycle_rule" {
    for_each = var.lifecycle_rules
    content {
      action {
        type          = lifecycle_rule.value.action_type
        storage_class = lifecycle_rule.value.action_storage_class
      }
      condition {
        age                = lifecycle_rule.value.age
        num_newer_versions = lifecycle_rule.value.num_newer_versions
        with_state         = lifecycle_rule.value.with_state
      }
    }
  }
}
