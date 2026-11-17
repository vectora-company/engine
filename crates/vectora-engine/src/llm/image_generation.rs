//! Image generation over the OpenAI Images and OpenRouter image protocols.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::chat::{MediaExecutionContext, MediaKind, MediaRef, classify, sha256_hex};

use super::{LlmError, http::ProviderHttpClient, http::ProviderHttpSettings};

pub const IMAGE_GENERATION_CONTRACT_VERSION: u16 = 1;
pub const DEFAULT_MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const OPENAI_URL: &str = "https://api.openai.com/v1";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";

/// Image APIs have different authentication and response behavior; the
/// OpenAI-compatible profile is opt-in and does not imply universal support.
#[derive(Deserialize)]
#[serde(tag = "protocol", content = "settings", rename_all = "snake_case")]
pub enum ImageGenerationClientSettings {
    OpenAi(ProviderHttpSettings),
    OpenAiCompatible(ProviderHttpSettings),
    OpenRouter(ProviderHttpSettings),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageOutputMode {
    /// Return bytes only for the lifetime of this response.
    #[default]
    ReturnBytes,
    /// Write bytes through the consumer-owned writable media store.
    StoreInMediaContext,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageGenerationRequest {
    pub contract_version: u16,
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_count")]
    pub count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default)]
    pub output_mode: ImageOutputMode,
    #[serde(default = "default_max_image_bytes")]
    pub max_image_bytes: u64,
}

const fn default_count() -> u8 {
    1
}

fn default_output_format() -> String {
    "png".to_owned()
}

