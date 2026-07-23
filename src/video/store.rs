use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use super::VideoPipeline;

pub type PipelineStore =
    Rc<RefCell<HashMap<u32, Rc<VideoPipeline>>>>;

pub fn new_pipeline_store() -> PipelineStore {
    Rc::new(RefCell::new(HashMap::new()))
}