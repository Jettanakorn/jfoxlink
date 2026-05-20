-- AeroFlow Agent — Initial Database Schema
-- Developer: Jettanakorn Pengsiri by JFOX Aircraft Co., Ltd.

-- Enable UUID generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Users (for SaaS multi-tenant future)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    role TEXT NOT NULL DEFAULT 'engineer',
    active BOOLEAN DEFAULT true,
    last_login TIMESTAMPTZ,
    quota_max_concurrent INT DEFAULT 2,
    quota_max_cores INT DEFAULT 4,
    quota_max_memory_gb INT DEFAULT 8,
    preferences JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Geometries: deduplicated STL fingerprints
CREATE TABLE IF NOT EXISTS geometries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sha256_hash BYTEA UNIQUE NOT NULL,
    voxel_hash_8 BYTEA NOT NULL,
    voxel_hash_32 BYTEA NOT NULL,
    voxel_hash_64 BYTEA NOT NULL,
    voxel_data BYTEA,
    bounding_box JSONB NOT NULL,
    surface_area DOUBLE PRECISION,
    volume DOUBLE PRECISION,
    aspect_ratio DOUBLE PRECISION,
    num_triangles INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Skills: learned parameter sets
CREATE TABLE IF NOT EXISTS skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    geometry_id UUID REFERENCES geometries(id) NOT NULL,
    flow_regime_key TEXT NOT NULL,
    parameters JSONB,
    reward_score DOUBLE PRECISION DEFAULT 0.0,
    confidence DOUBLE PRECISION DEFAULT 0.0,
    n_trials INTEGER DEFAULT 0,
    version INTEGER DEFAULT 1,
    gp_model BYTEA,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (geometry_id, flow_regime_key, version)
);

-- Parameter trials: every attempted configuration
CREATE TABLE IF NOT EXISTS parameter_trials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    skill_id UUID REFERENCES skills(id) NOT NULL,
    case_id UUID,
    parameters JSONB NOT NULL,
    reward DOUBLE PRECISION NOT NULL,
    converged BOOLEAN NOT NULL,
    mesh_quality JSONB,
    forces JSONB,
    runtime_s DOUBLE PRECISION,
    peak_memory_gb DOUBLE PRECISION,
    residual_history JSONB,
    deprecated BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Cases
CREATE TABLE IF NOT EXISTS cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    name TEXT NOT NULL,
    geometry_id UUID REFERENCES geometries(id),
    skill_id UUID REFERENCES skills(id),
    status TEXT NOT NULL DEFAULT 'created',
    flow_type TEXT,
    solver TEXT,
    mesh_stats JSONB,
    results JSONB,
    resource_usage JSONB,
    report_path TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- Events
CREATE TABLE IF NOT EXISTS events (
    id BIGSERIAL PRIMARY KEY,
    case_id UUID REFERENCES cases(id),
    skill_id UUID REFERENCES skills(id),
    stage TEXT NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    timestamp TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_skills_geom_regime ON skills(geometry_id, flow_regime_key);
CREATE INDEX IF NOT EXISTS idx_skills_confidence ON skills(confidence DESC);
CREATE INDEX IF NOT EXISTS idx_trials_skill_reward ON parameter_trials(skill_id, reward DESC);
CREATE INDEX IF NOT EXISTS idx_geoms_voxel_hash ON geometries(voxel_hash_8, voxel_hash_32);
CREATE INDEX IF NOT EXISTS idx_cases_user ON cases(user_id);
CREATE INDEX IF NOT EXISTS idx_cases_status ON cases(status);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO schema_migrations (version) VALUES ('001');
