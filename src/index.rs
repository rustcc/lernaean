use crate::{
    crates::{CrateIdentity, CrateMetadata},
    errors::GenResult,
    magic::INDEX_QUERY_CACHE_SIZE,
    utils::CommandExt,
    GLOBAL_CONFIG,
};
use futures::{compat::Compat01As03, lock::Mutex};
use http::Uri;
use lru::LruCache;
use std::{path::Path, process::Command, time::Duration};

lazy_static! {
    static ref CRATE_CACHE: Mutex<LruCache<CrateIdentity, [u8; 32]>> =
        Mutex::new(LruCache::new(INDEX_QUERY_CACHE_SIZE));
}

#[allow(clippy::needless_lifetimes)] // see: https://github.com/rust-lang/rust-clippy/issues/3988
pub async fn query(ident: &CrateIdentity) -> GenResult<Option<[u8; 32]>> {
    if let Some(checksum) = CRATE_CACHE.lock().await.get(ident) {
        return Ok(Some(*checksum));
    }

    let rel_path = match ident.name.len() {
        0 => unreachable!(),
        1 => format!("1/{}", ident.name),
        2 => format!("2/{}", ident.name),
        3 => format!("3/{}/{}", &ident.name[..1], ident.name),
        _ => format!("{}/{}/{}", &ident.name[..2], &ident.name[2..4], ident.name),
    };
    let full_path = GLOBAL_CONFIG.index.join(rel_path);
    let content: Vec<u8> = Compat01As03::new(tokio_fs::read(full_path)).await?;
    let text = String::from_utf8(content)?;

    for line in text.lines() {
        let meta = serde_json::from_str::<CrateMetadata>(line)?;
        debug_assert_eq!(ident.name, meta.name);
        if meta.version == ident.version {
            CRATE_CACHE
                .lock()
                .await
                .put(ident.to_owned(), meta.checksum);
            return Ok(Some(meta.checksum));
        }
    }
    Err(format_err!(
        "no such crate found: {}-{}",
        ident.name,
        ident.version
    ))
}

pub fn init() -> GenResult<()> {
    let crate::Config {
        index,
        upstream,
        origin,
        dl,
        ..
    } = &*GLOBAL_CONFIG;

    init_index(index, upstream, origin, dl)?;

    std::thread::spawn(move || loop {
        if let Err(error) = pull_from_upstream(index) {
            error!("pull index failed: {:?}", error);
        } else if let Err(error) = push_to_origin(index) {
            error!("push index failed: {:?}", error);
        } else {
            info!("update index succeeded");
        }
        std::thread::sleep(Duration::from_secs(GLOBAL_CONFIG.interval))
    });

    Ok(())
}

fn init_index(index: &Path, upstream: &str, origin: &str, dl: &Uri) -> GenResult<()> {
    if index.join(".git").exists() {
        return Ok(());
    }

    Command::new("git")
        .arg("clone")
        .arg(upstream.to_string())
        .arg(index)
        .arg("--origin")
        .arg("upstream")
        .checked_call()?;

    Command::new("git")
        .current_dir(index)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(origin.to_string())
        .checked_call()?;

    let config_path = &index.join("config.json");
    let config = &file::get_text(config_path)?;
    let mut doc = serde_json::from_str::<serde_json::Value>(config)?;
    doc["dl"] = serde_json::Value::String(dl.to_string());
    file::put_text(config_path, serde_json::to_string_pretty(&doc)?)?;

    Command::new("git")
        .current_dir(index)
        .arg("commit")
        .arg("--all")
        .arg("--message")
        .arg("update download url")
        .arg("--author")
        .arg("DCjanus<dcjanus@dcjanus.com>")
        .checked_call()?;

    Ok(())
}

fn pull_from_upstream(index: &Path) -> GenResult<()> {
    Command::new("git")
        .current_dir(index)
        .arg("fetch")
        .arg("upstream")
        .arg("--quiet")
        .checked_call()?;

    Command::new("git")
        .current_dir(index)
        .arg("rebase")
        .arg("upstream/master")
        .arg("master")
        .arg("--quiet")
        .checked_call()?;

    Ok(())
}

fn push_to_origin(index: &Path) -> GenResult<()> {
    Command::new("git")
        .current_dir(index)
        .arg("push")
        .arg("--force")
        .arg("origin")
        .arg("--quiet")
        .checked_call()?;

    Ok(())
}
