use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Ollama,
    Anthropic,
    Gemini,
}

// ── STAR Agent Loop Types ──

/// Structured intake configuration (equivalent to agent-intake.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeConfig {
    pub goal: String,
    pub geometry_type: String,
    pub flow_regime: FlowRegime,
    pub target_metrics: TargetMetrics,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRegime {
    pub mach: f64,
    pub reynolds: f64,
    pub flow_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetMetrics {
    pub target_cl: Option<f64>,
    pub target_cd: Option<f64>,
    pub max_y_plus: Option<f64>,
    pub min_convergence: Option<f64>,
}

/// Agent-manifest.json: proposed configuration for one iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub iteration: u32,
    pub mesh_params: serde_json::Value,
    pub solver_params: serde_json::Value,
    pub bc_params: serde_json::Value,
    pub reasoning: String,
}

/// Forces computed from the solver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forces {
    pub cd: f64,
    pub cl: f64,
    pub cm: Option<f64>,
}

/// Mesh quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshQuality {
    pub max_y_plus: f64,
    pub max_non_orthogonality: f64,
    pub max_skewness: f64,
    pub n_cells: u64,
}

/// Convergence metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convergence {
    pub final_residual: f64,
    pub n_iterations: u32,
    pub converged: bool,
}

/// Result of a single agent iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIterationResult {
    pub iteration: u32,
    pub manifest: AgentManifest,
    pub status: String,
    pub forces: Option<Forces>,
    pub mesh_quality: Option<MeshQuality>,
    pub convergence: Option<Convergence>,
    pub score: Option<f64>,
    pub runtime_s: Option<f64>,
    pub fix_applied: Option<String>,
}

/// Scored iteration for comparison engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredIteration {
    pub iteration: u32,
    pub score: f64,
    pub cd: Option<f64>,
    pub cl: Option<f64>,
    pub max_y_plus: Option<f64>,
    pub converged: bool,
    pub reasoning: String,
}

