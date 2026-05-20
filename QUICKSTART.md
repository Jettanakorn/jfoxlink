# AeroFlow Agent — Quickstart Guide

> Developer: **Jettanakorn Pengsiri** — JFOX Aircraft Co., Ltd.
> Rust nightly | 14-crate workspace | OpenFOAM + PostgreSQL + Prometheus

---

## 1. Prerequisites

- Docker & Docker Compose
- OpenFOAM (v2312 or later) — optional if using the Docker image
- Rust nightly (for development; the binary is statically linked)

## 2. Getting Started

### Option A — Docker Compose (recommended)

```bash
# Start the full stack (agent + PostgreSQL)
docker compose -f docker/docker-compose.yml up -d

# Verify everything is healthy
docker compose -f docker/docker-compose.yml ps
```

### Option B — Bare Metal

```bash
# Build the binary
cargo build --release

# Initialize workspace and settings
./target/release/aeroflow settings init /path/to/workspace

# Start PostgreSQL (adjust connection string as needed)
# Run database migrations
psql -U aeroflow -d aeroflow -f db/migrations/001_initial_schema.sql
```

### Option C — Kubernetes (production)

```bash
# Install with Helm
helm upgrade --install aeroflow helm/aeroflow -f my-values.yaml
```

## 3. System Health Check

Always start by verifying the system:

```bash
# Full health check
aeroflow doctor

# Check specific category
aeroflow doctor docker
aeroflow doctor database
aeroflow doctor openfoam

# Auto-fix common issues
aeroflow doctor --fix

# Continuous monitoring (30s interval)
aeroflow doctor --watch
```

## 4. Core Workflow: STL → Report

### Step 1 — Ingest a geometry

```bash
# Interactive guided setup
aeroflow init my-wing

# This will:
#   - Prompt for STL file path
#   - Compute 64³ voxel fingerprint + SHA-256 hash
#   - Check database for duplicate geometry
#   - Insert geometry into `geometries` table
#   - Create case record in `cases` table
#   - Copy STL to constant/triSurface/
#   - Write manifest.json
```

### Step 2 — Run the pipeline

```bash
# Run full 8-stage CFD pipeline
aeroflow run cases/my-wing

# Pipeline stages:
#   1. Import   → validate STL, copy to case
#   2. Surface  → surfaceFeatureExtract
#   3. Mesh     → blockMesh + snappyHexMesh (adaptive, up to 3 retries)
#   4. Quality  → checkMesh with aerospace-grade thresholds
#   5. Setup    → controlDict, fvSchemes, fvSolution (binary format)
#   6. Solve    → simpleFoam / rhoCentralFoam (auto-selected by Mach)
#   7. Post     → forces function object, forceCoeffs extraction
#   8. Report   → HTML report with mesh quality, convergence, Cl/Cd/Cm

# With N optimization trials (Bayesian optimization):
aeroflow run cases/my-wing --trials 20
```

### Step 3 — Monitor progress

```bash
# List all cases
aeroflow status

# Launch interactive TUI dashboard
aeroflow tui

# Or via the REST API (see Section 8)
```

### Step 4 — View results

```bash
# Generate HTML report
aeroflow report my-wing

# Opens: reports/my-wing/index.html
```

## 5. Auto-Import Workflow

Watch a directory for new STL files — ideal for batch processing or CI pipelines:

```bash
# Start the file watcher
aeroflow watch /data/import

# When a new .stl file is dropped:
#   - Auto-detects via notify v7
#   - Computes fingerprint
#   - Deduplicates by path + mtime
#   - Inserts geometry + creates case
#   - Creates case directory + manifest
```

## 6. REST API & Web Server

Start the full SaaS server (API + file watcher combined):

```bash
aeroflow serve
# => API on http://0.0.0.0:8080
# => File watcher on /data/import
# => Metrics on  http://0.0.0.0:8080/metrics
```

### API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET    | `/api/health` | No | System health + version |
| GET    | `/metrics` | No | Prometheus metrics |
| POST   | `/api/auth/login` | No | JWT login |
| POST   | `/api/auth/register` | No | User registration |
| GET    | `/api/cases` | JWT | List user's cases |
| POST   | `/api/cases` | JWT | Create new case |
| GET    | `/api/cases/{id}` | JWT | Case detail |
| POST   | `/api/cases/{id}/run` | JWT | Execute pipeline |
| GET    | `/api/users` | Admin | List users |
| GET    | `/api/events` | JWT | SSE event stream |

### Example API Usage

```bash
# Login
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@aeroflow.io","password":"changeme"}' | jq -r '.token')

# Create a case
curl -X POST http://localhost:8080/api/cases \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"api-wing","geometry_id":"...","solver":"simpleFoam","flow_type":"subsonic"}'

# Stream live events
curl -N http://localhost:8080/api/events \
  -H "Authorization: Bearer $TOKEN"
```

## 7. User Management

```bash
# Create a user (interactive)
aeroflow user create

# List users
aeroflow user list

# Authenticate
aeroflow user login admin@aeroflow.io
```

## 8. Skills & Optimization

The skills database learns the best CFD parameters for each (geometry, flow regime) combination using Gaussian Process regression.

### Commands

```bash
# List all skills in the database
aeroflow skills list

# Show a specific skill's details
aeroflow skills show my-wing-subsonic

# Run Bayesian optimization (10 trials by default)
aeroflow skills optimize my-wing-subsonic --trials 20

# Export a skill (JSON) for sharing
aeroflow skills export my-wing-subsonic --format json

# Import a skill
aeroflow skills import ./shared-skill.json

# Reset a skill (start learning from scratch)
aeroflow skills reset my-wing-subsonic
```

