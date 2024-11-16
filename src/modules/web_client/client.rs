use crate::modules::env::env::Env;
use crate::modules::web_client::types::*;
use reqwest::blocking::Client;

pub struct WebClient {
    env: Env,
    context: Vec<Message>,
    client: Client,
    text_completion_uri: String,
    speech_to_text_uri: String,
}

impl WebClient {
    pub fn new(env: Env) -> Self {
        let client = Client::new();
        Self {
            env,
            client,
            context: Vec::new(),
            text_completion_uri: "https://api.groq.com/openai/v1/chat/completions".into(),
            speech_to_text_uri: "https://api.groq.com/openai/v1/audio/transcriptions".into(),
        }
    }

    pub fn load_context(&mut self, context: Vec<Message>) {
        self.context = context;
    }

    pub fn make_request(&mut self, content: String) {
        let message = Message::new_user_message(&content);
        self.context.push(message);
        let request = GroqTextRequest::new(self.context.clone(), self.env.text_model());
        if let Ok(response) = self
            .client
            .post(&self.text_completion_uri)
            .json(&request)
            .send()
        {
            if let Ok(response) = response.json::<ApiResponse>() {
                println!("response is ok");
            }
        }
    }
}
