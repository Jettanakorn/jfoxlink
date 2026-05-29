-- Agent Loop iterations tracking
CREATE TABLE IF NOT EXISTS agent_iterations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    iteration INTEGER NOT NULL,
    manifest JSONB NOT NULL DEFAULT '{}',
    forces JSONB,
    mesh_quality JSONB,
    convergence JSONB,
    score DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(case_id, iteration)
);

-- Agent fixes history
CREATE TABLE IF NOT EXISTS agent_fixes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    iteration INTEGER NOT NULL,
    diagnosis TEXT NOT NULL DEFAULT '',
    fix_action TEXT NOT NULL DEFAULT '',
    details TEXT NOT NULL DEFAULT '',
    manifest JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_iterations_case ON agent_iterations(case_id, iteration);
CREATE INDEX IF NOT EXISTS idx_agent_fixes_case ON agent_fixes(case_id, iteration);