### What Optimization Does

```
For each trial:
  1. GP suggests next parameters via Expected Improvement
  2. Pipeline runs with those parameters
  3. Reward computed: w₁·Clₑᵣᵣ + w₂·Cdₑₓ + w₃·y⁺ + w₄·residual + w₅·mesh quality
  4. GP updated with new observation
  5. Repeat until budget exhausted
```

## 9. Settings

```bash
# View current configuration
aeroflow settings show

# Initialize workspace at a custom path
aeroflow settings init /data/aeroflow-workspace

# Override a setting (also via AEROFLOW_* env vars)
aeroflow settings set max_concurrent_cases 8
aeroflow settings set workspace_dir /mnt/nvme/cases

# Show effective config file path
aeroflow settings path
```

Settings are read from: `$WORKSPACE/settings/aeroflow-settings.toml` → `~/.config/aeroflow/settings.toml` → env vars

## 10. Production Deployment

### Docker Compose (single-node)

```yaml
# docker-compose.yml (already provided)
services:
  aeroflow:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - /mnt/data/workspace:/workspace
    environment:
      - AEROFLOW_DATABASE_URL=postgres://aeroflow:pass@postgres:5432/aeroflow
  postgres:
    image: postgres:16-alpine
    volumes:
      - pgdata:/var/lib/postgresql/data
```

### Kubernetes (production SaaS)

```bash
# Deploy with Helm
helm upgrade --install aeroflow helm/aeroflow \
  --set postgresql.auth.password=secure-pass \
  --set aeroflow.jwtSecret=your-jwt-secret \
  --set ingress.enabled=true \
  --set ingress.hosts[0].host=aeroflow.example.com

# Scale horizontally
kubectl scale deployment aeroflow --replicas=3

# Monitor with Prometheus + Grafana
# ServiceMonitor auto-registers with Prometheus Operator
```

### Prometheus Metrics

Available at `http://localhost:8080/metrics`:

| Metric | Type | Description |
|--------|------|-------------|
| `aeroflow_cases_created_total` | Counter | Cases created |
| `aeroflow_cases_completed_total` | Counter | Cases completed |
| `aeroflow_cases_failed_total` | Counter | Failed cases |
| `aeroflow_cases_active` | Gauge | Currently running |
| `aeroflow_queue_depth` | Gauge | Queued cases |
| `aeroflow_pipeline_duration_seconds` | Histogram | Pipeline runtime |
| `aeroflow_solver_iterations` | Histogram | Iterations per case |
| `aeroflow_http_request_duration_seconds` | Histogram | API latency |
| `aeroflow_db_query_duration_seconds` | Histogram | DB query latency |
| `aeroflow_mesh_quality_failures_total` | Counter | Mesh failures |
| `aeroflow_skill_trials_total` | Counter | GP trials run |

### Structured Logging

```bash
# JSON output for log aggregation (Loki, ELK, etc.)
aeroflow serve --json-logs

# With log level control
RUST_LOG=aeroflow=debug aeroflow serve --json-logs
```

## 11. End-to-End Example

```bash
# 1. Verify system
aeroflow doctor

# 2. Initialize workspace
aeroflow settings init ~/aeroflow-workspace

# 3. Create admin user
aeroflow user create
# => Email: admin@aeroflow.io
# => Password: ********
# => Role: admin

# 4. Import wing geometry and create case
aeroflow init naca2412-wing
# => Enter path to STL: /data/stl/naca2412.stl
# => Geometry fingerprinted (64³ voxels)
# => Case created: id = abc-123

# 5. Run the pipeline with 5 optimization trials
aeroflow run ~/aeroflow-workspace/cases/naca2412-wing --trials 5

# 6. Monitor via TUI
aeroflow tui

# 7. Generate and view report
aeroflow report naca2412-wing
# open ~/aeroflow-workspace/reports/naca2412-wing/index.html

# 8. Check the learned skill
aeroflow skills list
aeroflow skills show naca2412-subsonic

# 9. Deploy to production
helm upgrade --install aeroflow helm/aeroflow \
  --set postgresql.auth.password=...
```

---

## Quick Reference

```text
aeroflow                    # CLI tool
├── init [name]             # Start a case
├── run <case> [--trials N]  # Execute pipeline
├── status                  # List cases
├── report <case>           # Generate report
├── watch [path]            # Auto-import STL files
├── serve [port]            # REST API + watcher
├── doctor [category]       # Health checks
│   ├── --fix               # Auto-remediate
│   ├── --json              # JSON output
│   └── --watch             # Continuous monitoring
├── skills                  # Skills management
│   ├── list                # List skills
│   ├── show <name>         # Skill details
│   ├── optimize <name>     # Bayesian optimization
│   ├── export <name>       # Export skill
│   ├── import <path>       # Import skill
│   └── reset <name>        # Reset skill
├── user                    # User management
│   ├── create              # Create user
│   ├── list                # List users
│   ├── show <email>        # User details
│   ├── update <email>      # Update user
│   ├── delete <email>      # Delete user
│   └── login <email>       # Authenticate
├── settings                # Configuration
│   ├── show                # Current settings
│   ├── set <key>=<val>     # Override setting
│   ├── init [path]         # Initialize workspace
│   ├── reset               # Reset to defaults
│   └── path                # Config file location
├── tui                     # Interactive dashboard
└── --json-logs             # Structured JSON logs
```
