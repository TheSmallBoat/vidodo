use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use vidodo_ir::{
    CapabilityDescriptor, CapabilityRequest, Diagnostic, OperationTicket, ResponseEnvelope,
};

// ---------------------------------------------------------------------------
// Capability Registry
// ---------------------------------------------------------------------------

pub struct CapabilityRegistry {
    descriptors: Vec<CapabilityDescriptor>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self { descriptors: Vec::new() }
    }

    /// Build the default registry containing all Phase 0 + Phase 1 capabilities.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();
        for descriptor in builtin_descriptors() {
            registry.register(descriptor);
        }
        registry
    }

    pub fn register(&mut self, descriptor: CapabilityDescriptor) {
        if !self.descriptors.iter().any(|d| d.capability == descriptor.capability) {
            self.descriptors.push(descriptor);
        }
    }

    pub fn lookup(&self, capability: &str) -> Option<&CapabilityDescriptor> {
        self.descriptors.iter().find(|d| d.capability == capability)
    }

    pub fn list(&self) -> &[CapabilityDescriptor] {
        &self.descriptors
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

// ---------------------------------------------------------------------------
// Capability Router
// ---------------------------------------------------------------------------

/// Result of routing a capability request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteTarget {
    AssetIngest,
    AssetList,
    AssetShow,
    PlanValidate,
    PlanSubmit,
    CompileRun,
    RevisionList,
    RevisionPublish,
    RevisionArchive,
    RunStart,
    RunStatus,
    PatchCheck,
    PatchSubmit,
    PatchRollback,
    PatchDeferredRollback,
    TraceShow,
    TraceEvents,
    EvalRun,
    ExportAudio,
    SystemDoctor,
    SystemCapabilities,
}

/// Route a capability identifier to a typed target.
pub fn route(capability: &str) -> Result<RouteTarget, Box<Diagnostic>> {
    match capability {
        "asset.ingest" => Ok(RouteTarget::AssetIngest),
        "asset.list" => Ok(RouteTarget::AssetList),
        "asset.show" => Ok(RouteTarget::AssetShow),
        "plan.validate" => Ok(RouteTarget::PlanValidate),
        "plan.submit" => Ok(RouteTarget::PlanSubmit),
        "compile.run" => Ok(RouteTarget::CompileRun),
        "revision.list" => Ok(RouteTarget::RevisionList),
        "revision.publish" => Ok(RouteTarget::RevisionPublish),
        "revision.archive" => Ok(RouteTarget::RevisionArchive),
        "run.start" => Ok(RouteTarget::RunStart),
        "run.status" => Ok(RouteTarget::RunStatus),
        "patch.check" => Ok(RouteTarget::PatchCheck),
        "patch.submit" => Ok(RouteTarget::PatchSubmit),
        "patch.rollback" => Ok(RouteTarget::PatchRollback),
        "patch.deferred_rollback" => Ok(RouteTarget::PatchDeferredRollback),
        "trace.show" => Ok(RouteTarget::TraceShow),
        "trace.events" => Ok(RouteTarget::TraceEvents),
        "eval.run" => Ok(RouteTarget::EvalRun),
        "export.audio" => Ok(RouteTarget::ExportAudio),
        "system.doctor" => Ok(RouteTarget::SystemDoctor),
        "system.capabilities" => Ok(RouteTarget::SystemCapabilities),
        _ => Err(Box::new(Diagnostic::error(
            "CAP-001",
            format!("unsupported capability: {capability}"),
        ))),
    }
}

// ---------------------------------------------------------------------------
// MCP Tool ↔ Capability Mapping (WSI-01)
// ---------------------------------------------------------------------------

/// An MCP tool definition that maps to a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpToolMapping {
    pub tool_name: String,
    pub capability: String,
    pub read_only: bool,
    pub is_async: bool,
}

