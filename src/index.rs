use crate::{errors::GenResult, utils::CommandExt, GLOBAL_CONFIG};
use http::Uri;
use std::{path::Path, process::Command, time::Duration};

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
        if let Err(error) = pull_from_upstream(index).and_then(move |_| push_to_origin(index)) {
            error!("update index failed: {:?}", error);
        } else {
            info!("update index succeeded");
        }
        std::thread::sleep(Duration::from_secs(GLOBAL_CONFIG.interval))
    });

    Ok(())
}

fn init_index(index: &Path, upstream: &Uri, origin: &Uri, dl: &Uri) -> GenResult<()> {
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
