use saucer_sys::*;

/// The policy towards an event. Can be used to allow or block the default behavior.
pub enum Policy {
    Allow,
    Block,
}

impl From<Policy> for saucer_policy {
    fn from(value: Policy) -> Self {
        match value {
            Policy::Allow => SAUCER_POLICY_ALLOW,
            Policy::Block => SAUCER_POLICY_BLOCK,
        }
    }
}
