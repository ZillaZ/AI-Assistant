use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub content: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebMessage {
    pub message: Message,
    created_at: u64,
    pub id: String,
}

impl WebMessage {
    pub fn new(message: Message, created_at: u64, id: String) -> Self {
        Self {
            message,
            created_at,
            id,
        }
    }
}

impl Message {
    pub fn new<T: ToString>(role: T, content: T) -> Self {
        Self {
            content: Some(content.to_string()),
            role: Some(role.to_string()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: i32,
    #[serde(rename = "total_tokens")]
    pub total_tokens: i32,
    #[serde(rename = "completion_tokens")]
    pub completion_tokens: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: String,
    // Use `Option` if you want to handle cases where this field might not exist in the JSON.
    // pub logprobs: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiResponse {
    pub id: String,
    pub object: String,
    pub created: i32,
    pub model: String,
    // Use `Option` if `system_configuration` might not always be present in the JSON.
    // pub system_configuration: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Messages {
    pub answer_tokens: u64,
    used_tokens: u64,
    pub max_tokens: u64,
    messages: Vec<WebMessage>,
}

impl Messages {
    pub fn new(messages: Vec<WebMessage>) -> Self {
        Self {
            messages,
            answer_tokens: 0,
            max_tokens: 0,
            used_tokens: 0,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let message = &self.messages[0];
        let json = json!(message);
        json.to_string().as_bytes().to_vec()
    }

    pub fn push(&mut self, message: WebMessage) {
        self.messages.push(message);
    }

    pub fn get_window(&self, content_len: u64) -> Vec<WebMessage> {
        let max_size = self.max_tokens - (self.answer_tokens + content_len);
        let mut count = 0;
        let mut messages = Vec::new();
        for message in self.messages.iter().rev() {
            if count >= max_size {
                break;
            }

            count += json!(message).to_string().len() as u64;
            messages.push(message.clone());
        }
        messages.reverse();
        messages
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroqTextRequest {
    model: String,
    messages: Vec<Message>,
}

impl GroqTextRequest {
    pub fn new(messages: Vec<WebMessage>, model: String) -> Self {
        Self {
            model,
            messages: messages.iter().map(|x| x.message.clone()).collect::<_>(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Error {
    r#type: String,
    message: String,
}
