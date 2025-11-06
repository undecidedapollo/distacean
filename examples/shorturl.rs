use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use clap::Parser;
use distacean::{Distacean, DistaceanConfig};
use tracing_subscriber::EnvFilter;

struct AppState {
    distkv: distacean::DistKV,
}

async fn shorten(data: web::Data<AppState>, url: web::Json<String>) -> impl Responder {
    let hash = format!("{:x}", md5::compute(url.as_bytes()));
    let short = &hash[..6];

    if let Err(e) = data.distkv.set(short.to_string(), url.to_string()).await {
        return HttpResponse::InternalServerError()
            .body(format!("Failed to store URL in DistKV: {}", e));
    }

    HttpResponse::Ok().body(format!("http://localhost:8080/l/{}", short))
}

async fn redirect(data: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let url = match data.distkv.eventual_read(path.into_inner()).await {
        Ok(url) => url,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to retrieve URL from DistKV: {}", e));
        }
    };

    if url.is_empty() {
        return HttpResponse::NotFound().body("URL not found");
    }

    HttpResponse::Found()
        .append_header(("Location", url.as_str()))
        .finish()
}

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Opt {
    #[clap(long)]
    pub id: u64,

    #[clap(long)]
    pub tcp_port: u16,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Parse the parameters passed by arguments.
    let options = Opt::parse();

    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(false)
        .with_level(true)
        .with_ansi(false)
        .with_env_filter(EnvFilter::from_default_env())
        .fmt_fields(tracing_subscriber::fmt::format::DefaultFields::new())
        .init();

    tracing::info!("Starting node {} on {}", options.id, options.tcp_port);

    let distacean = Distacean::init(DistaceanConfig {
        node_id: options.id,
        tcp_port: options.tcp_port,
        nodes: vec![
            (1, "127.0.0.1:22001".to_owned()),
            // (2, "127.0.0.1:22002".to_owned()),
            // (3, "127.0.0.1:22003".to_owned()),
        ],
    })
    .await
    .map_err(|e| {
        eprintln!("Failed to initialize Distacean: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "Distacean initialization failed")
    })?;

    // Sleep for a second to warm up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let kv = distacean.kv_store();
    let app_state = web::Data::new(AppState { distkv: kv });

    println!("Starting URL shortener at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/shorten", web::post().to(shorten))
            .route("/l/{hash}", web::get().to(redirect))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
