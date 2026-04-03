use vidodo_ir::{AuthorizationPolicy, CapabilityRequest, Diagnostic, PolicyRule};

/// Result of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny { reason: String },
    Degrade { reason: String },
}

/// Actor context submitted alongside a capability request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorContext {
    pub role: String,
    pub actor_id: String,
}

/// Minimal authorization engine that evaluates capability requests against
/// an [`AuthorizationPolicy`].
///
/// Rule evaluation follows Doc 08 §4:
/// - `human_operator`: full access to all capabilities
/// - `external_agent`: restricted — cannot invoke emergency patch or
///   deferred rollback
/// - `auto_recovery`: limited to degrade and rollback capabilities only
pub struct PolicyEngine {
    policy: AuthorizationPolicy,
}

impl PolicyEngine {
    pub fn new(policy: AuthorizationPolicy) -> Self {
        Self { policy }
    }

    /// Build the default policy implementing Doc 08 §4 three-role model.
    pub fn default_policy() -> Self {
        Self::new(AuthorizationPolicy {
            policy_id: String::from("default-policy"),
            version: String::from("0.1"),
            default_effect: String::from("deny"),
            rules: vec![
                PolicyRule {
                    role: String::from("human_operator"),
                    effect: String::from("allow"),
                    capabilities: Vec::new(), // empty = all
                    patch_classes: Vec::new(),
                    conditions: Default::default(),
                },
                PolicyRule {
                    role: String::from("external_agent"),
                    effect: String::from("allow"),
                    capabilities: Vec::new(), // will be narrowed by deny rules
                    patch_classes: Vec::new(),
                    conditions: Default::default(),
                },
                PolicyRule {
                    role: String::from("external_agent"),
                    effect: String::from("deny"),
                    capabilities: vec![
                        String::from("patch.submit"),
                        String::from("patch.deferred_rollback"),
                    ],
                    patch_classes: vec![String::from("emergency")],
                    conditions: Default::default(),
                },
                PolicyRule {
                    role: String::from("auto_recovery"),
                    effect: String::from("allow"),
                    capabilities: vec![
                        String::from("patch.rollback"),
                        String::from("patch.deferred_rollback"),
                        String::from("run.status"),
                        String::from("system.doctor"),
                    ],
                    patch_classes: Vec::new(),
                    conditions: Default::default(),
                },
            ],
        })
    }

    /// Evaluate a capability request under an actor context.
    ///
    /// Rules are evaluated in order. A deny rule that matches takes
    /// precedence over a prior allow for the same role if the deny rule
    /// is more specific (non-empty `capabilities` list). If no rule
    /// matches, the policy's `default_effect` applies.
    pub fn evaluate(&self, request: &CapabilityRequest, actor: &ActorContext) -> PolicyDecision {
        let capability = &request.capability;

        // Collect matching rules for this actor's role
        let role_rules: Vec<&PolicyRule> =
            self.policy.rules.iter().filter(|rule| rule.role == actor.role).collect();

        if role_rules.is_empty() {
            return self.apply_default(capability);
        }

        // Check deny rules first (specific capability matches)
        for rule in &role_rules {
            if rule.effect == "deny" && rule_matches_capability(rule, capability) {
                return PolicyDecision::Deny {
                    reason: format!(
                        "AUZ-001: role '{}' denied capability '{}'",
                        actor.role, capability
                    ),
                };
            }
        }

        // Check allow rules
        for rule in &role_rules {
            if rule.effect == "allow" && rule_matches_capability(rule, capability) {
                return PolicyDecision::Allow;
            }
        }

        // Check degrade rules
        for rule in &role_rules {
            if rule.effect == "degrade" && rule_matches_capability(rule, capability) {
                return PolicyDecision::Degrade {
                    reason: format!(
                        "AUZ-002: role '{}' degraded for capability '{}'",
                        actor.role, capability
                    ),
                };
            }
        }

        self.apply_default(capability)
    }

    fn apply_default(&self, capability: &str) -> PolicyDecision {
        match self.policy.default_effect.as_str() {
            "allow" => PolicyDecision::Allow,
            "degrade" => PolicyDecision::Degrade {
                reason: format!("AUZ-003: default degrade for '{capability}'"),
            },
            _ => {
                PolicyDecision::Deny { reason: format!("AUZ-001: default deny for '{capability}'") }
            }
        }
    }

