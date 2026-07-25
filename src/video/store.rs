use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::VideoPipeline;

pub type PipelineStore = Arc<Mutex<HashMap<u32, Arc<VideoPipeline>>>>;

pub fn new_pipeline_store() -> PipelineStore {
    Arc::new(Mutex::new(HashMap::new()))
}
