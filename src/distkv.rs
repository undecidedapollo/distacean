use std::sync::Arc;

use crate::core::DistaceanCore;
use crate::core::LeaderResponse;
use crate::protocol::LinearizerData;
use crate::protocol::RequestType;
use crate::raft::KVOperation;
use crate::raft::Request;
use crate::raft::RequestOperation;
use openraft::error::decompose::DecomposeResult;
use openraft::raft::linearizable_read::Linearizer;

pub(crate) struct DistKVCore {
    pub(crate) distacean: Arc<DistaceanCore>,
}

#[derive(Clone)]
pub struct DistKV {
    pub(crate) core: Arc<DistKVCore>,
}

impl DistKVCore {}

impl DistKV {
    pub async fn set(
        self: &DistKV,
        key: String,
        value: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.core
            .distacean
            .write_or_forward_to_leader(RequestOperation::KV(KVOperation::Set {
                key: key.clone(),
                value: value.clone(),
            }))
            .await?;
        Ok(())
    }

    pub async fn delete(
        self: &DistKV,
        key: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.core
            .distacean
            .write_or_forward_to_leader(RequestOperation::KV(KVOperation::Del { key: key.clone() }))
            .await?;
        Ok(())
    }

    pub async fn eventual_read(
        self: &DistKV,
        key: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let value = self.core.distacean.state_machine_store.get(&key).await?;
        Ok(value.unwrap_or_default())
    }

    pub async fn read(
        self: &DistKV,
        key: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let leader = self.core.distacean.get_leader_peer().await?;

        match leader {
            LeaderResponse::NodeIsLeader => {
                // Get linearizer from local raft
                let linearizer = self
                    .core
                    .distacean
                    .raft
                    .get_read_linearizer(openraft::raft::ReadPolicy::ReadIndex)
                    .await
                    .decompose()
                    .unwrap()?;

                // Wait for local state machine to catch up
                linearizer.await_ready(&self.core.distacean.raft).await?;

                // Read from local state machine
                let value = self
                    .core
                    .distacean
                    .state_machine_store
                    .get(&key)
                    .await?
                    .unwrap_or_default();

                Ok(value)
            }
            LeaderResponse::NodeIsFollower(leader_peer) => {
                let data = rmp_serde::to_vec(&RequestType::Linearizer)?;
                let res = leader_peer.req_res(data).await?;
                let linearizer_data: Result<LinearizerData, String> = rmp_serde::from_slice(&res)?;
                let linearizer_data = linearizer_data?;

                let linearizer = Linearizer::new(
                    linearizer_data.node_id,
                    linearizer_data.read_log_id,
                    linearizer_data.applied,
                );

                // Wait for local state machine to catch up
                if let Err(e) = linearizer.await_ready(&self.core.distacean.raft).await {
                    return Err(format!("Failed to await linearizer: {}", e).into());
                }

                // Read from local state machine
                let value = self
                    .core
                    .distacean
                    .state_machine_store
                    .get(&key)
                    .await?
                    .unwrap_or_default();
                Ok(value)
            }
            LeaderResponse::NoLeader => Err("No leader available".to_string().into()),
        }
    }
}
