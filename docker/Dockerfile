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
COPY templates/ ./templates/
COPY frontend/ ./frontend/

RUN cargo build --release -p aeroflow-cli 2>&1

# Stage 2: Runtime — OpenFOAM + ParaView + VTK + AeroFlow
FROM microfluidica/openfoam:com AS runtime

LABEL maintainer="Jettanakorn Pengsiri"
LABEL description="AeroFlow Agent — Automated CFD Analysis"

USER root

# Install additional dependencies + Python visualization stack
RUN apt-get update && apt-get install -y \
    libvtk9-dev \
    libosmesa6-dev \
    postgresql-client \
    ca-certificates \
    curl \
    python3-pip \
    python3-numpy \
    python3-matplotlib \
    && rm -rf /var/lib/apt/lists/* && \
    pip3 install --break-system-packages vtk --quiet 2>/dev/null || true

# Copy Rust binary from builder
COPY --from=builder /build/target/release/aeroflow /usr/local/bin/aeroflow
COPY --from=builder /build/templates /build/templates
COPY --from=builder /build/frontend /build/frontend

# Copy visualization Python scripts (from repo root, outside builder)
COPY scripts/viz /usr/local/share/aeroflow/scripts/viz
COPY frontend /usr/local/share/aeroflow/frontend

# Create data directories
RUN mkdir -p /data/{cases,import,reports,skills}

# Install OpenFOAM env globally (so docker exec inherits it)
RUN bash -c 'source /usr/lib/openfoam/openfoam2512/etc/bashrc && env | grep -v "^_\|^BASH\|^SHELL\|^HOME\|^PWD\|^USER\|^LOGNAME\|^MAIL\|^TERM\|^HOSTNAME\|^LS_\|^TMPDIR\|^PS1" > /etc/environment' && \
    mkdir -p /etc/profile.d && \
    echo "source /usr/lib/openfoam/openfoam2512/etc/bashrc" > /etc/profile.d/openfoam.sh && \
    chmod +x /etc/profile.d/openfoam.sh

# Add OpenFOAM bin and lib directories to PATH and LD_LIBRARY_PATH globally
ENV PATH="/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/bin:/usr/lib/openfoam/openfoam2512/bin:$PATH" \
    LD_LIBRARY_PATH="/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/lib:/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/lib/sys-openmpi:/usr/lib/x86_64-linux-gnu/openmpi/lib:/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/lib/dummy" \
    WM_PROJECT_VERSION=v2512 \
    FOAM_ETC=/usr/lib/openfoam/openfoam2512/etc \
    FOAM_APPBIN=/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/bin \
    FOAM_LIBBIN=/usr/lib/openfoam/openfoam2512/platforms/linux64GccDPInt32Opt/lib \
    WM_PROJECT_DIR=/usr/lib/openfoam/openfoam2512

# Create entrypoint that sources OpenFOAM then execs command
RUN printf '#!/bin/bash\nsource /usr/lib/openfoam/openfoam2512/etc/bashrc 2>/dev/null || true\nexec "$@"\n' > /usr/local/bin/aeroflow-entrypoint.sh && \
    chmod +x /usr/local/bin/aeroflow-entrypoint.sh

WORKDIR /data

ENTRYPOINT ["/usr/local/bin/aeroflow-entrypoint.sh"]
CMD ["aeroflow", "--help"]