/// Return the complete MCP tool → capability mapping table.
pub fn mcp_tool_mappings() -> Vec<McpToolMapping> {
    vec![
        mcp_map("asset.ingest", "asset.ingest", false, true),
        mcp_map("asset.list", "asset.list", true, false),
        mcp_map("asset.show", "asset.show", true, false),
        mcp_map("plan.validate", "plan.validate", true, false),
        mcp_map("plan.submit", "plan.submit", false, true),
        mcp_map("compile.run", "compile.run", false, true),
        mcp_map("revision.list", "revision.list", true, false),
        mcp_map("revision.publish", "revision.publish", false, false),
        mcp_map("revision.archive", "revision.archive", false, false),
        mcp_map("run.start", "run.start", false, true),
        mcp_map("run.status", "run.status", true, false),
        mcp_map("patch.check", "patch.check", true, false),
        mcp_map("patch.submit", "patch.submit", false, true),
        mcp_map("patch.rollback", "patch.rollback", false, false),
        mcp_map("patch.deferred_rollback", "patch.deferred_rollback", false, false),
        mcp_map("trace.show", "trace.show", true, false),
        mcp_map("trace.events", "trace.events", true, false),
        mcp_map("eval.run", "eval.run", false, true),
        mcp_map("export.audio", "export.audio", false, true),
        mcp_map("system.doctor", "system.doctor", true, false),
        mcp_map("system.capabilities", "system.capabilities", true, false),
    ]
}

/// Resolve an MCP tool name to a capability identifier.
pub fn resolve_mcp_tool(tool_name: &str) -> Option<&'static str> {
    // Tool names currently map 1:1 to capability identifiers.
    match tool_name {
        "asset.ingest" => Some("asset.ingest"),
        "asset.list" => Some("asset.list"),
        "asset.show" => Some("asset.show"),
        "plan.validate" => Some("plan.validate"),
        "plan.submit" => Some("plan.submit"),
        "compile.run" => Some("compile.run"),
        "revision.list" => Some("revision.list"),
        "revision.publish" => Some("revision.publish"),
        "revision.archive" => Some("revision.archive"),
        "run.start" => Some("run.start"),
        "run.status" => Some("run.status"),
        "patch.check" => Some("patch.check"),
        "patch.submit" => Some("patch.submit"),
        "patch.rollback" => Some("patch.rollback"),
        "patch.deferred_rollback" => Some("patch.deferred_rollback"),
        "trace.show" => Some("trace.show"),
        "trace.events" => Some("trace.events"),
        "eval.run" => Some("eval.run"),
        "export.audio" => Some("export.audio"),
        "system.doctor" => Some("system.doctor"),
        "system.capabilities" => Some("system.capabilities"),
        _ => None,
    }
}

fn mcp_map(tool_name: &str, capability: &str, read_only: bool, is_async: bool) -> McpToolMapping {
    McpToolMapping {
        tool_name: String::from(tool_name),
        capability: String::from(capability),
        read_only,
        is_async,
    }
}

// ---------------------------------------------------------------------------
// Operation Tracker
// ---------------------------------------------------------------------------

/// Return current UNIX epoch in milliseconds.
fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub struct OperationTracker {
    tickets: Vec<OperationTicket>,
    next_id: u64,
}

impl OperationTracker {
    pub fn new() -> Self {
        Self { tickets: Vec::new(), next_id: 1 }
    }

    /// Create a new operation ticket for an async capability invocation.
    pub fn start(&mut self, request: &CapabilityRequest) -> OperationTicket {
        let ticket = OperationTicket {
            operation_id: format!("op-{:04}", self.next_id),
            request_id: request.request_id.clone(),
            capability: request.capability.clone(),
            state: String::from("running"),
            started_at: now_millis(),
            updated_at: None,
            artifact_refs: Vec::new(),
        };
        self.next_id += 1;
        self.tickets.push(ticket.clone());
        ticket
    }

    /// Start an operation only if the capability is async.
    /// Returns `None` for sync capabilities.
    pub fn start_if_async(
        &mut self,
        request: &CapabilityRequest,
        registry: &CapabilityRegistry,
    ) -> Option<OperationTicket> {
        let descriptor = registry.lookup(&request.capability)?;
        if descriptor.execution_mode == "async" { Some(self.start(request)) } else { None }
    }

    /// Mark an operation as succeeded, attaching optional artifact refs.
    pub fn complete(&mut self, operation_id: &str, artifact_refs: Vec<String>) -> bool {
        if let Some(ticket) = self.tickets.iter_mut().find(|t| t.operation_id == operation_id) {
            ticket.state = String::from("succeeded");
            ticket.updated_at = Some(now_millis());
            ticket.artifact_refs = artifact_refs;
            true
        } else {
            false
        }
    }

    /// Mark an operation as failed.
    pub fn fail(&mut self, operation_id: &str) -> bool {
        if let Some(ticket) = self.tickets.iter_mut().find(|t| t.operation_id == operation_id) {
            ticket.state = String::from("failed");
            ticket.updated_at = Some(now_millis());
            true
        } else {
            false
        }
    }

