# Private qualification transport. The build context supplies only this file,
# canary-edge.conf, and its non-secret, explicitly generated CA certificate.
FROM nginx@sha256:970364228b1e4f932abc22d321932816782cca8a20b85a77552a7cfe9acfb3b5
COPY canary-edge.conf /etc/nginx/templates/default.conf.template
COPY ca.crt /etc/space/ca.crt
ENV NGINX_ENVSUBST_FILTER=SPACE_CANARY_
