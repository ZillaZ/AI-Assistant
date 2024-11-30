use crate::modules::env::env::Env;
use crate::modules::web_client::{google_types::*, types::*};
use reqwest::blocking::Client;
use serde_json::json;

pub struct WebClient {
    env: Env,
    pub chat_id: String,
    context: Messages,
    client: Client,
    text_completion_uri: String,
    speech_to_text_uri: String,
    text_to_speech_uri: String,
}

impl WebClient {
    pub fn new() -> Self {
        let env = Env::new();
        let client = Client::new();
        Self {
            client,
            env,
            chat_id: String::new(),
            context: Messages::new(Vec::new()),
            text_completion_uri: "https://api.fireworks.ai/inference/v1/chat/completions".into(),
            speech_to_text_uri: "https://api.groq.com/openai/v1/audio/transcriptions".into(),
            text_to_speech_uri: "https://texttospeech.googleapis.com/v1/text:synthesize".into(),
        }
    }

    pub fn load_context(&mut self, context: Messages) {
        self.context = context;
        self.context.max_tokens = self.env.context_size;
        self.context.answer_tokens = self.env.answer_max;
    }

    pub fn new_message(&mut self, message: WebMessage) -> Option<Message> {
        self.env = Env::new();
        let content_len = message.message.content.as_ref().unwrap().len();
        self.context.push(message);
        let request = GroqTextRequest::new(
            self.context.get_window(content_len as u64),
            self.env.text_model(),
        );
        if let Ok(response) = self
            .client
            .post(&self.text_completion_uri)
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", self.env.api_key()))
            .json(&request)
            .send()
        {
            if response.status().is_success() {
                if let Ok(mut response) = response.json::<ApiResponse>() {
                    let message = response.choices.pop().unwrap().message;
                    let timestamp = response.created;
                    let id = uuid::Uuid::new_v4().to_string();
                    self.context
                        .push(WebMessage::new(message.clone(), timestamp as u64, id));
                    Some(message)
                } else {
                    println!("IS NOT APIRESPONSE");
                    None
                }
            } else {
                println!("ERROR:");
                return None;
            }
        } else {
            return None;
        }
    }

    pub fn new_audio(&mut self, message: String) -> Option<String> {
        self.env = Env::new();
        println!("AFTER ENV");
        let message = VoiceRequest::new(message, self.env.voice());
        let payload = json!(message).to_string();
        println!("getting new audio");

        if let Ok(response) = self
            .client
            .post(&self.text_to_speech_uri)
            .header("Content-Type", "application/json;")
            .header("x-goog-user-project", self.env.project_id())
            .bearer_auth(self.env.google_api_key().trim())
            .body(payload)
            .send()
        {
            if let Ok(content) = response.json::<VoiceResponse>() {
                return Some(content.audio_content);
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
}
