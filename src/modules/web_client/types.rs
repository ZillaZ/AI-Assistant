use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenericMessage {
    content: Option<String>,
    name: Option<String>,
    role: Option<String>,
    tool_call_id: Option<String>,
}

impl GenericMessage {
    pub fn new<T: ToString>(role: T, content: T) -> Self {
        Self {
            content: Some(content.to_string()),
            role: Some(role.to_string()),
            name: None,
            tool_call_id: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Message {
    SystemMessage(GenericMessage),
    UserMessage(GenericMessage),
    AssistantMessage(GenericMessage),
}

impl Message {
    pub fn new_user_message(content: &str) -> Self {
        Message::UserMessage(GenericMessage::new("user", content))
    }
}

#[derive(Deserialize, Serialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub prompt_time: f32,
    pub completion_time: f32,
    pub total_time: f32,
}

#[derive(Deserialize, Serialize)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: String,
    pub logprobs: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct ApiResponse {
    pub id: String,
    pub object: String,
    pub created: i32,
    pub model: String,
    pub system_configuration: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroqTextRequest {
    messages: Vec<Message>,
    model: String,
}

impl GroqTextRequest {
    pub fn new(messages: Vec<Message>, model: String) -> Self {
        Self { messages, model }
    }
}

#[derive(Deserialize, Serialize)]
pub struct ClientRequest {
    token: String,
}
