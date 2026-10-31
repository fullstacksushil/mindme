use eframe::egui::{
    self,
    load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

/// A fetched picture's bytes, or why it failed. `None` while the fetch runs.
type Fetch = Option<Result<Arc<[u8]>, String>>;

/// Pictures in notes, fetched from the web. egui's own loaders read only files and memory.
/// Each URL is fetched once per session, on its own thread.
#[derive(Default)]
pub struct WebImages {
    got: Arc<Mutex<HashMap<String, Fetch>>>,
}

impl BytesLoader for WebImages {
    fn id(&self) -> &str {
        "mindme::WebImages"
    }

    fn load(&self, ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if !uri.starts_with("https://") && !uri.starts_with("http://") {
            return Err(LoadError::NotSupported);
        }
        // The Wikipedia pictures are already in memory, so they are not fetched twice.
        if let ready @ Ok(_) = ctx.loaders().include.load(ctx, uri) {
            return ready;
        }
        let mut got = self.got.lock().unwrap();
        match got.get(uri) {
            Some(Some(Ok(bytes))) => Ok(BytesPoll::Ready { size: None, bytes: Bytes::Shared(bytes.clone()), mime: None }),
            Some(Some(Err(e))) => Err(LoadError::Loading(e.clone())),
            Some(None) => Ok(BytesPoll::Pending { size: None }),
            None => {
                got.insert(uri.to_owned(), None);
                let (got, uri, ctx) = (self.got.clone(), uri.to_owned(), ctx.clone());
                std::thread::spawn(move || {
                    let result = download(&uri);
                    got.lock().unwrap().insert(uri, Some(result));
                    ctx.request_repaint();
                });
                Ok(BytesPoll::Pending { size: None })
            }
        }
    }

    fn forget(&self, uri: &str) {
        self.got.lock().unwrap().remove(uri);
    }

    fn forget_all(&self) {
        self.got.lock().unwrap().clear();
    }

    fn byte_size(&self) -> usize {
        self.got.lock().unwrap().values().flatten().flatten().map(|b| b.len()).sum()
    }
}

fn download(uri: &str) -> Result<Arc<[u8]>, String> {
    if cfg!(test) {
        return Err("offline in tests".into());
    }
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).user_agent(super::fetch::AGENT).build().new_agent();
    agent.get(uri).call().and_then(|mut r| r.body_mut().read_to_vec()).map(Into::into).map_err(|e| e.to_string())
}
