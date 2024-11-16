use crate::modules::web_client::types::*;

pub enum DatabaseMessage {
    Chats(Vec<String>),
    Messages(Vec<Message>),
    Email(String),
    Ok,
    Err,
}

pub enum NetworkMessage {
    ChatRequest(String, String),
    LoginRequest(String, String, String),
    TokenValidation(String, String),
    NewChat(String, String),
}
