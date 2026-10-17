# Official nginx stable-alpine index resolved on 2026-10-17.
FROM nginx@sha256:970364228b1e4f932abc22d321932816782cca8a20b85a77552a7cfe9acfb3b5
COPY cloudrun.conf /etc/nginx/templates/default.conf.template
COPY ca.crt /etc/space/ca.crt
ENV NGINX_ENVSUBST_FILTER=SPACE_EDGE_PORT
ARG OA_COMMIT
LABEL com.openagents.source="${OA_COMMIT}" com.openagents.issue="11814"
