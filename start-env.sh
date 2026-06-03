#!/usr/bin/env bash
set -euo pipefail

NETWORK="dockerman-test-net"
LABEL="dockerman.test=true"

case "${1:-}" in
  setup)
    docker network inspect "$NETWORK" >/dev/null 2>&1 || docker network create "$NETWORK"
    docker volume inspect dockerman-data >/dev/null 2>&1 || docker volume create dockerman-data

    docker run -d --restart no --name dockerman-nginx --label "$LABEL" --network "$NETWORK" -p 8080:80 nginx:latest
    docker run -d --restart no --name dockerman-redis --label "$LABEL" --network "$NETWORK" redis:latest
    docker run -d --restart no --name dockerman-alpine --label "$LABEL" --network "$NETWORK" alpine:latest sleep infinity

    docker run -d --restart no --name dockerman-stopped --label "$LABEL" alpine:latest sleep infinity
    docker stop dockerman-stopped

    docker run -d --restart no --name dockerman-volume --label "$LABEL" --network "$NETWORK" \
      -v dockerman-data:/data \
      alpine:latest sleep infinity

    docker run -d --restart no --name dockerman-env --label "$LABEL" --network "$NETWORK" \
      -e APP_ENV=development \
      -e VERSION=1.0 \
      alpine:latest sleep infinity
    ;;

  start)
    docker start $(docker ps -aq --filter "label=$LABEL")
    ;;

  stop)
    docker stop $(docker ps -aq --filter "label=$LABEL")
    ;;

  restart)
    docker restart $(docker ps -aq --filter "label=$LABEL")
    ;;

  status)
    docker ps -a --filter "label=$LABEL"
    ;;

  clean)
    docker rm -f $(docker ps -aq --filter "label=$LABEL") 2>/dev/null || true
    docker network rm "$NETWORK" 2>/dev/null || true
    docker volume rm dockerman-data 2>/dev/null || true
    ;;

  *)
    echo "Usage: $0 {setup|start|stop|restart|status|clean}"
    exit 1
    ;;
esac
