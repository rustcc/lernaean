#![feature(async_await)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use crate::errors::GenResult;
use http::{header, StatusCode, Uri};
use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;
use tide::{error::ResultExt, middleware::RootLogger, EndpointResult};

pub mod errors;
pub mod index;
pub mod utils;

lazy_static! {
    pub static ref GLOBAL_CONFIG: Config = Config::from_args();
}

/// A simple crates.io mirror implement
#[derive(Debug, StructOpt)]
#[structopt(
    after_help = "Read more: https://doc.rust-lang.org/cargo/reference/registries.html#index-format"
)]
pub struct Config {
    /// Crates.io local index path
    #[structopt(long, value_name = "path", default_value = "./cache")]
    pub index: PathBuf,

    /// Upstream index url
    #[structopt(
        long,
        value_name = "url",
        default_value = "https://github.com/rust-lang/crates.io-index.git"
    )]
    pub upstream: Uri,

    /// Downstream index url
    #[structopt(long, value_name = "uri")]
    pub origin: Uri,

    /// Config.json 'dl' field
    #[structopt(long, value_name = "uri")]
    pub dl: Uri,

    /// Config.json 'api' field
    #[structopt(long, value_name = "uri", default_value = "https://crates.io")]
    pub api: Uri,

    /// Index update interval in seconds
    #[structopt(long, value_name = "seconds", default_value = "600")]
    pub interval: u64,

    /// The address server want to listen
    #[structopt(long, value_name = "address", default_value = "0.0.0.0:8000")]
    pub listen: SocketAddr,
}

pub fn init() -> GenResult<()> {
    flexi_logger::Logger::with_env_or_str("actix_web=debug,info")
        .format(|w, record| {
            write!(
                w,
                "[{}] {} [{}:{}] {}",
                chrono::Local::now().to_rfc3339(),
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

    crate::index::init()?;

    Ok(())
}

pub async fn download_view(context: tide::Context<()>) -> EndpointResult {
    let name: String = context.param("name").client_err()?;
    let version: semver::Version = context.param("version").client_err()?;

    let url = format!(
        "https://static-crates-io.proxy.ustclug.org/crates/{name}/{name}-{version}.crate",
        name = name,
        version = version
    );

    let response = http::response::Builder::new()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, url)
        .body(http_service::Body::empty())
        .server_err()?;
    Ok(response)
}

pub fn main() -> GenResult<()> {
    self::init()?;

    let mut app = tide::App::new(());
    app.middleware(RootLogger::new());
    app.at("/api/v1/crates/:name/:version/download")
        .get(download_view);
    app.serve(GLOBAL_CONFIG.listen)?;

    Ok(())
}
