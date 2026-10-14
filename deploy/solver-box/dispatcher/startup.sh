#!/bin/sh
# The dispatcher host's GCE startup script (#11559): on every boot, installs
# the dispatcher of the commit named by the instance metadata attribute
# `oa-solver-box-commit` from gs://openagentsgemini-coder-artifacts/oa-solver-box/COMMIT/
# (built by cloudbuild.yaml here) and (re)starts oa-solver-dispatch.service.
# To move the host to another commit: build it, set the attribute, and
# rerun this script (`sudo google_metadata_script_runner startup`) or reboot.
set -eu
md() {
    curl -fsS -H 'Metadata-Flavor: Google' \
        "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1"
}
COMMIT=$(md oa-solver-box-commit)
SRC=gs://openagentsgemini-coder-artifacts/oa-solver-box/$COMMIT
mkdir -p /opt/oa-solver/new
gcloud storage cp "$SRC/oa-solver-box" "$SRC/oa-solver-dispatch-start" \
    "$SRC/oa-solver-dispatch.service" /opt/oa-solver/new/ --quiet
chmod 0755 /opt/oa-solver/new/oa-solver-box /opt/oa-solver/new/oa-solver-dispatch-start
mv /opt/oa-solver/new/oa-solver-box /opt/oa-solver/new/oa-solver-dispatch-start /opt/oa-solver/
mv /opt/oa-solver/new/oa-solver-dispatch.service /etc/systemd/system/
echo "$COMMIT" > /opt/oa-solver/COMMIT
systemctl daemon-reload
systemctl enable oa-solver-dispatch.service
systemctl restart oa-solver-dispatch.service
