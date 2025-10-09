use super::completers::ChatCompleter;

mod input;
mod render;

pub struct ChatMode {
    input_history: Vec<String>,
    history_index: Option<usize>,
    completer: ChatCompleter,
    current_suggestion: Option<String>,
}

impl ChatMode {
    pub fn new() -> Self {
        Self {
            input_history: Vec::new(),
            history_index: None,
            completer: ChatCompleter::new(Vec::new()),
            current_suggestion: None,
        }
    }

    pub fn update_friends(&mut self, friends: Vec<String>) {
        self.completer.update_friends(friends);
    }

    pub fn update_discovered_peers(&mut self, peers: Vec<String>) {
        self.completer.update_discovered_peers(peers);
    }

    pub fn get_current_suggestion(&self) -> Option<&str> {
        self.current_suggestion.as_deref()
    }
}
