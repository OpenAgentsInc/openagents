#!/usr/bin/env bash

STATE=$(docker container ls --filter name=boltz-scripts --format '{{.State}}')

if [ "$STATE" != "running" ] ;
then
set -xe
docker compose down --volumes
docker compose up --remove-orphans -d
fi
