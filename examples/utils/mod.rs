use distacean::{Distacean, NodeId, SingleNodeDistaceanConfig};

pub async fn ephemeral_distacian_cluster() -> Result<Distacean, std::io::Error> {
    let node_id = rand::random::<NodeId>() % 10000 + 1;
    Distacean::init_single_node_cluster(SingleNodeDistaceanConfig { node_id })
        .await
        .map_err(|e| {
            eprintln!("Failed to initialize Distacean: {}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Distacean initialization failed")
        })
}
