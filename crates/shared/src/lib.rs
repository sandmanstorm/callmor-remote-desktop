/// Shared types and utilities for the Callmor platform.

/// Protocol message types exchanged between relay, agent, and browser clients.
pub mod protocol {
    use serde::{Deserialize, Serialize};

    /// Envelope for all signaling messages.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum SignalMessage {
        /// Identify this connection (agent or browser).
        #[serde(rename = "hello")]
        Hello { role: Role, machine_id: String },

        /// Forwarded to the peer on the other side.
        #[serde(rename = "relay")]
        Relay { payload: serde_json::Value },

        /// Server-sent error.
        #[serde(rename = "error")]
        Error { message: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum Role {
        Agent,
        Browser,
    }
}
