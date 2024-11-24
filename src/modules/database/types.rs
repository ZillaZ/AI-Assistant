use crate::modules::web_client::types::*;

pub enum DatabaseMessage {
    Chats(Vec<String>),
    Messages(String, Vec<WebMessage>),
    Email(String),
    Token(String),
    Timestamp(u64),
    Message(String),
    AudioPath(String),
    UserInfo(UserInfo),
    Ok,
    Err,
}

pub enum NetworkMessage {
    ChatRequest(String, String, String),
    LoginRequest(String, String, String),
    TokenValidation(String, String),
    NewChat(String, String),
    NewMessage(String, String, String, String, String, String),
    GetChats(String, String),
    DeleteChat(String, String, String),
    RegisterUser(String, String, String, String),
    GetMessage(String, String),
    GetAudioPath(String, String),
    RecordAudioPath(String, String),
}
