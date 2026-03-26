// Processing pipeline — implemented in Task 4
use crate::types::{ProcessInput, ProcessOutput};

pub fn process(_input: &ProcessInput) -> ProcessOutput {
    ProcessOutput {
        top_hits: vec![],
        ddg_suggestions: vec![],
        local_suggestions: vec![],
        can_be_autocompleted: false,
    }
}
