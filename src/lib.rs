pub mod app;
pub mod client_http;
pub mod log_store;
pub mod network;
pub mod network_http;
pub mod store;

use std::sync::Arc;

use actix_web::HttpServer;
use actix_web::middleware;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use openraft::Config;

use crate::app::App;
use crate::network::api;
use crate::network::management;
use crate::network::raft;
use crate::store::Request;
use crate::store::Response;

pub type NodeId = u64;

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    pub TypeConfig:
        D = Request,
        R = Response,
);

pub type LogStore = store::LogStore;
pub type StateMachineStore = store::StateMachineStore;
pub type Raft = openraft::Raft<TypeConfig>;

pub async fn start_example_raft_node(node_id: NodeId, http_addr: String) -> std::io::Result<()> {
    // Create a configuration for the raft instance.
    let config = Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    // Create a instance of where the Raft logs will be stored.
    let log_store = LogStore::default();
    // Create a instance of where the Raft data will be stored.
    let state_machine_store = Arc::new(StateMachineStore::default());

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = network_http::NetworkFactory {};

    // Create a local raft instance.
    let raft = openraft::Raft::new(
        node_id,
        config.clone(),
        network,
        log_store.clone(),
        state_machine_store.clone(),
    )
    .await
    .unwrap();

    // Create an application that will store all the instances created above, this will
    // later be used on the actix-web services.
    let app_data = Data::new(App {
        id: node_id,
        addr: http_addr.clone(),
        raft,
        state_machine_store,
    });

    // Start the actix-web server.
    let log_format = Arc::new(format!(
        "[Node {}:{}] %a \"%r\" %s %b \"%{{User-Agent}}i\" %T",
        node_id,
        http_addr.split(':').last().unwrap_or("unknown")
    ));

    let server = HttpServer::new(move || {
        let fmt = log_format.clone();
        actix_web::App::new()
            .wrap(Logger::new(fmt.as_str()))
            .wrap(middleware::Compress::default())
            .app_data(app_data.clone())
            // raft internal RPC
            .service(raft::append)
            .service(raft::snapshot)
            .service(raft::vote)
            // admin API
            .service(management::init)
            .service(management::add_learner)
            .service(management::change_membership)
            .service(management::metrics)
            .service(management::get_linearizer)
            // application API
            .service(api::write)
            .service(api::read)
            .service(api::delete)
            .service(api::linearizable_read)
            .service(api::follower_read)
    });

    let x = server.bind(http_addr)?;

    x.run().await
}
