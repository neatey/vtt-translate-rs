use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use std::collections::HashMap;
use uuid::Uuid;

static DEFAULT_ENDPOINT: &str = "https://api.cognitive.microsofttranslator.com";
static TRANSLATE_PATH: &str = "/translate";
static LANGUAGES_PATH: &str = "/languages";
static DEFAULT_VERSION: &str = "3.0";

// @@TODO Instead of hardcoding this enum, dynamically call the /languages?scope=translation endpoint to get the full list of supported languages
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, serde::Serialize, serde::Deserialize,
)]
pub enum Language {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "en-gb")]
    EnGB,
    #[serde(rename = "fa")]
    Fa,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(&self)
                .expect("Failed to serialize Language")
                .replace('"', "")
        )
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct TranslateRequestItem {
    text: String,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct TranslateResponseDetectedLanguage {
    language: Language,
    score: f32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TranslateResponseTranslation {
    #[serde(rename = "to")]
    _language: Language,
    text: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TranslateResponseItem {
    #[serde(rename = "detectedLanguage")]
    detected_language: Option<TranslateResponseDetectedLanguage>,
    translations: Vec<TranslateResponseTranslation>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum Direction {
    #[serde(rename = "ltr")]
    Ltr,
    #[serde(rename = "rtl")]
    Rtl,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LanguagesResponseLanguage {
    #[serde(rename = "name")]
    _name: String,
    #[serde(rename = "nativeName")]
    _native_name: String,
    #[serde(rename = "dir")]
    direction: Direction,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LanguagesResponse {
    translation: HashMap<String, LanguagesResponseLanguage>,
    //transliteration: Option<serde_json::Value>,
    //dictionary: Option<serde_json::Value>,
}

pub struct TranslationClient {
    endpoint: String,
    version: String,
    key: String,
    region: String,
}

impl TranslationClient {
    pub fn new(key: String, region: String) -> TranslationClient {
        TranslationClient {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            version: DEFAULT_VERSION.to_string(),
            key,
            region,
        }
    }

    async fn translation_languages(&self) -> Result<HashMap<String, LanguagesResponseLanguage>> {
        let params = vec![
            ("api-version", self.version.clone()),
            ("scope", "translation".to_string()),
        ];
        let url = reqwest::Url::parse_with_params(
            &format!("{}{}", self.endpoint, LANGUAGES_PATH),
            &params,
        )
        .with_context(|| "Failed to generate request URL with params")?;

        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .send()
            .await
            .with_context(|| "Error calling the Azure translation API /languages endpoint")?;

        if resp.status() != 200 {
            return Err(anyhow!(
                "Azure translation API /languates endpoint returned error response code {}",
                resp.status()
            ));
        };

        let resp_body = resp.json::<LanguagesResponse>().await.unwrap();

        Ok(resp_body.translation)
    }

    pub async fn translate(
        &self,
        sentences: Vec<String>,
        from: Option<Language>,
        to: Language,
    ) -> Result<(Language, Direction, Vec<String>)> {
        let mut params = vec![
            ("api-version", self.version.clone()),
            ("to", to.to_string()),
        ];
        if let Some(source_language) = from {
            params.push(("from", source_language.to_string()));
        }
        let url = reqwest::Url::parse_with_params(
            &format!("{}{}", self.endpoint, TRANSLATE_PATH),
            &params,
        )
        .with_context(|| "Failed to generate request URL with params")?;
        let req_body: Vec<TranslateRequestItem> = sentences
            .into_iter()
            .map(|s| TranslateRequestItem { text: s })
            .collect();

        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .json(&req_body)
            .header("Ocp-Apim-Subscription-Key", self.key.clone())
            .header("Ocp-Apim-Subscription-Region", self.region.clone())
            .header("X-ClientTraceId", Uuid::new_v4().to_string())
            .send()
            .await
            .with_context(|| "Error calling the Azure translation API")?;

        if resp.status() != 200 {
            return Err(anyhow!(
                "Azure translation API returned error response code {}",
                resp.status()
            ));
        };

        let resp_body = resp.json::<Vec<TranslateResponseItem>>().await.unwrap();

        let mut translated_sentences = vec![];
        let mut detected_language = TranslateResponseDetectedLanguage {
            language: Language::EnGB,
            score: 0.0,
        };
        if let Some(source_language) = from {
            detected_language.language = source_language;
            detected_language.score = 1.0;
        }
        for response_item in resp_body.into_iter() {
            if response_item
                .detected_language
                .unwrap_or(detected_language)
                .score
                > detected_language.score
            {
                detected_language = response_item.detected_language.unwrap();
            }

            // The response always contains a single translation in the language that we asked for
            assert_eq!(response_item.translations.len(), 1);
            assert_eq!(response_item.translations[0]._language, to);
            let mut sentence = response_item.translations[0].text.clone();

            // The translation API doesn't always return full sentences - add a fullstop if it is missing.
            if !sentence.ends_with('.') {
                sentence.push('.');
            }

            translated_sentences.push(sentence);
        }

        let direction = self
            .translation_languages()
            .await?
            .get(&to.to_string())
            .with_context(|| "Target language not returned by /languages endpoint")?
            .direction;

        Ok((detected_language.language, direction, translated_sentences))
    }
}
