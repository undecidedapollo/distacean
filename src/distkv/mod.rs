pub mod operator_set;

use std::sync::Arc;

use crate::core::DistaceanCore;
use crate::core::LeaderResponse;
use crate::distkv::operator_set::SetRequest;
use crate::distkv::operator_set::SetRequestBuilder;
use crate::protocol::LinearizerData;
use crate::protocol::RequestType;
use crate::raft::KVOperation;
use crate::raft::RequestOperation;
use openraft::error::decompose::DecomposeResult;
use openraft::raft::linearizable_read::Linearizer;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::operator_set::SetError;

pub(crate) struct DistKVCore {
    pub(crate) distacean: Arc<DistaceanCore>,
}

#[derive(Clone)]
pub struct DistKV {
    pub(crate) core: Arc<DistKVCore>,
}

pub type InitialSetBuilder =
    SetRequestBuilder<operator_set::SetValue<operator_set::SetKey<operator_set::SetDistacean>>>;

impl DistKVCore {}

impl DistKV {
    pub fn set<T: Serialize, V: std::borrow::Borrow<T>>(
        self: &DistKV,
        key: impl Into<String>,
        value: V,
    ) -> InitialSetBuilder {
        SetRequest::builder()
            .distacean(self.core.distacean.clone())
            .key(key.into())
            .value(rmp_serde::to_vec(value.borrow()).expect("Failed to serialize value"))
    }

    pub async fn delete(
        self: &DistKV,
        key: impl Into<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.core
            .distacean
            .write_or_forward_to_leader(RequestOperation::KV(KVOperation::Del { key: key.into() }))
            .await?;
        Ok(())
    }

    pub async fn eventual_read<T: DeserializeOwned>(
        self: &DistKV,
        key: impl Into<String>,
    ) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
        let key = key.into();
        let value = self.core.distacean.state_machine_store.get(&key).await?;
        Ok(if let Some(value) = value {
            if value.is_empty() {
                None
            } else {
                Some(rmp_serde::from_slice(&value)?)
            }
        } else {
            None
        })
    }

    pub async fn read<T: DeserializeOwned>(
        self: &DistKV,
        key: impl Into<String>,
    ) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
        let key = key.into();
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

                Ok(if value.is_empty() {
                    None
                } else {
                    Some(rmp_serde::from_slice(&value)?)
                })
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
                Ok(if value.is_empty() {
                    None
                } else {
                    Some(rmp_serde::from_slice(&value)?)
                })
            }
            LeaderResponse::NoLeader => Err("No leader available".to_string().into()),
        }
    }

    pub async fn get_with_revision<T: DeserializeOwned>(
        self: &DistKV,
        key: impl Into<String>,
    ) -> Result<Option<(T, u64)>, Box<dyn std::error::Error + Send + Sync>> {
        let key = key.into();
        let result = self
            .core
            .distacean
            .state_machine_store
            .get_with_revision(&key)
            .await?;

        Ok(if let Some((value, revision)) = result {
            if value.is_empty() {
                None
            } else {
                Some((rmp_serde::from_slice(&value)?, revision))
            }
        } else {
            None
        })
    }
}
