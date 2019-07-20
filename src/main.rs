#![feature(async_await)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use crate::{
    crates::{CrateIdentity, CrateMetadata},
    errors::GenResult,
};
use http::{StatusCode, Uri};
use std::{net::SocketAddr, num::NonZeroU64, path::PathBuf};
use structopt::StructOpt;
use tide::{error::ResultExt, response::IntoResponse, EndpointResult};

mod cache;
mod crates;
mod errors;
mod index;
mod logger;
mod pubsub;
mod utils;

lazy_static! {
    static ref GLOBAL_CONFIG: Config = Config::from_args();
}

/// A simple crates.io mirror implement
#[derive(Debug, StructOpt)]
#[structopt(
    after_help = "Read more: https://doc.rust-lang.org/cargo/reference/registries.html#index-format"
)]
struct Config {
    /// Crates.io local index path
    #[structopt(long, value_name = "path", default_value = "./cache/crates.io-index")]
    pub index: PathBuf,

    /// Crates local cache path
    #[structopt(long, value_name = "path", default_value = "./cache/crates.sled")]
    pub files: PathBuf,

    /// Upstream registry index url
    #[structopt(
        long,
        value_name = "uri",
        default_value = "https://github.com/rust-lang/crates.io-index.git"
    )]
    pub upstream_index: String,

    /// Upstream registry dl url, see https://doc.rust-lang.org/cargo/reference/registries.html#index-format, but named parameters are required
    #[structopt(
        long,
        value_name = "url",
        default_value = "https://crates.io/api/v1/crates/{crate}/{version}/download"
    )]
    pub upstream_dl: String,

    /// Downstream registry index url
    #[structopt(long, value_name = "uri")]
    pub origin: String,

    /// Config.json 'dl' field
    #[structopt(long, value_name = "uri")]
    pub dl: Uri,

    /// Config.json 'api' field
    #[structopt(long, value_name = "uri", default_value = "https://crates.io")]
    pub api: Uri,

    /// Index update interval in seconds
    #[structopt(long, value_name = "seconds", default_value = "600")]
    pub interval: NonZeroU64,

    /// The address server want to listen
    #[structopt(long, value_name = "address", default_value = "0.0.0.0:8000")]
    pub listen: SocketAddr,

    /// Number of cache fetch threads
    #[structopt(long, default_value = "8")]
    pub worker: usize,

    /// Maximum number of tasks waiting
    #[structopt(long, default_value = "65536")]
    pub tasks: usize,

    /// Interval of prefetch
    #[structopt(long, value_name = "millis")]
    pub prefetch_interval: Option<u64>,
}

fn init() -> GenResult<()> {
    flexi_logger::Logger::with_env_or_str("actix_web=debug,info")
        .format(|w, now, record| {
            write!(
                w,
                "[{}] {} [{}:{}] {}",
                now.now().to_rfc3339(),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                record
                    .line()
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string()),
                &record.args()
            )
        })
        .start()?;

    lazy_static::initialize(&GLOBAL_CONFIG);

    crate::crates::init()?;

    crate::index::init()?;

    crate::cache::init()?;

    Ok(())
}

async fn download_view(context: tide::Context<()>) -> EndpointResult {
    let name: String = context.param("name").client_err()?;
    let version: String = context.param("version").client_err()?;

    let ident = CrateIdentity { name, version };
    let checksum = match crate::index::query(&ident) {
        Some(checksum) => checksum,
        None => return Ok(StatusCode::NOT_FOUND.into_response()),
    };

    let CrateIdentity { name, version } = ident;
    let meta = CrateMetadata {
        name,
        version,
        checksum,
    };
    match crate::cache::get(meta).await {
        Ok(data) => http::response::Builder::new()
            .status(StatusCode::OK)
            .body(http_service::Body::from(&*data))
            .server_err(),
        Err(error) => {
            error!("{}", error);
            http::response::Builder::new()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("unexpected error".into())
                .server_err()
        }
    }
}

fn main() -> GenResult<()> {
    self::init()?;

    let mut app = tide::App::new(());
    app.middleware(crate::logger::Logger);
    app.at("/api/v1/crates/:name/:version/download")
        .get(download_view);
    app.serve(GLOBAL_CONFIG.listen)?;

    Ok(())
}
