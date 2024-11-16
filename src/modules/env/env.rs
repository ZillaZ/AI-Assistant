use std::collections::HashMap;

#[derive(Debug)]
pub struct Env {
    api_key: String,
    text_model: String,
    voice_model: String,
}

impl Env {
    pub fn new() -> Self {
        let vars: HashMap<String, String> = dotenvy::vars().collect::<HashMap<String, String>>();
        Self {
            api_key: vars.get("API_KEY").unwrap().into(),
            text_model: vars.get("TEXT_MODEL").unwrap().into(),
            voice_model: vars.get("VOICE_MODEL").unwrap().into(),
        }
    }

    pub fn text_model(&self) -> String {
        self.text_model.clone()
    }

    pub fn voice_model(&self) -> String {
        self.voice_model.clone()
    }

    pub fn api_key(&self) -> String {
        self.api_key.clone()
    }
}
