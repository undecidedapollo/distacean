use openraft::LogId;
use serde::{Deserialize, Serialize};

use crate::raft::{NodeId, TypeConfig};

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestType {
    AppendEntriesRequest(Vec<u8>),
    InstallSnapshotRequest(Vec<u8>),
    VoteRequest(Vec<u8>),
    AppRequest(crate::raft::Request),
    Linearizer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReadSource {
    Local,
    Leader,
    // We could add "Quorum", reach out to a majority of nodes (including self, doesn't have to be leader)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadConsistency {
    Stale,
    LeaderLease,
    Linearizable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadOptions {
    pub source: ReadSource,
    pub consistency: ReadConsistency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearizerData {
    pub node_id: NodeId,
    pub read_log_id: LogId<TypeConfig>,
    pub applied: Option<LogId<TypeConfig>>,
}
