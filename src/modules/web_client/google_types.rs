use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SynthesisInput {
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "ssml")]
    SSML(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SsmlVoiceGender {
    #[serde(rename = "MALE")]
    Male,
    #[serde(rename = "FEMALE")]
    Female,
    #[serde(rename = "NEUTRAL")]
    Neutral,
    #[serde(rename = "SSML_VOICE_GENDER_UNSPECIFIED")]
    SsmlVoiceGenderUnspecified,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomVoiceParams {
    model: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceSelectionParams {
    #[serde(rename = "languageCode")]
    language_code: String,
    name: String,
}

impl VoiceSelectionParams {
    pub fn new(name: String) -> Self {
        Self {
            name,
            language_code: "pt-BR".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AudioEncoding {
    #[serde(rename = "LINEAR16")]
    Linear16,
    #[serde(rename = "MP3")]
    Mp3,
    #[serde(rename = "OGG_OPUS")]
    OggOpus,
    #[serde(rename = "MULAW")]
    Mulaw,
    #[serde(rename = "ALAW")]
    Alaw,
    #[serde(rename = "AUDIO_ENCODING_UNSPECIFIED")]
    AudioEncodingUnspecified,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: AudioEncoding,
}

impl AudioConfig {
    pub fn new(audio_encoding: AudioEncoding) -> Self {
        Self { audio_encoding }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceRequest {
    input: SynthesisInput,
    voice: VoiceSelectionParams,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

impl VoiceRequest {
    pub fn new(message: String, voice: String) -> Self {
        Self {
            input: SynthesisInput::Text(message),
            voice: VoiceSelectionParams::new(voice),
            audio_config: AudioConfig::new(AudioEncoding::Mp3),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VoiceResponse {
    #[serde(rename = "audioContent")]
    pub audio_content: String,
}