    pub fn get(&self, operation_id: &str) -> Option<&OperationTicket> {
        self.tickets.iter().find(|t| t.operation_id == operation_id)
    }

    pub fn list(&self) -> &[OperationTicket] {
        &self.tickets
    }
}

impl Default for OperationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an error envelope for a capability-level failure.
pub fn error_envelope<T: Serialize + Default>(
    capability: &str,
    request_id: &str,
    diagnostic: Diagnostic,
) -> ResponseEnvelope<T> {
    ResponseEnvelope::new(
        "error",
        capability,
        request_id,
        T::default(),
        vec![diagnostic],
        vec![],
        vec![],
    )
}

// ---------------------------------------------------------------------------
// Built-in capability descriptors
// ---------------------------------------------------------------------------

fn cap(
    capability: &str,
    execution_mode: &str,
    idempotency: &str,
    authorization: &[&str],
    description: &str,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: String::from(capability),
        version: String::from("0.1"),
        execution_mode: String::from(execution_mode),
        idempotency: String::from(idempotency),
        authorization: authorization.iter().map(|s| String::from(*s)).collect(),
        description: String::from(description),
        input_schema: String::new(),
        output_schema: String::new(),
        target_service: String::new(),
    }
}

fn builtin_descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        cap(
            "asset.ingest",
            "async",
            "conditional",
            &["operator"],
            "Import assets from a source directory",
        ),
        cap("asset.list", "sync", "idempotent", &["operator", "planner"], "List published assets"),
        cap(
            "asset.show",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "Show asset details and analysis",
        ),
        cap("plan.validate", "sync", "idempotent", &["planner"], "Validate a plan bundle"),
        cap(
            "plan.submit",
            "async",
            "conditional",
            &["planner"],
            "Submit a plan bundle for compilation",
        ),
        cap(
            "compile.run",
            "async",
            "conditional",
            &["planner"],
            "Compile a plan into a candidate revision",
        ),
        cap(
            "revision.list",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "List revisions for a show",
        ),
        cap(
            "revision.publish",
            "sync",
            "conditional",
            &["operator"],
            "Publish a candidate revision",
        ),
        cap("revision.archive", "sync", "conditional", &["operator"], "Archive a revision"),
        cap("run.start", "async", "conditional", &["operator"], "Start a live or offline run"),
        cap(
            "run.status",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "Query current run status",
        ),
        cap("patch.check", "sync", "idempotent", &["planner"], "Pre-check a patch proposal"),
        cap("patch.submit", "async", "conditional", &["planner"], "Submit a patch proposal"),
        cap("patch.rollback", "sync", "conditional", &["operator"], "Roll back an active patch"),
        cap(
            "patch.deferred_rollback",
            "sync",
            "conditional",
            &["operator"],
            "Trigger deferred rollback on anomaly",
        ),
        cap(
            "trace.show",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "Show trace manifest for a run",
        ),
        cap(
            "trace.events",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "Query trace events by bar range",
        ),
        cap(
            "eval.run",
            "async",
            "conditional",
            &["planner"],
            "Generate evaluation report from a trace",
        ),
        cap("export.audio", "async", "conditional", &["operator"], "Export audio from a trace run"),
        cap("system.doctor", "sync", "idempotent", &["operator"], "Run full pipeline health check"),
        cap(
            "system.capabilities",
            "sync",
            "idempotent",
            &["operator", "planner"],
            "List available capabilities",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_21_capabilities() {
        let registry = CapabilityRegistry::default();
        assert_eq!(registry.len(), 21);
    }

    #[test]
    fn lookup_returns_descriptor() {
        let registry = CapabilityRegistry::default();
        let descriptor = registry.lookup("compile.run").expect("missing compile.run");
        assert_eq!(descriptor.execution_mode, "async");
        assert_eq!(descriptor.idempotency, "conditional");
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let registry = CapabilityRegistry::default();
        assert!(registry.lookup("unknown.verb").is_none());
    }

    #[test]
    fn route_known_capability() {
        assert_eq!(route("asset.ingest"), Ok(RouteTarget::AssetIngest));
        assert_eq!(route("system.capabilities"), Ok(RouteTarget::SystemCapabilities));
    }

    #[test]
    fn route_unknown_capability_returns_diagnostic() {
        let result = route("nonexistent.verb");
        assert!(result.is_err());
        let diag = result.unwrap_err();
        assert_eq!(diag.code, "CAP-001");
    }

    #[test]
    fn operation_tracker_lifecycle() {
        let mut tracker = OperationTracker::new();
        let req = CapabilityRequest {
            request_id: String::from("req-001"),
            capability: String::from("compile.run"),
            payload: serde_json::Value::Null,
            actor: None,
            metadata: None,
        };

        let ticket = tracker.start(&req);
        assert_eq!(ticket.state, "running");
        assert_eq!(ticket.operation_id, "op-0001");
        assert!(ticket.started_at > 0, "started_at should be a real timestamp");
        assert!(ticket.updated_at.is_none());

        assert!(tracker.complete(&ticket.operation_id, vec![String::from("artifact.json")]));
        let updated = tracker.get(&ticket.operation_id).expect("missing ticket");
        assert_eq!(updated.state, "succeeded");
        assert_eq!(updated.artifact_refs, vec!["artifact.json"]);
        assert!(updated.updated_at.is_some(), "updated_at should be set on completion");
        assert!(updated.updated_at.unwrap() >= updated.started_at);
    }

    #[test]
    fn operation_tracker_fail() {
        let mut tracker = OperationTracker::new();
        let req = CapabilityRequest {
            request_id: String::from("req-002"),
            capability: String::from("eval.run"),
            payload: serde_json::Value::Null,
            actor: None,
            metadata: None,
        };

        let ticket = tracker.start(&req);
        assert!(tracker.fail(&ticket.operation_id));
        let updated = tracker.get(&ticket.operation_id).expect("missing ticket");
        assert_eq!(updated.state, "failed");
        assert!(updated.updated_at.is_some(), "updated_at should be set on failure");
    }

    #[test]
    fn start_if_async_creates_ticket_for_async_capability() {
        let mut tracker = OperationTracker::new();
        let registry = CapabilityRegistry::default();
        let req = CapabilityRequest {
            request_id: String::from("req-003"),
            capability: String::from("compile.run"), // async
            payload: serde_json::Value::Null,
            actor: None,
            metadata: None,
        };
        let ticket = tracker.start_if_async(&req, &registry);
        assert!(ticket.is_some(), "compile.run is async, should get a ticket");
        assert_eq!(tracker.list().len(), 1);
    }

    #[test]
    fn start_if_async_skips_sync_capability() {
        let mut tracker = OperationTracker::new();
        let registry = CapabilityRegistry::default();
        let req = CapabilityRequest {
            request_id: String::from("req-004"),
            capability: String::from("asset.list"), // sync
            payload: serde_json::Value::Null,
            actor: None,
            metadata: None,
        };
        let ticket = tracker.start_if_async(&req, &registry);
        assert!(ticket.is_none(), "asset.list is sync, should not get a ticket");
        assert!(tracker.list().is_empty());
    }

    #[test]
    fn descriptors_serde_round_trip() {
        let registry = CapabilityRegistry::default();
        let json = serde_json::to_string(registry.list()).expect("serialize");
        let deserialized: Vec<CapabilityDescriptor> =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.len(), 21);
        assert_eq!(deserialized[0].capability, "asset.ingest");
    }

    #[test]
    fn mcp_tool_mappings_has_21_entries() {
        let mappings = mcp_tool_mappings();
        assert_eq!(mappings.len(), 21);
        // Every mapping's capability should exist in the registry
        let registry = CapabilityRegistry::default();
        for m in &mappings {
            assert!(
                registry.lookup(&m.capability).is_some(),
                "MCP tool {} maps to unknown capability {}",
                m.tool_name,
                m.capability
            );
        }
    }

    #[test]
    fn resolve_mcp_tool_known_and_unknown() {
        assert_eq!(resolve_mcp_tool("compile.run"), Some("compile.run"));
        assert_eq!(resolve_mcp_tool("patch.submit"), Some("patch.submit"));
        assert!(resolve_mcp_tool("nonexistent.tool").is_none());
    }

    #[test]
    fn mcp_async_flags_match_capability_execution_mode() {
        let registry = CapabilityRegistry::default();
        for m in mcp_tool_mappings() {
            let desc = registry.lookup(&m.capability).unwrap();
            let expected_async = desc.execution_mode == "async";
            assert_eq!(
                m.is_async, expected_async,
                "MCP tool {} is_async={} but capability execution_mode={}",
                m.tool_name, m.is_async, desc.execution_mode
            );
        }
    }
}
