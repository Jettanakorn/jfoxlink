# Stage 1: Build Rust binary
FROM rust:latest AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y \
    libvtk9-dev \
    libosmesa6-dev \
    cmake \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
COPY crates/ ./crates/
COPY db/ ./db/

RUN cargo build --release -p aeroflow-cli 2>&1 || true

# Stage 2: Runtime — OpenFOAM + ParaView + VTK + AeroFlow
FROM microfluidica/openfoam:com AS runtime

LABEL maintainer="JFOX Aircraft Co., Ltd."
LABEL description="AeroFlow Agent — Automated CFD Analysis"

USER root

# Install additional dependencies
RUN apt-get update && apt-get install -y \
    libvtk9-dev \
    libosmesa6-dev \
    postgresql-client \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust binary from builder
COPY --from=builder /build/target/release/aeroflow /usr/local/bin/aeroflow

# Create data directories
RUN mkdir -p /data/{cases,import,reports,skills}

# Source OpenFOAM environment at startup
RUN echo "source /opt/openfoam*/etc/bashrc 2>/dev/null || true" >> /root/.bashrc

WORKDIR /data

ENTRYPOINT ["aeroflow"]
CMD ["--help"]
