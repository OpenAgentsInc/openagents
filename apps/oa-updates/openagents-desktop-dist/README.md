# OpenAgents Desktop release seed

This directory is intentionally present in the update-service image even when
no Desktop release has been published. A release job may populate it with the
bounded `openagents-desktop-release.json` descriptor and its referenced signed
manifest files. Until then, the Desktop feed routes fail closed with `404`.
