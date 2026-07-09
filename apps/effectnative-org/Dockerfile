# Static-file container for https://effectnative.org (Cloud Run service
# `effectnative-org`, project openagentsgemini, us-central1). The site
# artifact in ./site is produced by `bun run site:build` in the public
# OpenAgentsInc/effect-native repo; see scripts/deploy-cloudrun.sh.
FROM nginx:1.27-alpine

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY site/ /usr/share/nginx/html/
