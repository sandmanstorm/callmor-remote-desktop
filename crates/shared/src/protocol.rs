use serde::{Deserialize, Serialize};

/// Envelope for all signaling messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    #[serde(rename = "hello")]
    Hello {
        role: Role,
        machine_id: String,
        /// Agent token (for Role::Agent) or session JWT (for Role::Browser).
        /// Optional for backwards compat during dev; relay enforces when AUTH_REQUIRED is set.
        #[serde(default)]
        token: Option<String>,
    },

    #[serde(rename = "relay")]
    Relay { payload: serde_json::Value },

    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Agent,
    Browser,
}
