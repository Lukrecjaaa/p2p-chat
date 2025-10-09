use super::UIMode;

mod input;
mod render;

pub struct LogMode {
    input_history: Vec<String>,
    history_index: Option<usize>,
}

impl LogMode {
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
        }
    }
}
