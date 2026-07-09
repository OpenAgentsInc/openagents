# Static-file container for the Effect Native component gallery (Cloud Run
# service `effect-native-gallery`, project openagentsgemini, us-central1).
# The artifact in ./gallery is produced by `bun run gallery:build` in the
# public OpenAgentsInc/effect-native repo; see scripts/deploy-cloudrun.sh.
FROM nginx:1.27-alpine

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY gallery/ /usr/share/nginx/html/
