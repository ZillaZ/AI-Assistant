use std::collections::HashMap;

#[derive(Debug)]
pub struct Env {
    api_key: String,
    text_model: String,
    voice_model: String,
    pub context_size: u64,
    pub answer_max: u64,
    google_api_key: String,
    voice: String,
    project_id: String,
}

impl Env {
    pub fn new() -> Self {
        dotenvy::dotenv_override().unwrap();
        let vars: HashMap<String, String> = dotenvy::vars().collect::<HashMap<String, String>>();
        Self {
            api_key: vars.get("API_KEY").unwrap().into(),
            text_model: vars.get("TEXT_MODEL").unwrap().into(),
            voice_model: vars.get("VOICE_MODEL").unwrap().into(),
            context_size: vars.get("CONTEXT_SIZE").unwrap().parse::<u64>().unwrap(),
            answer_max: vars.get("ANSWER_MAX").unwrap().parse::<u64>().unwrap(),
            google_api_key: vars.get("GOOGLE_API_KEY").unwrap().into(),
            voice: vars.get("VOICE").unwrap().into(),
            project_id: vars.get("PROJECT_ID").unwrap().into(),
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

    pub fn voice(&self) -> String {
        self.voice.clone()
    }

    pub fn google_api_key(&self) -> String {
        self.google_api_key.clone()
    }

    pub fn project_id(&self) -> String {
        self.project_id.clone()
    }
}
