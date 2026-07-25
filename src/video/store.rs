use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::VideoPipeline;

pub type PipelineStore = Arc<Mutex<HashMap<u32, Arc<VideoPipeline>>>>;

pub fn new_pipeline_store() -> PipelineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Stop and remove all pipelines. Call this during shutdown.
pub fn stop_all_pipelines(store: &PipelineStore) {
    let mut pipelines = store.lock().unwrap();
    for (id, pipeline) in pipelines.drain() {
        log::info!("[pipelines] stopping pipeline for device_id={}", id);
        pipeline.stop();
    }
}
