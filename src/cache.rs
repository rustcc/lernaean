use crate::{crates::CrateMetadata, errors::GenResult, GLOBAL_CONFIG};
use crossbeam_channel::{bounded, Receiver, Sender};
use reqwest::Client;
use sled::{IVec, Tree};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

// XXX Too many global variables
lazy_static! {
    static ref TASK_SET: Arc<Mutex<HashSet<CrateMetadata>>> =
        Arc::new(Mutex::new(HashSet::with_capacity(GLOBAL_CONFIG.tasks)));
    static ref TASK_SENDER: Sender<CrateMetadata> = {
        let (to_workers, from_manager) = bounded(GLOBAL_CONFIG.tasks);
        let (to_manager, from_workers) = bounded(GLOBAL_CONFIG.tasks);

        for _ in 0..GLOBAL_CONFIG.worker {
            let (from_manager, to_manager) = (from_manager.clone(), to_manager.clone());
            std::thread::spawn(move || {
                cache_fetch_worker(from_manager.clone(), to_manager.clone());
            });
        }

        std::thread::spawn(move || {
            for task in from_workers {
                if !TASK_SET.lock().unwrap().remove(&task) {
                    warn!("unexpected!")
                }
            }
        });

        to_workers
    };
    static ref TREE: Arc<Tree> = sled::Db::start_default(&GLOBAL_CONFIG.files)
        .unwrap()
        .open_tree("files")
        .unwrap();
    static ref CLIENT: Client = Client::new();
}

/// Query local cache
pub fn query(checksum: &[u8; 32]) -> Option<IVec> {
    match TREE.get(checksum) {
        Ok(x) => x,
        Err(error) => {
            error!("get cache failed: {:?}", error);
            None
        }
    }
}

/// Create task to fetch cache
pub fn fetch_cache(meta: CrateMetadata) {
    let mut tasks = TASK_SET.lock().unwrap();
    if tasks.len() >= GLOBAL_CONFIG.tasks {
        warn!("too many tasks are waiting");
        return;
    }
    if tasks.insert(meta.clone()) {
        TASK_SENDER.send(meta).unwrap();
    }
}

/// This function receive tasks from 'from_manager', download and put data to cache.
/// Regardless of success or failure, the processed tasks will be transmitted from the 'to_manager'.
fn cache_fetch_worker(from_manager: Receiver<CrateMetadata>, to_manager: Sender<CrateMetadata>) {
    fn inner(task: &CrateMetadata) -> GenResult<()> {
        let mut response = CLIENT.get(&task.upstream_url()).send()?;
        let mut buffer = Vec::with_capacity(1024 * 200);
        response.copy_to(&mut buffer)?;

        let actual = openssl::sha::sha256(&buffer);
        if actual != task.checksum {
            return Err(format_err!(
                "checksum error: expect {}, actual {}",
                hex::encode(&task.checksum),
                hex::encode(actual)
            ));
        }
        if TREE.set(&task.checksum, &buffer[..])?.is_some() {
            warn!("unexpected cache replace for {}", task);
        }
        Ok(())
    }
    for task in from_manager {
        if query(&task.checksum).is_some() {
            info!("skip cache fetch: {}", task);
            continue;
        }

        if let Err(error) = inner(&task) {
            error!("fetch cache failed: {:?}", error);
        } else {
            info!("cache fetch done: {}", task);
        }
        to_manager.send(task).unwrap();
    }
}
