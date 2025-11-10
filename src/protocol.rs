use openraft::{LogId, ReadPolicy as OpenraftReadPolicy};
use serde::{Deserialize, Serialize};

use crate::raft::{NodeId, TypeConfig};

/// Copied from openraft as it doesn't implement Deserialize/Serialize.
/// Policy that determines how to handle read operations in a Raft cluster.
///
/// This enum defines strategies for ensuring linearizable reads in distributed systems
/// while balancing between consistency guarantees and performance.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ReadPolicy {
    /// Uses leader lease to avoid network round-trips for read operations.
    ///
    /// With `LeaseRead`, the leader can serve reads locally without contacting followers
    /// as long as it believes its leadership lease is still valid. This provides better
    /// performance compared to `ReadIndex` but assumes clock drift between nodes is negligible.
    ///
    /// Note: This offers slightly weaker consistency guarantees than `ReadIndex` in exchange
    /// for lower latency.
    LeaseRead,

    /// Implements the ReadIndex protocol to ensure linearizable reads.
    ///
    /// With `ReadIndex`, the leader confirms its leadership status by contacting a quorum
    /// of followers before serving read requests. This ensures strong consistency but incurs
    /// the cost of network communication for each read operation.
    ///
    /// This is the safer option that provides the strongest consistency guarantees.
    ReadIndex,
}

impl From<ReadPolicy> for OpenraftReadPolicy {
    fn from(policy: ReadPolicy) -> Self {
        match policy {
            ReadPolicy::LeaseRead => OpenraftReadPolicy::LeaseRead,
            ReadPolicy::ReadIndex => OpenraftReadPolicy::ReadIndex,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestType {
    AppendEntriesRequest(Vec<u8>),
    InstallSnapshotRequest(Vec<u8>),
    VoteRequest(Vec<u8>),
    AppRequest(crate::raft::Request),
    Linearizer { read_policy: ReadPolicy },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearizerData {
    pub node_id: NodeId,
    pub read_log_id: LogId<TypeConfig>,
    pub applied: Option<LogId<TypeConfig>>,
}
