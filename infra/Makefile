# Terraform/OpenTofu baseline for the OpenAgents GCP footprint (issue #8527).
# `make plan` is read-only and always safe. Apply is intentionally not a
# target here — see README.md (plan-on-PR / apply-on-main intent).

TF ?= tofu
ENV_DIR ?= prod
# The GCS backend + google provider use ADC; fall back to a gcloud CLI token
# so a stale ADC grant does not block read-only plans.
TOKEN = GOOGLE_OAUTH_ACCESS_TOKEN=$$(gcloud auth print-access-token)

.PHONY: init plan validate fmt

init:
	cd $(ENV_DIR) && $(TOKEN) $(TF) init

plan:
	cd $(ENV_DIR) && $(TOKEN) $(TF) plan

validate:
	$(TF) fmt -check -recursive
	cd $(ENV_DIR) && $(TOKEN) $(TF) validate

fmt:
	$(TF) fmt -recursive
