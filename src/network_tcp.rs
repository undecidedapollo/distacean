use std::{sync::Arc, time::Duration};

use crate::{peernet::StartableStream, protocol::RequestType};
use openraft::{
    BasicNode, RaftNetwork, RaftNetworkFactory, RaftTypeConfig,
    error::{InstallSnapshotError, RPCError, RaftError, Unreachable},
    network::RPCOption,
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};
use tokio::{
    io::{AsyncRead, AsyncSeek, AsyncWrite},
    net::TcpStream,
    time::sleep,
};

use crate::peernet::{PeerConnection, PeerManager};

pub struct TcpStreamStarter {}

impl StartableStream<TcpStream> for TcpStreamStarter {
    fn connect(&self, addr: String) -> impl std::future::Future<Output = TcpStream> + Send {
        async move {
            loop {
                match TcpStream::connect(&addr).await {
                    Ok(stream) => {
                        if let Err(e) = stream.set_nodelay(true) {
                            eprintln!("nodelay: {e}");
                        }
                        return stream;
                    }
                    Err(_) => sleep(Duration::from_secs(5)).await,
                }
            }
        }
    }
}

pub struct RaftPeerManager {
    inner: Arc<PeerManager<TcpStream, TcpStreamStarter>>,
}

impl RaftPeerManager {
    pub fn new(mgr: Arc<PeerManager<TcpStream, TcpStreamStarter>>) -> Self {
        RaftPeerManager { inner: mgr.clone() }
    }
}

impl<C> RaftNetworkFactory<C> for RaftPeerManager
where
    C: RaftTypeConfig<Node = BasicNode>,
    // RaftNetworkV2 is implemented automatically for RaftNetwork, but requires the following trait bounds.
    // In V2 network, the snapshot has no constraints, but RaftNetwork assumes a Snapshot is a file-like
    // object that can be seeked, read from, and written to.
    <C as RaftTypeConfig>::SnapshotData: AsyncRead + AsyncWrite + AsyncSeek + Unpin,
{
    type Network = RaftPeerNetwork<C>;

    async fn new_client(&mut self, target: C::NodeId, node: &BasicNode) -> Self::Network {
        // unimplemented!("test");
        let addr = node.addr.clone();
        let port = addr.split(':').last().and_then(|s| s.parse().ok()).unwrap();
        let x: Arc<PeerConnection<TcpStream, TcpStreamStarter>> =
            self.inner.get_or_create_connection(port).await;

        RaftPeerNetwork {
            port,
            target,
            inner: x,
        }
    }
}

pub struct RaftPeerNetwork<C>
where
    C: RaftTypeConfig,
{
    #[allow(dead_code)]
    port: u16,
    target: C::NodeId,
    inner: Arc<PeerConnection<TcpStream, TcpStreamStarter>>,
}

#[allow(clippy::blocks_in_conditions)]
impl<C> RaftNetwork<C> for RaftPeerNetwork<C>
where
    C: RaftTypeConfig,
{
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<C>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<C>, RPCError<C, RaftError<C>>> {
        let bytes = rmp_serde::to_vec(&req).unwrap();
        let req_bytes = rmp_serde::to_vec(&RequestType::AppendEntriesRequest(bytes)).unwrap();
        let res = self.inner.clone().req_res(req_bytes).await.map_err(|e| {
            let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
            RPCError::Unreachable(Unreachable::new(&io_err))
        })?;
        let resp: Result<AppendEntriesResponse<C>, RaftError<C>> =
            rmp_serde::from_slice(&res).unwrap();
        match resp {
            Ok(x) => Ok(x),
            Err(x) => Err(RPCError::RemoteError(openraft::error::RemoteError::new(
                self.target.clone(),
                x,
            ))),
        }
    }

    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<C>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<C>, RPCError<C, RaftError<C, InstallSnapshotError>>> {
        let bytes = rmp_serde::to_vec(&req).unwrap();
        let req_bytes = rmp_serde::to_vec(&RequestType::InstallSnapshotRequest(bytes)).unwrap();
        let res = self.inner.clone().req_res(req_bytes).await.map_err(|e| {
            let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
            RPCError::Unreachable(Unreachable::new(&io_err))
        })?;
        let resp: Result<InstallSnapshotResponse<C>, RaftError<C, InstallSnapshotError>> =
            rmp_serde::from_slice(&res).unwrap();
        match resp {
            Ok(x) => Ok(x),
            Err(x) => Err(RPCError::RemoteError(openraft::error::RemoteError::new(
                self.target.clone(),
                x,
            ))),
        }
    }

    async fn vote(
        &mut self,
        req: VoteRequest<C>,
        _option: RPCOption,
    ) -> Result<VoteResponse<C>, RPCError<C, RaftError<C>>> {
        let bytes = rmp_serde::to_vec(&req).unwrap();
        let req_bytes = rmp_serde::to_vec(&RequestType::VoteRequest(bytes)).unwrap();
        let res = self.inner.clone().req_res(req_bytes).await.map_err(|e| {
            let io_err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
            RPCError::Unreachable(Unreachable::new(&io_err))
        })?;
        let resp: Result<VoteResponse<C>, RaftError<C>> = rmp_serde::from_slice(&res).unwrap();
        match resp {
            Ok(x) => Ok(x),
            Err(x) => Err(RPCError::RemoteError(openraft::error::RemoteError::new(
                self.target.clone(),
                x,
            ))),
        }
    }
}
