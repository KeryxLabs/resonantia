# Binary is pre-built on the host by build-image.sh before docker build runs.
# Build context: gateway/ (the directory containing this file)
FROM ubuntu:24.04 AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY publish/ ./

RUN chmod +x /app/resonantia-gateway \
    && mkdir -p /data/gateway

ENV RESONANTIA_GATEWAY_BIND=0.0.0.0:8090
ENV RESONANTIA_GATEWAY_DATA_DIR=/data/gateway
ENV HOME=/data

VOLUME ["/data/gateway"]

EXPOSE 8090

ENTRYPOINT ["/app/resonantia-gateway"]