const fn default_max_image_bytes() -> u64 {
    DEFAULT_MAX_IMAGE_BYTES
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GeneratedImage {
    pub sha256: String,
    pub mime_type: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_ref: Option<MediaRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ImageGenerationResponse {
    pub contract_version: u16,
    pub images: Vec<GeneratedImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
}

struct ProviderImage {
    bytes: Vec<u8>,
    mime_type: String,
    revised_prompt: Option<String>,
}

enum ImageProtocol {
    Official(ProviderHttpClient),
    Compatible(ProviderHttpClient),
    Router(ProviderHttpClient),
}

pub struct ImageGenerationClient {
    protocol: ImageProtocol,
}

impl ImageGenerationClient {
    pub fn new(settings: ImageGenerationClientSettings) -> Result<Self, LlmError> {
        let protocol = match settings {
            ImageGenerationClientSettings::OpenAi(settings) => {
                ImageProtocol::Official(ProviderHttpClient::new(settings, OPENAI_URL, true)?)
            }
            ImageGenerationClientSettings::OpenAiCompatible(settings) => {
                ImageProtocol::Compatible(ProviderHttpClient::new(settings, OPENAI_URL, false)?)
            }
            ImageGenerationClientSettings::OpenRouter(settings) => {
                ImageProtocol::Router(ProviderHttpClient::new(settings, OPENROUTER_URL, true)?)
            }
        };
        Ok(Self { protocol })
    }

    pub async fn generate(
        &self,
        request: ImageGenerationRequest,
        media: Option<&MediaExecutionContext>,
    ) -> Result<ImageGenerationResponse, LlmError> {
        validate_request(&request)?;
        if request.output_mode == ImageOutputMode::StoreInMediaContext && media.is_none() {
            return Err(LlmError::InvalidConfiguration(
                "storing generated images requires a media context",
            ));
        }

        let payload = request_payload(&request);
        let (client, path) = match &self.protocol {
            ImageProtocol::Official(client) => (client, "images/generations"),
            ImageProtocol::Compatible(client) => (client, "images/generations"),
            ImageProtocol::Router(client) => (client, "images"),
        };
        let response = client.post_json(path, payload).await?;
        let (provider_images, usage) = parse_response(response, &request)?;
        let mut images = Vec::with_capacity(provider_images.len());
        for image in provider_images {
            let sha256 = sha256_hex(&image.bytes);
            let size_bytes = image.bytes.len() as u64;
            let (bytes, media_ref) = match request.output_mode {
                ImageOutputMode::ReturnBytes => (Some(image.bytes), None),
                ImageOutputMode::StoreInMediaContext => {
                    let stored = media
                        .expect("validated above")
                        .store_generated_image(
                            image.bytes,
                            &image.mime_type,
                            &format!("generated-{sha256}.{}", extension(&image.mime_type)),
                        )
                        .await
                        .map_err(map_media_error)?;
                    (None, Some(stored))
                }
            };
            images.push(GeneratedImage {
                sha256,
                mime_type: image.mime_type,
                size_bytes,
                revised_prompt: image.revised_prompt,
                bytes,
                media_ref,
            });
        }
        Ok(ImageGenerationResponse {
            contract_version: IMAGE_GENERATION_CONTRACT_VERSION,
            images,
            usage,
        })
    }
}

fn validate_request(request: &ImageGenerationRequest) -> Result<(), LlmError> {
    if request.contract_version != IMAGE_GENERATION_CONTRACT_VERSION
        || request.model.trim().is_empty()
        || request.model.len() > 256
        || request.prompt.trim().is_empty()
        || request.prompt.len() > 200_000
        || !(1..=10).contains(&request.count)
        || request.max_image_bytes == 0
        || request.max_image_bytes > 100 * 1024 * 1024
        || !matches!(request.output_format.as_str(), "png" | "jpeg" | "webp")
        || request.quality.as_deref().is_some_and(|quality| {
            !matches!(
                quality,
                "auto" | "low" | "medium" | "high" | "xhigh" | "max"
            )
        })
        || request
            .size
            .as_deref()
            .is_some_and(|size| !valid_size(size))
    {
        return Err(LlmError::InvalidConfiguration(
            "invalid image generation request",
        ));
    }
    Ok(())
}

fn valid_size(size: &str) -> bool {
    if size == "auto" {
        return true;
    }
    let Some((width, height)) = size.split_once('x') else {
        return false;
    };
    let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>()) else {
        return false;
    };
    width > 0 && height > 0 && width <= 4096 && height <= 4096
}

fn request_payload(request: &ImageGenerationRequest) -> Value {
    let mut payload = json!({
        "model": request.model,
        "prompt": request.prompt,
        "n": request.count,
        "output_format": request.output_format,
    });
    if let Some(size) = &request.size {
        payload["size"] = json!(size);
    }
    if let Some(quality) = &request.quality {
        payload["quality"] = json!(quality);
    }
    payload
}

fn parse_response(
    response: Value,
    request: &ImageGenerationRequest,
) -> Result<(Vec<ProviderImage>, Option<Value>), LlmError> {
    #[derive(Deserialize)]
    struct WireResponse {
        data: Vec<WireImage>,
        #[serde(default)]
        usage: Option<Value>,
    }
    #[derive(Deserialize)]
    struct WireImage {
        b64_json: String,
        #[serde(default)]
        revised_prompt: Option<String>,
        #[serde(default)]
        media_type: Option<String>,
    }
    let response: WireResponse =
        serde_json::from_value(response).map_err(|_| LlmError::InvalidResponse)?;
    if response.data.is_empty() || response.data.len() > usize::from(request.count) {
        return Err(LlmError::InvalidResponse);
    }
    let mut images = Vec::with_capacity(response.data.len());
    for image in response.data {
        let bytes = STANDARD
            .decode(image.b64_json)
            .map_err(|_| LlmError::InvalidResponse)?;
        if bytes.is_empty() || bytes.len() as u64 > request.max_image_bytes {
            return Err(if bytes.len() as u64 > request.max_image_bytes {
                LlmError::ResponseTooLarge
            } else {
                LlmError::InvalidResponse
            });
        }
        let detected = classify(&bytes, image.media_type.as_deref());
        if detected.kind != MediaKind::Image
            || image
                .media_type
                .as_deref()
                .is_some_and(|mime| mime != detected.mime_type)
        {
            return Err(LlmError::InvalidResponse);
        }
        images.push(ProviderImage {
            bytes,
            mime_type: detected.mime_type,
            revised_prompt: image.revised_prompt,
        });
    }
    Ok((images, response.usage))
}

fn extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

fn map_media_error(error: crate::chat::MediaError) -> LlmError {
    match error {
        crate::chat::MediaError::TooLarge => LlmError::ResponseTooLarge,
        crate::chat::MediaError::UnsupportedType | crate::chat::MediaError::Integrity => {
            LlmError::InvalidResponse
        }
        crate::chat::MediaError::Unavailable => {
            LlmError::InvalidConfiguration("media store does not support generated assets")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::chat::{CapabilityResolver, MediaError, MediaStore};

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/Vb8AAAAASUVORK5CYII=";

    #[derive(Default)]
    struct MemoryStore(std::sync::Mutex<HashMap<String, Vec<u8>>>);

    #[async_trait]
    impl MediaStore for MemoryStore {
        async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError> {
            self.0
                .lock()
                .unwrap()
                .get(&media_ref.asset_id)
                .cloned()
                .ok_or(MediaError::Unavailable)
        }

        async fn write_generated(
            &self,
            bytes: Vec<u8>,
            mime_type: &str,
            name: &str,
        ) -> Result<MediaRef, MediaError> {
            let sha256 = sha256_hex(&bytes);
            let asset_id = format!("test-{sha256}");
            self.0
                .lock()
                .unwrap()
                .insert(asset_id.clone(), bytes.clone());
            Ok(MediaRef {
                asset_id,
                sha256,
                mime_type: mime_type.to_owned(),
                size_bytes: bytes.len() as u64,
                name: name.to_owned(),
            })
        }
    }

    fn request(output_mode: ImageOutputMode) -> ImageGenerationRequest {
        ImageGenerationRequest {
            contract_version: IMAGE_GENERATION_CONTRACT_VERSION,
            model: "image-model".into(),
            prompt: "a red panda".into(),
            count: 1,
            size: Some("1024x1024".into()),
            quality: Some("high".into()),
            output_format: "png".into(),
            output_mode,
            max_image_bytes: 1024,
        }
    }

    async fn serve_once(listener: TcpListener, response: String) -> String {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&chunk[..read]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
        let content_length: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .unwrap()
            .parse()
            .unwrap();
        while request.len() < header_end + content_length {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&chunk[..read]);
        }
        let body = response.as_bytes();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        stream.write_all(body).await.unwrap();
        String::from_utf8_lossy(&request).into_owned()
    }

    fn settings(protocol: &str, address: std::net::SocketAddr) -> ImageGenerationClientSettings {
        let settings = ProviderHttpSettings {
            base_url: Some(format!("http://{address}/api/v1")),
            api_key: Some("test-key".into()),
            ..ProviderHttpSettings::default()
        };
        match protocol {
            "openrouter" => ImageGenerationClientSettings::OpenRouter(settings),
            _ => ImageGenerationClientSettings::OpenAi(settings),
        }
    }

    #[tokio::test]
    async fn openrouter_image_api_returns_verified_ephemeral_image_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(r#"{{"data":[{{"b64_json":"{PNG}"}}],"usage":{{"cost":0.04}}}}"#);
        let server = tokio::spawn(serve_once(listener, response));
        let result = ImageGenerationClient::new(settings("openrouter", address))
            .unwrap()
            .generate(request(ImageOutputMode::ReturnBytes), None)
            .await
            .unwrap();
        let wire = server.await.unwrap();
        assert!(wire.starts_with("POST /api/v1/images HTTP/1.1\r\n"));
        assert!(
            wire.to_ascii_lowercase()
                .contains("authorization: bearer test-key")
        );
        let body = wire.split("\r\n\r\n").nth(1).unwrap();
        let payload: Value = serde_json::from_str(body).unwrap();
        assert_eq!(payload["model"], "image-model");
        assert_eq!(payload["prompt"], "a red panda");
        assert_eq!(payload["n"], 1);
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].mime_type, "image/png");
        assert_eq!(
            result.images[0].bytes.as_deref(),
            Some(STANDARD.decode(PNG).unwrap().as_slice())
        );
        assert!(result.images[0].media_ref.is_none());
        assert_eq!(result.usage, Some(json!({"cost":0.04})));
    }

    #[tokio::test]
    async fn store_mode_writes_through_the_consumers_media_store() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(r#"{{"data":[{{"b64_json":"{PNG}","revised_prompt":"revised"}}]}}"#);
        let server = tokio::spawn(serve_once(listener, response));
        let store = Arc::new(MemoryStore::default());
        let context = MediaExecutionContext::new(CapabilityResolver::new(), store.clone());
        let result = ImageGenerationClient::new(settings("openai", address))
            .unwrap()
            .generate(
                request(ImageOutputMode::StoreInMediaContext),
                Some(&context),
            )
            .await
            .unwrap();
        server.await.unwrap();
        let image = &result.images[0];
        assert!(image.bytes.is_none());
        let reference = image.media_ref.as_ref().unwrap();
        assert_eq!(reference.sha256, image.sha256);
        assert_eq!(reference.mime_type, "image/png");
        assert_eq!(reference.name, format!("generated-{}.png", image.sha256));
        assert_eq!(
            store.read(reference).await.unwrap(),
            STANDARD.decode(PNG).unwrap()
        );
    }

    #[tokio::test]
    async fn rejects_store_mode_without_a_store_and_untrusted_image_content() {
        let client = ImageGenerationClient::new(ImageGenerationClientSettings::OpenAiCompatible(
            ProviderHttpSettings::default(),
        ))
        .unwrap();
        assert!(matches!(
            client
                .generate(request(ImageOutputMode::StoreInMediaContext), None)
                .await,
            Err(LlmError::InvalidConfiguration(_))
        ));
        let bad = json!({"data":[{"b64_json":STANDARD.encode(b"not an image")}]});
        assert!(matches!(
            parse_response(bad, &request(ImageOutputMode::ReturnBytes)),
            Err(LlmError::InvalidResponse)
        ));
        let oversized = ImageGenerationRequest {
            max_image_bytes: 1,
            ..request(ImageOutputMode::ReturnBytes)
        };
        let big = json!({"data":[{"b64_json":PNG}]});
        assert!(matches!(
            parse_response(big, &oversized),
            Err(LlmError::ResponseTooLarge)
        ));
        assert!(valid_size("1536x1024"));
        assert!(!valid_size("../image.png"));
    }
}
