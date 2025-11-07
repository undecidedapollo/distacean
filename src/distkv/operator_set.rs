use std::sync::Arc;

use self::set_request_builder::State;
use crate::core::DistaceanCore;
use crate::raft::KVOperation;
use crate::raft::RequestOperation;
use crate::raft::SetResponse;
use bon::Builder;

pub use self::set_request_builder::{SetDistacean, SetKey, SetReturnPrevious, SetValue};

#[derive(Builder)]
pub struct SetRequest {
    distacean: Arc<DistaceanCore>,
    key: String,
    value: Vec<u8>,

    #[builder(default = false)]
    pub return_previous: bool,

    pub expected_revision: Option<u64>,
}

impl SetRequest {
    async fn execute(self) -> Result<SetResponse, SetError> {
        let distacean = self.distacean;
        let key = self.key;
        let value = self.value;
        let return_previous = self.return_previous;
        let expected_revision = self.expected_revision;

        let response = if let Some(expected_revision) = expected_revision {
            // CAS operation
            distacean
                .write_or_forward_to_leader(RequestOperation::KV(KVOperation::Cas {
                    key,
                    expected_revision,
                    value,
                    return_previous,
                }))
                .await
                .map_err(|e| SetError::Other(e))?
        } else {
            // Set operation
            distacean
                .write_or_forward_to_leader(RequestOperation::KV(KVOperation::Set {
                    key,
                    value,
                    return_previous,
                }))
                .await
                .map_err(|e| SetError::Other(e))?
        };

        // Extract SetResponse from response
        match response {
            crate::raft::Response::Result {
                res: crate::raft::ResponseResult::KV(crate::raft::KVResponse::Set(set_response)),
                ..
            } => Ok(set_response),
            crate::raft::Response::Result {
                res:
                    crate::raft::ResponseResult::KV(crate::raft::KVResponse::Cas {
                        success: true,
                        response,
                    }),
                ..
            } => Ok(response),
            crate::raft::Response::Result {
                res:
                    crate::raft::ResponseResult::KV(crate::raft::KVResponse::Cas {
                        success: false,
                        response,
                    }),
                ..
            } => Err(SetError::RevisionMismatch {
                current_revision: response.revision,
            }),
            _ => Err(SetError::Other("Unexpected response type".into())),
        }
    }
}

impl<S> SetRequestBuilder<S>
where
    S: State + set_request_builder::IsComplete,
{
    pub async fn execute(self) -> Result<SetResponse, SetError> {
        self.build().execute().await
    }
}

impl<S> SetRequestBuilder<S>
where
    S: State,
    <S as State>::ReturnPrevious: set_request_builder::IsUnset,
{
    pub fn with_previous(self) -> SetRequestBuilder<SetReturnPrevious<S>> {
        self.return_previous(true)
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
