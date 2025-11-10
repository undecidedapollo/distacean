use std::sync::Arc;

use self::read_request_builder::State;
use crate::core::DistaceanCore;
use crate::core::ReadSource;
use crate::distkv::operator_read::read_request_builder::SetConsistency;
use crate::distkv::operator_read::read_request_builder::SetSource;
use crate::protocol::ReadPolicy;
use bon::Builder;
use serde::de::DeserializeOwned;

pub use self::read_request_builder::{SetDistacean, SetKey};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum KVReadError {
    #[error("Unknown error: {0}")]
    Unknown(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadConsistency {
    AsIs,
    LeaseRead,
    Linearizable,
}

#[derive(Builder)]
pub struct ReadRequest {
    distacean: Arc<DistaceanCore>,
    key: String,

    #[builder(default = ReadSource::Leader)]
    source: ReadSource,
    #[builder(default = ReadConsistency::Linearizable)]
    consistency: ReadConsistency,
}

impl ReadRequest {
    async fn get_linearizer(
        &self,
    ) -> Result<
        Option<openraft::raft::linearizable_read::LinearizeState<crate::raft::TypeConfig>>,
        KVReadError,
    > {
        let linearizer_state: Option<
            openraft::raft::linearizable_read::LinearizeState<crate::raft::TypeConfig>,
        > = match self.consistency {
            ReadConsistency::AsIs => None, // TODO: Implement AsIs for leader reads
            ReadConsistency::LeaseRead => Some(
                self.distacean
                    .get_linearizer(self.source, ReadPolicy::LeaseRead)
                    .await
                    .map_err(|e| KVReadError::Unknown(e.into()))?,
            ),
            ReadConsistency::Linearizable => Some(
                self.distacean
                    .get_linearizer(self.source, ReadPolicy::ReadIndex)
                    .await
                    .map_err(|e| KVReadError::Unknown(e.into()))?,
            ),
        };
        Ok(linearizer_state)
    }

    async fn execute<T: DeserializeOwned>(self) -> Result<T, KVReadError> {
        self.get_linearizer().await?;

        // Read from local state machine
        let value = self
            .distacean
            .state_machine_store
            .get(&self.key)
            .await
            .map_err(|e| KVReadError::Unknown(e.into()))?
            .unwrap_or_default();

        return Ok(rmp_serde::from_slice(&value).map_err(|e| KVReadError::Unknown(e.into()))?);
    }

    async fn execute_with_revision<T: DeserializeOwned>(
        self,
    ) -> Result<Option<(T, u64)>, KVReadError> {
        let _ = self.get_linearizer().await;

        // Read from local state machine
        let result = self
            .distacean
            .state_machine_store
            .get_with_revision(&self.key)
            .await
            .map_err(|e| KVReadError::Unknown(e.into()))?;

        Ok(match result {
            Some((value, revision)) => Some((
                rmp_serde::from_slice(&value).map_err(|e| KVReadError::Unknown(e.into()))?,
                revision,
            )),
            None => None,
        })
    }
}

impl<S> ReadRequestBuilder<S>
where
    S: State + read_request_builder::IsComplete,
{
    pub async fn execute<T: DeserializeOwned>(self) -> Result<T, KVReadError> {
        self.build().execute().await
    }
    pub async fn execute_with_revision<T: DeserializeOwned>(
        self,
    ) -> Result<Option<(T, u64)>, KVReadError> {
        self.build().execute_with_revision().await
    }
}

impl<S> ReadRequestBuilder<S>
where
    S: State,
    <S as State>::Source: read_request_builder::IsUnset,
{
    pub fn local(self) -> ReadRequestBuilder<SetSource<S>> {
        self.source(ReadSource::Local)
    }

    pub fn leader(self) -> ReadRequestBuilder<SetSource<S>> {
        self.source(ReadSource::Leader)
    }
}

impl<S> ReadRequestBuilder<S>
where
    S: State,
    <S as State>::Consistency: read_request_builder::IsUnset,
{
    pub fn as_is(self) -> ReadRequestBuilder<SetConsistency<S>> {
        self.consistency(ReadConsistency::AsIs)
    }

    pub fn leader_lease(self) -> ReadRequestBuilder<SetConsistency<S>> {
        self.consistency(ReadConsistency::LeaseRead)
    }

    pub fn linearizable(self) -> ReadRequestBuilder<SetConsistency<S>> {
        self.consistency(ReadConsistency::Linearizable)
    }
}

#[derive(Debug)]
pub enum SetError {
    RevisionMismatch { current_revision: u64 },
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetError::RevisionMismatch { current_revision } => {
                write!(
                    f,
                    "Revision mismatch: current revision is {}",
                    current_revision
                )
            }
            SetError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for SetError {}
