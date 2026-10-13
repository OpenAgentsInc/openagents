#!/usr/bin/env bash
# Run a command on a subset of this host's TPU chips. usage: pin.sh "0,1,2,3" "2,2,1" PORT cmd...
CH=$1 B=$2 PORT=$3; shift 3
export TPU_VISIBLE_CHIPS=$CH TPU_CHIPS_PER_PROCESS_BOUNDS=$B TPU_PROCESS_BOUNDS=1,1,1 \
  TPU_PROCESS_PORT=$PORT TPU_PROCESS_ADDRESSES=localhost:$PORT CLEF_CHIPS=$CH
exec "$@"
