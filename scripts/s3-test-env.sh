#!/usr/bin/env bash
# 启动/停止本地 MinIO 供 S3 集成测试使用。
# 用法: ./scripts/s3-test-env.sh [start|stop]
set -euo pipefail
NAME=club-oa-minio-test
BUCKET=club-oa-test
case "${1:-start}" in
  start)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" -p 59000:9000 \
      -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_PASSWORD=minioadmin \
      quay.io/minio/minio server /data >/dev/null
    for _ in $(seq 1 30); do
      curl -sf http://127.0.0.1:59000/minio/health/live >/dev/null && break
      sleep 1
    done
    docker run --rm --add-host host.docker.internal:host-gateway \
      --entrypoint sh quay.io/minio/mc -c \
      "mc alias set t http://host.docker.internal:59000 minioadmin minioadmin >/dev/null && mc mb -p t/$BUCKET >/dev/null"
    echo "MinIO 就绪: http://127.0.0.1:59000 (bucket: $BUCKET)"
    ;;
  stop)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    echo "MinIO 已停止"
    ;;
  *)
    echo "用法: $0 [start|stop]"; exit 1;;
esac
