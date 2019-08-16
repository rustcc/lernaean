use crate::{
    crates::{CrateIdentity, CrateMetadata},
    errors::GenResult,
    utils::CommandExt,
    GLOBAL_CONFIG,
};
use http::Uri;
use std::{path::Path, process::Command, time::Duration};

// from crate name and version to checksum
pub fn query(ident: &CrateIdentity) -> Option<[u8; 32]> {
    let name = ident.name.to_lowercase();

    let raw_path = match name.len() {
        0 => return None,
        1 => format!("1/{}", name),
        2 => format!("2/{}", name),
        3 => format!("3/{}/{}", &name[..1], name),
        _ => format!("{}/{}/{}", &name[..2], &name[2..4], name),
    };

    let real_path = GLOBAL_CONFIG.index.join(raw_path);
    if !real_path.exists() {
        return None;
    }

    std::fs::read_to_string(real_path)
        .ok()?
        .lines()
        .filter_map(|x| serde_json::from_str::<CrateMetadata>(x).ok())
        .find(|x: &CrateMetadata| x.version == ident.version)
        .map(|x| x.checksum)
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

    std::thread::spawn(move || loop {
        if let Err(error) = pull_from_upstream(index) {
            error!("pull index failed: {:?}", error);
        } else if let Err(error) = push_to_origin(index) {
            error!("push index failed: {:?}", error);
        } else {
            info!("update index succeeded");
        }

        std::thread::sleep(Duration::from_secs(GLOBAL_CONFIG.interval.get()))
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
