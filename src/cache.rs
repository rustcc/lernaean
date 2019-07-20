use crate::{
    crates::{upstream_url, CrateMetadata},
    errors::GenResult,
    pubsub::{Publisher, Subscriber},
    utils::BytesSize,
    GLOBAL_CONFIG,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use reqwest::Client;
use sled::{IVec, Tree};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

// XXX Too many global variables
lazy_static! {
    static ref TASKS: Arc<Mutex<HashMap<CrateMetadata, Subscriber>>> =
        Arc::new(Mutex::new(HashMap::with_capacity(GLOBAL_CONFIG.tasks)));
    static ref TASK_SENDER: Sender<(CrateMetadata, Publisher)> = {
        let (sender, receiver) = bounded(GLOBAL_CONFIG.tasks);

        for id in 0..GLOBAL_CONFIG.worker {
            let receiver = receiver.clone();
            std::thread::spawn(move || {
                cache_fetch_worker(id, receiver);
            });
        }

        sender
    };
    static ref TREE: Arc<Tree> = sled::Db::start_default(&GLOBAL_CONFIG.files)
        .unwrap()
        .open_tree("files")
        .unwrap();
}

pub fn init() -> GenResult<()> {
    Ok(())
}

pub async fn get(meta: CrateMetadata) -> GenResult<IVec> {
    let checksum = meta.checksum;
    if let Some(data) = query(&checksum) {
        return Ok(data);
    }

    fetch_cache(meta)?.await;

    if let Some(data) = query(&checksum) {
        Ok(data)
    } else {
        Err(format_err!("unexpected"))
    }
}

/// Query local cache
fn query(checksum: &[u8; 32]) -> Option<IVec> {
    match TREE.get(checksum) {
        Ok(x) => x,
        Err(error) => {
            error!("get cache failed: {:?}", error);
            None
        }
    }
}

/// Create task to fetch cache
fn fetch_cache(meta: CrateMetadata) -> GenResult<Subscriber> {
    let mut tasks = TASKS.lock().unwrap();
    if tasks.len() >= GLOBAL_CONFIG.tasks {
        return Err(format_err!("too many tasks are waiting"));
    }

    let subscriber = tasks
        .entry(meta.clone())
        .or_insert_with(|| {
            let (publisher, subscriber) = crate::pubsub::new_pair();
            TASK_SENDER.send((meta, publisher)).unwrap();
            subscriber
        })
        .clone();

    Ok(subscriber)
}

/// This function receive tasks from 'tasks', download and put data to cache.
fn cache_fetch_worker(id: usize, tasks: Receiver<(CrateMetadata, Publisher)>) {
    lazy_static! {
        // The crate file is gzip file, and some response from static.crates.io might contain header `content-encoding: gzip`,
        // for example: 'https://static.crates.io/crates/google-discovery1/google-discovery1-0.1.5+00000000.crate'.
        // Default `reqwest::Client` decompress it, which means that we got the extracted crate file,
        // so we should turn off auto gzip decompression
        // see: https://github.com/rust-lang/crates.io/issues/1179
        static ref CLIENT: Client = Client::builder().gzip(false).build().unwrap();
    }

    fn inner(task: &CrateMetadata) -> GenResult<usize> {
        let mut response = CLIENT
            .get(&upstream_url(&task.name, &task.version))
            .send()?;
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
        Ok(buffer.len())
    }

    for (task, publisher) in tasks {
        if query(&task.checksum).is_some() {
            info!("skip cache fetch: {}", task);
            continue;
        }

        let begin = std::time::Instant::now();

        match inner(&task) {
            Ok(size) => info!(
                "{}/{:?} fetch cache done: {}",
                BytesSize(size),
                begin.elapsed(),
                task
            ),
            Err(error) => {
                error!(
                    "{:?}@{} fetch cache failed: {:?}",
                    begin.elapsed(),
                    id,
                    error
                );
            }
        }
        publisher.finish();
        if TASKS.lock().unwrap().remove(&task).is_none() {
            warn!("unexpected!");
        }
    }
}