    /// Return a diagnostic for a denied request (convenience helper).
    pub fn deny_diagnostic(decision: &PolicyDecision) -> Option<Diagnostic> {
        match decision {
            PolicyDecision::Deny { reason } => Some(Diagnostic::error("AUZ-001", reason.clone())),
            PolicyDecision::Degrade { reason } => {
                Some(Diagnostic::warning("AUZ-002", reason.clone()))
            }
            PolicyDecision::Allow => None,
        }
    }
}

/// Check whether a rule matches a specific capability.
/// An empty `capabilities` list means "all capabilities".
fn rule_matches_capability(rule: &PolicyRule, capability: &str) -> bool {
    rule.capabilities.is_empty() || rule.capabilities.iter().any(|c| c == capability)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(capability: &str) -> CapabilityRequest {
        CapabilityRequest {
            request_id: String::from("req-001"),
            capability: capability.to_string(),
            payload: serde_json::Value::Null,
            actor: None,
            metadata: None,
        }
    }

    fn actor(role: &str) -> ActorContext {
        ActorContext { role: role.to_string(), actor_id: String::from("test-actor") }
    }

    #[test]
    fn human_operator_full_access() {
        let engine = PolicyEngine::default_policy();
        let op = actor("human_operator");
        assert_eq!(engine.evaluate(&request("compile.run"), &op), PolicyDecision::Allow);
        assert_eq!(engine.evaluate(&request("patch.submit"), &op), PolicyDecision::Allow);
        assert_eq!(
            engine.evaluate(&request("patch.deferred_rollback"), &op),
            PolicyDecision::Allow
        );
        assert_eq!(engine.evaluate(&request("system.capabilities"), &op), PolicyDecision::Allow);
    }

    #[test]
    fn external_agent_allowed_normal_capabilities() {
        let engine = PolicyEngine::default_policy();
        let agent = actor("external_agent");
        assert_eq!(engine.evaluate(&request("compile.run"), &agent), PolicyDecision::Allow);
        assert_eq!(engine.evaluate(&request("asset.list"), &agent), PolicyDecision::Allow);
        assert_eq!(engine.evaluate(&request("run.start"), &agent), PolicyDecision::Allow);
    }

    #[test]
    fn external_agent_blocked_emergency_patch() {
        let engine = PolicyEngine::default_policy();
        let agent = actor("external_agent");
        let decision = engine.evaluate(&request("patch.submit"), &agent);
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
        let decision2 = engine.evaluate(&request("patch.deferred_rollback"), &agent);
        assert!(matches!(decision2, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn auto_recovery_only_degrade_rollback() {
        let engine = PolicyEngine::default_policy();
        let auto = actor("auto_recovery");
        // Allowed
        assert_eq!(engine.evaluate(&request("patch.rollback"), &auto), PolicyDecision::Allow);
        assert_eq!(
            engine.evaluate(&request("patch.deferred_rollback"), &auto),
            PolicyDecision::Allow
        );
        assert_eq!(engine.evaluate(&request("run.status"), &auto), PolicyDecision::Allow);
        assert_eq!(engine.evaluate(&request("system.doctor"), &auto), PolicyDecision::Allow);
        // Denied (not in allowed list)
        let denied = engine.evaluate(&request("compile.run"), &auto);
        assert!(matches!(denied, PolicyDecision::Deny { .. }));
        let denied2 = engine.evaluate(&request("patch.submit"), &auto);
        assert!(matches!(denied2, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn unknown_role_gets_default_deny() {
        let engine = PolicyEngine::default_policy();
        let unknown = actor("rogue_agent");
        let decision = engine.evaluate(&request("compile.run"), &unknown);
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn deny_diagnostic_produces_structured_error() {
        let decision = PolicyDecision::Deny { reason: String::from("AUZ-001: denied") };
        let diag = PolicyEngine::deny_diagnostic(&decision);
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code, "AUZ-001");
    }

    #[test]
    fn allow_has_no_diagnostic() {
        assert!(PolicyEngine::deny_diagnostic(&PolicyDecision::Allow).is_none());
    }
}