/// Composite scoring weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub w_cl_error: f64,
    pub w_cd_excess: f64,
    pub w_y_plus_penalty: f64,
    pub w_residual_penalty: f64,
    pub w_mesh_quality: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            w_cl_error: 1.0,
            w_cd_excess: 2.0,
            w_y_plus_penalty: 0.5,
            w_residual_penalty: 0.3,
            w_mesh_quality: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_results: Vec<ToolResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub case_id: Uuid,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub case_id: Uuid,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantResponse {
    pub message: Message,
    pub conversation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    Token { content: String },
    ToolCall { tool: String, args: serde_json::Value, id: String },
    ToolResult { tool: String, result: serde_json::Value },
    Done { message_id: String },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: Option<String>,
    pub tool_call: Option<(String, String, serde_json::Value)>, // (id, name, args)
    pub done: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn utc_dt(s: &str) -> DateTime<Utc> {
        s.parse::<DateTime<Utc>>().unwrap()
    }

    fn make_conversation() -> Conversation {
        Conversation {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            case_id: Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap(),
            model: "gpt-4".into(),
            created_at: utc_dt("2024-01-01T00:00:00Z"),
            updated_at: utc_dt("2024-01-02T00:00:00Z"),
        }
    }

    #[test]
    fn test_role_serde_and_partial_eq() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), r#""user""#);
        assert_eq!(serde_json::to_string(&Role::Assistant).unwrap(), r#""assistant""#);
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), r#""system""#);
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), r#""tool""#);
        assert_eq!(Role::User, Role::User);
        assert_ne!(Role::User, Role::Assistant);
        for role in &[Role::User, Role::Assistant, Role::System, Role::Tool] {
            let json = serde_json::to_string(role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(*role, back);
        }
    }

    #[test]
    fn test_message_round_trip() {
        let msg = Message {
            role: Role::Assistant,
            content: "What are the forces?".into(),
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "evaluate_results".into(),
                arguments: json!({"case_id": "abc"}),
                result: None,
            }],
            tool_results: vec![ToolResult {
                tool_call_id: "call_1".into(),
                name: "evaluate_results".into(),
                content: "Cd=0.05".into(),
            }],
        };
        let json = serde_json::to_string_pretty(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.role, back.role);
        assert_eq!(msg.content, back.content);
        assert_eq!(msg.tool_calls.len(), back.tool_calls.len());
        assert_eq!(msg.tool_results.len(), back.tool_results.len());
        let empty = Message {
            role: Role::User,
            content: "hi".into(),
            tool_calls: vec![],
            tool_results: vec![],
        };
        let j = serde_json::to_string(&empty).unwrap();
        assert!(!j.contains("tool_calls"));
        let back: Message = serde_json::from_str(&j).unwrap();
        assert_eq!(empty.role, back.role);
        assert_eq!(empty.content, back.content);
        assert!(back.tool_calls.is_empty());
    }

    #[test]
    fn test_tool_def_round_trip() {
        let td = ToolDef {
            name: "test_tool".into(),
            description: "A test tool".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_string_pretty(&td).unwrap();
        let back: ToolDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test_tool");
        assert_eq!(back.description, "A test tool");
        assert_eq!(back.input_schema["type"], "object");
    }

    #[test]
    fn test_tool_call_round_trip() {
        let tc = ToolCall {
            id: "call_42".into(),
            name: "run_simulation".into(),
            arguments: json!({"case_id": "uuid", "iteration": 3}),
            result: Some(json!({"completed": true})),
        };
        let json = serde_json::to_string_pretty(&tc).unwrap();
        let back: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(tc.id, back.id);
        assert_eq!(tc.name, back.name);
        assert_eq!(tc.result, back.result);
        let tc2 = ToolCall {
            result: None,
            ..tc
        };
        let json2 = serde_json::to_string_pretty(&tc2).unwrap();
        assert!(!json2.contains("result"));
        let back2: ToolCall = serde_json::from_str(&json2).unwrap();
        assert_eq!(tc2.id, back2.id);
        assert_eq!(tc2.name, back2.name);
        assert!(back2.result.is_none());
    }

    #[test]
    fn test_tool_result_round_trip() {
        let tr = ToolResult {
            tool_call_id: "call_1".into(),
            name: "evaluate_results".into(),
            content: "success".into(),
        };
        let json = serde_json::to_string(&tr).unwrap();
        let back: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(tr.tool_call_id, back.tool_call_id);
        assert_eq!(tr.name, back.name);
        assert_eq!(tr.content, back.content);
    }

    #[test]
    fn test_conversation_round_trip() {
        let conv = make_conversation();
        let json = serde_json::to_string_pretty(&conv).unwrap();
        let back: Conversation = serde_json::from_str(&json).unwrap();
        assert_eq!(conv.id, back.id);
        assert_eq!(conv.case_id, back.case_id);
        assert_eq!(conv.model, back.model);
    }

    #[test]
    fn test_chat_request_round_trip() {
        let cr = ChatRequest {
            case_id: Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap(),
            message: "Run simulation".into(),
            conversation_id: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
            model: Some("gpt-4".into()),
        };
        let json = serde_json::to_string_pretty(&cr).unwrap();
        let back: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(cr.case_id, back.case_id);
        assert_eq!(cr.conversation_id, back.conversation_id);
        assert_eq!(cr.model, back.model);
        let cr2 = ChatRequest {
            case_id: Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap(),
            message: "Hi".into(),
            conversation_id: None,
            model: None,
        };
        let json2 = serde_json::to_string(&cr2).unwrap();
        assert!(!json2.contains("conversation_id"));
        assert!(!json2.contains("model"));
        let back2: ChatRequest = serde_json::from_str(&json2).unwrap();
        assert_eq!(cr2.case_id, back2.case_id);
        assert!(back2.conversation_id.is_none());
    }

    #[test]
    fn test_intake_config_round_trip() {
        let ic = IntakeConfig {
            goal: "Optimize drag".into(),
            geometry_type: "airfoil".into(),
            flow_regime: FlowRegime {
                mach: 0.3,
                reynolds: 5e6,
                flow_type: "subsonic".into(),
            },
            target_metrics: TargetMetrics {
                target_cl: Some(0.5),
                target_cd: Some(0.02),
                max_y_plus: Some(1.0),
                min_convergence: Some(1e-5),
            },
            constraints: vec!["max_cells=10M".into(), "time_budget=24h".into()],
        };
        let json = serde_json::to_string_pretty(&ic).unwrap();
        let back: IntakeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(ic.goal, back.goal);
        assert_eq!(ic.flow_regime.mach, back.flow_regime.mach);
        assert_eq!(ic.target_metrics.target_cl, back.target_metrics.target_cl);
        let ic2 = IntakeConfig {
            target_metrics: TargetMetrics {
                target_cl: None,
                target_cd: None,
                max_y_plus: None,
                min_convergence: None,
            },
            constraints: vec![],
            ..ic
        };
        let json2 = serde_json::to_string(&ic2).unwrap();
        let back2: IntakeConfig = serde_json::from_str(&json2).unwrap();
        assert!(back2.target_metrics.target_cl.is_none());
        assert!(back2.constraints.is_empty());
    }

    #[test]
    fn test_flow_regime_round_trip() {
        let fr = FlowRegime {
            mach: 2.0,
            reynolds: 1e7,
            flow_type: "supersonic".into(),
        };
        let json = serde_json::to_string(&fr).unwrap();
        let back: FlowRegime = serde_json::from_str(&json).unwrap();
        assert!((back.mach - fr.mach).abs() < 1e-12);
        assert!((back.reynolds - fr.reynolds).abs() < 1e-12);
        assert_eq!(back.flow_type, fr.flow_type);
        let zero = FlowRegime {
            mach: 0.0,
            reynolds: 0.0,
            flow_type: "".into(),
        };
        let j = serde_json::to_string(&zero).unwrap();
        let b: FlowRegime = serde_json::from_str(&j).unwrap();
        assert_eq!(b.mach, 0.0);
    }

    #[test]
    fn test_target_metrics_round_trip() {
        let tm = TargetMetrics {
            target_cl: Some(0.5),
            target_cd: Some(0.02),
            max_y_plus: Some(1.0),
            min_convergence: Some(1e-5),
        };
        let json = serde_json::to_string(&tm).unwrap();
        let back: TargetMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(tm.target_cl, back.target_cl);
        let none = TargetMetrics {
            target_cl: None,
            target_cd: None,
            max_y_plus: None,
            min_convergence: None,
        };
        let j = serde_json::to_string(&none).unwrap();
        let b: TargetMetrics = serde_json::from_str(&j).unwrap();
        assert!(b.target_cl.is_none());
    }

    #[test]
    fn test_agent_manifest_round_trip() {
        let am = AgentManifest {
            iteration: 1,
            mesh_params: json!({"surface_min_level": 4}),
            solver_params: json!({"solver": "simpleFoam"}),
            bc_params: json!({"inlet": "fixedValue"}),
            reasoning: "Standard setup".into(),
        };
        let json = serde_json::to_string_pretty(&am).unwrap();
        let back: AgentManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.iteration, 1);
        assert_eq!(back.mesh_params["surface_min_level"], 4);
        assert_eq!(back.reasoning, "Standard setup");
    }

    #[test]
    fn test_forces_round_trip() {
        let f = Forces {
            cd: 0.025,
            cl: 0.45,
            cm: Some(-0.01),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Forces = serde_json::from_str(&json).unwrap();
        assert!((back.cd - 0.025).abs() < 1e-12);
        assert_eq!(back.cm, Some(-0.01));
        let f2 = Forces { cd: 0.0, cl: 0.0, cm: None };
        let j2 = serde_json::to_string(&f2).unwrap();
        let b2: Forces = serde_json::from_str(&j2).unwrap();
        assert_eq!(b2.cd, 0.0);
        assert!(b2.cm.is_none());
    }

    #[test]
    fn test_mesh_quality_round_trip() {
        let mq = MeshQuality {
            max_y_plus: 1.2,
            max_non_orthogonality: 45.0,
            max_skewness: 0.8,
            n_cells: 1_500_000,
        };
        let json = serde_json::to_string(&mq).unwrap();
        let back: MeshQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(back.n_cells, 1_500_000);
        assert!((back.max_y_plus - 1.2).abs() < 1e-12);
        let zero = MeshQuality {
            max_y_plus: 0.0,
            max_non_orthogonality: 0.0,
            max_skewness: 0.0,
            n_cells: 0,
        };
        let j = serde_json::to_string(&zero).unwrap();
        let b: MeshQuality = serde_json::from_str(&j).unwrap();
        assert_eq!(b.n_cells, 0);
    }

    #[test]
    fn test_convergence_round_trip() {
        let cv = Convergence {
            final_residual: 1e-6,
            n_iterations: 500,
            converged: true,
        };
        let json = serde_json::to_string(&cv).unwrap();
        let back: Convergence = serde_json::from_str(&json).unwrap();
        assert!(back.converged);
        assert_eq!(back.n_iterations, 500);
        assert!((back.final_residual - 1e-6).abs() < 1e-12);
        let cv2 = Convergence {
            final_residual: 0.5,
            n_iterations: 0,
            converged: false,
        };
        let j2 = serde_json::to_string(&cv2).unwrap();
        let b2: Convergence = serde_json::from_str(&j2).unwrap();
        assert!(!b2.converged);
    }

    #[test]
    fn test_agent_iteration_result_round_trip() {
        let air = AgentIterationResult {
            iteration: 3,
            manifest: AgentManifest {
                iteration: 3,
                mesh_params: json!({"surface_min_level": 5}),
                solver_params: json!({"solver": "simpleFoam"}),
                bc_params: json!({"inlet": "fixedValue"}),
                reasoning: "Refined mesh".into(),
            },
            status: "completed".into(),
            forces: Some(Forces {
                cd: 0.02,
                cl: 0.48,
                cm: None,
            }),
            mesh_quality: Some(MeshQuality {
                max_y_plus: 1.5,
                max_non_orthogonality: 40.0,
                max_skewness: 0.7,
                n_cells: 2_000_000,
            }),
            convergence: Some(Convergence {
                final_residual: 1e-7,
                n_iterations: 800,
                converged: true,
            }),
            score: Some(0.15),
            runtime_s: Some(3600.0),
            fix_applied: Some("coarsen_mesh".into()),
        };
        let json = serde_json::to_string_pretty(&air).unwrap();
        let back: AgentIterationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.iteration, 3);
        assert!(back.score.is_some());
        assert_eq!(back.fix_applied.as_deref(), Some("coarsen_mesh"));
        let air2 = AgentIterationResult {
            forces: None,
            mesh_quality: None,
            convergence: None,
            score: None,
            runtime_s: None,
            fix_applied: None,
            ..air
        };
        let json2 = serde_json::to_string(&air2).unwrap();
        let back2: AgentIterationResult = serde_json::from_str(&json2).unwrap();
        assert!(back2.forces.is_none());
        assert!(back2.score.is_none());
    }

    #[test]
    fn test_scoring_weights_round_trip_and_default() {
        let sw = ScoringWeights {
            w_cl_error: 1.0,
            w_cd_excess: 2.0,
            w_y_plus_penalty: 0.5,
            w_residual_penalty: 0.3,
            w_mesh_quality: 0.2,
        };
        let json = serde_json::to_string(&sw).unwrap();
        let back: ScoringWeights = serde_json::from_str(&json).unwrap();
        assert!((back.w_cl_error - 1.0).abs() < 1e-12);
        let def = ScoringWeights::default();
        assert!((def.w_cl_error - 1.0).abs() < 1e-12);
        assert!((def.w_cd_excess - 2.0).abs() < 1e-12);
        assert!((def.w_y_plus_penalty - 0.5).abs() < 1e-12);
        assert!((def.w_residual_penalty - 0.3).abs() < 1e-12);
        assert!((def.w_mesh_quality - 0.2).abs() < 1e-12);
        let j = serde_json::to_string(&def).unwrap();
        let b: ScoringWeights = serde_json::from_str(&j).unwrap();
        assert!((b.w_cl_error - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_scored_iteration_round_trip() {
        let si = ScoredIteration {
            iteration: 2,
            score: 0.15,
            cd: Some(0.025),
            cl: Some(0.45),
            max_y_plus: Some(1.2),
            converged: true,
            reasoning: "Good convergence".into(),
        };
        let json = serde_json::to_string(&si).unwrap();
        let back: ScoredIteration = serde_json::from_str(&json).unwrap();
        assert_eq!(back.iteration, 2);
        assert!((back.score - 0.15).abs() < 1e-12);
        assert!(back.converged);
        assert_eq!(back.reasoning, "Good convergence");
        let si2 = ScoredIteration {
            cd: None,
            cl: None,
            max_y_plus: None,
            converged: false,
            ..si
        };
        let json2 = serde_json::to_string(&si2).unwrap();
        let back2: ScoredIteration = serde_json::from_str(&json2).unwrap();
        assert!(back2.cd.is_none());
        assert!(!back2.converged);
    }

    #[test]
    fn test_chunk_construction() {
        let c1 = Chunk {
            content: Some("Hello".into()),
            tool_call: None,
            done: false,
        };
        assert!(c1.content.is_some());
        assert!(c1.tool_call.is_none());
        assert!(!c1.done);
        let c2 = Chunk {
            content: None,
            tool_call: Some(("id1".into(), "run_simulation".into(), json!({}))),
            done: true,
        };
        assert!(c2.content.is_none());
        assert!(c2.done);
        let (id, name, _) = c2.tool_call.unwrap();
        assert_eq!(id, "id1");
        assert_eq!(name, "run_simulation");
    }
}
