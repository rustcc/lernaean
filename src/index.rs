use crate::{
    crates::{CrateIdentity, CrateMetadata},
    errors::GenResult,
    utils::CommandExt,
    GLOBAL_CONFIG,
};
use http::Uri;
use std::{
    collections::HashMap,
    path::Path,
    process::Command,
    sync::{Arc, RwLock},
    time::Duration,
};

lazy_static! {
    pub static ref CRATES: Arc<RwLock<HashMap<CrateIdentity, [u8; 32]>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

pub fn query(ident: &CrateIdentity) -> Option<[u8; 32]> {
    CRATES.read().unwrap().get(ident).copied()
}

fn fresh_crates_map() -> GenResult<()> {
    let mut result = vec![];

    for i in walkdir::WalkDir::new(&GLOBAL_CONFIG.index)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_str().unwrap().contains('.'))
        .filter(|x| {
            x.as_ref()
                .map(|dir_entry| dir_entry.file_type().is_file())
                .unwrap_or(false)
        })
    {
        let dir_entry = i?;

        for i in std::fs::read_to_string(dir_entry.path())?
            .lines()
            .map(|line| serde_json::from_str::<CrateMetadata>(line))
        {
            let meta = i?;
            let CrateMetadata {
                name,
                version,
                checksum,
            } = meta;

            let ident = CrateIdentity { name, version };
            result.push((ident, checksum));
        }
    }

    let mut crates = CRATES.write().unwrap();
    for (ident, checksum) in result {
        crates.insert(ident, checksum);
    }

    Ok(())
}

pub fn init() -> GenResult<()> {
    let crate::Config {
        index,
        upstream_index,
        origin,
        dl,
        ..
    } = &*GLOBAL_CONFIG;

    init_index(index, upstream_index, origin, dl)?;
    fresh_crates_map()?;

    std::thread::spawn(move || loop {
        if let Err(error) = pull_from_upstream(index) {
            error!("pull index failed: {:?}", error);
        } else if let Err(error) = push_to_origin(index) {
            error!("push index failed: {:?}", error);
        } else if let Err(error) = fresh_crates_map() {
            error!("fresh crates failed: {:?}", error);
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
    let config = &std::fs::read_to_string(config_path)?;
    let mut doc = serde_json::from_str::<serde_json::Value>(config)?;
    doc["dl"] = serde_json::Value::String(dl.to_string());
    std::fs::write(config_path, serde_json::to_string_pretty(&doc)?)?;

    Command::new("git")
        .current_dir(index)
        .args(&["config", "--local", "user.email", "dcjanus@dcjanus.com"])
        .checked_call()?;

    Command::new("git")
        .current_dir(index)
        .args(&["config", "--local", "user.name", "DCjanus"])
        .checked_call()?;

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
