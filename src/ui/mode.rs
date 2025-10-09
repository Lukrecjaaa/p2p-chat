use tracing::Level;

#[derive(Debug, Clone)]
pub enum UIMode {
    Chat,
    Logs {
        filter: Option<String>,
        level: Level,
    },
}

impl Default for UIMode {
    fn default() -> Self {
        Self::Chat
    }
}
