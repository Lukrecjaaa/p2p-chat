#[derive(Debug)]
pub enum UIAction {
    SendMessage(String, String),
    ExecuteCommand(String),
    Exit,
}
