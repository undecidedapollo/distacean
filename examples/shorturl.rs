use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use clap::{Parser, Subcommand};
use distacean::{ClusterDistaceanConfig, Distacean, NodeId, ReadConsistency, ReadSource};
use tracing_subscriber::EnvFilter;

use crate::utils::ephemeral_distacian_cluster;
mod utils;

struct AppState {
    distkv: distacean::DistKV,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ShortenRequest {
    url: String,
    hash: String,
}

async fn shorten(data: web::Data<AppState>, url: web::Json<String>) -> impl Responder {
    let hash = format!("{:x}", md5::compute(url.as_bytes()));
    let short = &hash[..6];

    if let Err(e) = data
        .distkv
        .set(
            short,
            ShortenRequest {
                url: url.into_inner(),
                hash: short.to_string(),
            },
        )
        .execute()
        .await
    {
        return HttpResponse::InternalServerError()
            .body(format!("Failed to store URL in DistKV: {}", e));
    }

    HttpResponse::Ok().body(format!("http://localhost:8080/l/{}", short))
}

async fn redirect(data: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let url: ShortenRequest = match data
        .distkv
        .read(path.as_str())
        .consistency(ReadConsistency::AsIs)
        .source(ReadSource::Local)
        .execute()
        .await
    {
        Ok(Some(url)) => url,
        Ok(None) => {
            return HttpResponse::InternalServerError().body(format!("Url not found in DistKV"));
        }
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to retrieve URL from DistKV: {}", e));
        }
    };

    HttpResponse::Found()
        .append_header(("Location", url.url.as_str()))
        .finish()
}

#[derive(Parser, Debug)]
#[command(name = "distacean", subcommand = "Ephemeral")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the server
    Cluster {
        #[arg(long)]
        tcp_port: u16,
        #[arg(long)]
        node_id: NodeId,

        #[arg(long)]
        #[clap(default_value = "8000")]
        http_port: u16,
    },

    Ephemeral {
        #[arg(long)]
        #[clap(default_value = "8000")]
        http_port: u16,
    },
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(false)
        .with_level(true)
        .with_ansi(false)
        .with_env_filter(EnvFilter::from_default_env())
        .fmt_fields(tracing_subscriber::fmt::format::DefaultFields::new())
        .init();

    let (distacean, http_port) = match cli.command {
        Commands::Cluster {
            tcp_port,
            node_id,
            http_port,
        } => (
            Distacean::init(ClusterDistaceanConfig {
                node_id,
                tcp_port,
                nodes: vec![
                    (1, "127.0.0.1:22001".to_string()),
                    (2, "127.0.0.1:22002".to_string()),
                    (3, "127.0.0.1:22003".to_string()),
                ],
            })
            .await
            .map_err(|e| {
                eprintln!("Failed to initialize Distacean: {}", e);
                std::io::Error::new(std::io::ErrorKind::Other, "Distacean initialization failed")
            })?,
            http_port,
        ),
        Commands::Ephemeral { http_port } => (ephemeral_distacian_cluster().await?, http_port),
    };

    // Sleep for a second to warm up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let kv = distacean.kv_store();
    let app_state = web::Data::new(AppState { distkv: kv });

    println!("Starting URL shortener at http://localhost:{http_port}");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/shorten", web::post().to(shorten))
            .route("/l/{hash}", web::get().to(redirect))
    })
    .bind(format!("127.0.0.1:{http_port}"))?
    .run()
    .await
}
