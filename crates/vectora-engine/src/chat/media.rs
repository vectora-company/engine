//! Mídia no chat: referências sem binário, classificação pelos bytes,
//! capability por modalidade e decisão por parte.
//!
//! O consumidor guarda os bytes (um [`MediaStore`] endereçado por conteúdo) e
//! entrega à Engine só referências. Antes de enviar ao provider, a Engine
//! reidrata os bytes, confere o digest e escolhe, para cada parte, um destes
//! modos: nativo, imagem (páginas ou quadros já derivados pelo consumidor),
//! texto ou referência. A decisão é devolvida como registro auditável, sem o
//! conteúdo da mídia.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::{ContentPart, ImageUrl};

/// Tipo de mídia, decidido pelos bytes e não pelo MIME ou pela extensão.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
    /// PDF e formatos Office ou ODF.
    Document,
    /// Texto legível (UTF-8 sem bytes nulos).
    Text,
    /// Qualquer outro conteúdo.
    File,
}

impl MediaKind {
    /// Rótulo curto usado nas formas textuais.
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "imagem",
            Self::Audio => "áudio",
            Self::Video => "vídeo",
            Self::Document => "documento",
            Self::Text => "texto",
            Self::File => "arquivo",
        }
    }

    /// Embrulha a mídia na variante de [`ContentPart`] correspondente. Texto
    /// legível viaja como `File`, com o conteúdo na forma textual.
    pub fn into_part(self, media: MediaPart) -> ContentPart {
        match self {
            Self::Image => ContentPart::Image(media),
            Self::Audio => ContentPart::Audio(media),
            Self::Video => ContentPart::Video(media),
            Self::Document => ContentPart::Document(media),
            Self::Text | Self::File => ContentPart::File(media),
        }
    }
}

/// Referência a um asset no store do consumidor. Não carrega bytes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MediaRef {
    /// Identificador opaco no store do consumidor.
    pub asset_id: String,
    /// Digest SHA-256 do conteúdo, em hexadecimal minúsculo.
    pub sha256: String,
    /// Tipo detectado pelos bytes.
    pub mime_type: String,
    pub size_bytes: u64,
    /// Nome para exibição; nunca é usado como caminho.
    pub name: String,
}

/// Conteúdo de uma parte de mídia.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MediaPart {
    pub media_ref: MediaRef,
    /// Forma textual: texto extraído, transcrição ou nota.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Imagens derivadas (páginas de PDF, quadros de vídeo) geradas pelo
    /// consumidor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previews: Vec<MediaRef>,
}

impl MediaPart {
    /// Forma textual, ou uma referência com nome, tipo e tamanho.
    pub fn textual_form(&self, kind: MediaKind) -> String {
        match self.text.as_deref().map(str::trim) {
            Some(text) if !text.is_empty() => text.to_owned(),
            _ => self.reference_note(kind),
        }
    }

    /// Nota que só diz que o anexo existe.
    pub fn reference_note(&self, kind: MediaKind) -> String {
        let media = &self.media_ref;
        format!(
            "[{}: {} — {}, {} bytes]",
            kind.label(),
            media.name,
            media.mime_type,
            media.size_bytes
        )
    }
}

/// Mídia inline, montada só na cópia do request enviada ao provider. Os bytes
/// nunca fazem parte do histórico persistido.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InlineMedia {
    pub kind: MediaKind,
    pub mime_type: String,
    pub name: String,
    /// Conteúdo em base64.
    pub data_base64: String,
}

impl InlineMedia {
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.data_base64)
    }
}

// ---------------------------------------------------------------------------
// Classificação pelos bytes
// ---------------------------------------------------------------------------

/// Resultado da classificação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classified {
    pub kind: MediaKind,
    pub mime_type: String,
}

fn classified(kind: MediaKind, mime_type: &str) -> Classified {
    Classified {
        kind,
        mime_type: mime_type.to_owned(),
    }
}

/// Classifica os bytes. A assinatura do conteúdo prevalece sobre o MIME e a
/// extensão declarados; só texto legível usa o MIME declarado (quando começa
/// com `text/`) para refinar o tipo.
pub fn classify(data: &[u8], declared_mime: Option<&str>) -> Classified {
    if let Some(found) = sniff_binary(data) {
        return found;
    }
    if is_readable_text(data) {
        let mime = declared_mime
            .map(|value| value.split(';').next().unwrap_or(value).trim())
            .filter(|value| value.starts_with("text/") || *value == "application/json")
            .unwrap_or("text/plain");
        return classified(MediaKind::Text, mime);
    }
    classified(MediaKind::File, "application/octet-stream")
}

fn sniff_binary(data: &[u8]) -> Option<Classified> {
    let head = &data[..data.len().min(16)];
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(classified(MediaKind::Image, "image/png"));
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(classified(MediaKind::Image, "image/jpeg"));
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some(classified(MediaKind::Image, "image/gif"));
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" {
        match &head[8..12] {
            b"WEBP" => return Some(classified(MediaKind::Image, "image/webp")),
            b"WAVE" => return Some(classified(MediaKind::Audio, "audio/wav")),
            _ => {}
        }
    }
    if data[..data.len().min(1024)]
        .windows(5)
        .any(|window| window == b"%PDF-")
    {
        return Some(classified(MediaKind::Document, "application/pdf"));
    }
    if head.starts_with(b"fLaC") {
        return Some(classified(MediaKind::Audio, "audio/flac"));
    }
    if head.starts_with(b"OggS") {
        return Some(classified(MediaKind::Audio, "audio/ogg"));
    }
    if head.starts_with(b"ID3") || (head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0) {
        return Some(classified(MediaKind::Audio, "audio/mpeg"));
    }
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some(classified(MediaKind::Video, "video/webm"));
    }
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        return Some(match &head[8..12] {
            b"M4A " | b"M4B " => classified(MediaKind::Audio, "audio/mp4"),
            b"qt  " => classified(MediaKind::Video, "video/quicktime"),
            _ => classified(MediaKind::Video, "video/mp4"),
        });
    }
    if head.starts_with(b"PK\x03\x04") {
        return Some(sniff_zip(data));
    }
    None
}

/// Distingue Office e ODF de um zip qualquer pelos nomes das entradas.
fn sniff_zip(data: &[u8]) -> Classified {
    let window = &data[..data.len().min(4096)];
    let has = |needle: &[u8]| window.windows(needle.len()).any(|chunk| chunk == needle);
    if has(b"word/") {
        return classified(
            MediaKind::Document,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        );
    }
    if has(b"xl/") {
        return classified(
            MediaKind::Document,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        );
    }
    if has(b"ppt/") {
        return classified(
            MediaKind::Document,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        );
    }
    for (needle, mime) in [
        (
            &b"application/vnd.oasis.opendocument.text"[..],
            "application/vnd.oasis.opendocument.text",
        ),
        (
            &b"application/vnd.oasis.opendocument.spreadsheet"[..],
            "application/vnd.oasis.opendocument.spreadsheet",
        ),
        (
            &b"application/vnd.oasis.opendocument.presentation"[..],
            "application/vnd.oasis.opendocument.presentation",
        ),
    ] {
        if has(needle) {
            return classified(MediaKind::Document, mime);
        }
    }
    classified(MediaKind::File, "application/zip")
}

fn is_readable_text(data: &[u8]) -> bool {
    let sample = &data[..data.len().min(8192)];
    if sample.contains(&0) {
        return false;
    }
    match std::str::from_utf8(sample) {
        Ok(_) => true,
        // A amostra pode cortar um caractere multibyte ao meio.
        Err(error) => error.error_len().is_none() && error.valid_up_to() + 4 > sample.len(),
    }
}

/// Digest SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Capability por modalidade
// ---------------------------------------------------------------------------

/// Resposta de uma capability.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Unsupported,
    /// Sem informação; para o envio nativo vale como não suportado.
    Unknown,
}

impl CapabilityState {
    pub fn is_supported(self) -> bool {
        self == Self::Supported
    }
}

/// Modalidades que um modelo pode aceitar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modality {
    Vision,
    Pdf,
    AudioInput,
    VideoInput,
    ToolResultImages,
}

/// Capabilities resolvidas de um modelo.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModalityCapabilities {
    pub vision: CapabilityState,
    pub pdf: CapabilityState,
    pub audio_input: CapabilityState,
    pub video_input: CapabilityState,
    pub tool_result_images: CapabilityState,
}

impl ModalityCapabilities {
    /// Padrão conservador: nada é declarado.
    pub const UNKNOWN: Self = Self {
        vision: CapabilityState::Unknown,
        pdf: CapabilityState::Unknown,
        audio_input: CapabilityState::Unknown,
        video_input: CapabilityState::Unknown,
        tool_result_images: CapabilityState::Unknown,
    };

    pub fn get(&self, modality: Modality) -> CapabilityState {
        match modality {
            Modality::Vision => self.vision,
            Modality::Pdf => self.pdf,
            Modality::AudioInput => self.audio_input,
            Modality::VideoInput => self.video_input,
            Modality::ToolResultImages => self.tool_result_images,
        }
    }
}

/// Capabilities parciais de uma camada; o que falta cai na camada seguinte.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PartialCapabilities {
    pub vision: Option<CapabilityState>,
    pub pdf: Option<CapabilityState>,
    pub audio_input: Option<CapabilityState>,
    pub video_input: Option<CapabilityState>,
    pub tool_result_images: Option<CapabilityState>,
}

impl PartialCapabilities {
    fn get(&self, modality: Modality) -> Option<CapabilityState> {
        match modality {
            Modality::Vision => self.vision,
            Modality::Pdf => self.pdf,
            Modality::AudioInput => self.audio_input,
            Modality::VideoInput => self.video_input,
            Modality::ToolResultImages => self.tool_result_images,
        }
    }
}

fn known_provider(provider: &str) -> PartialCapabilities {
    use CapabilityState::{Supported, Unsupported};
    match provider {
        "anthropic" => PartialCapabilities {
            vision: Some(Supported),
            pdf: Some(Supported),
            audio_input: Some(Unsupported),
            video_input: Some(Unsupported),
            tool_result_images: Some(Supported),
        },
        "google" | "gemini" => PartialCapabilities {
            vision: Some(Supported),
            pdf: Some(Supported),
            audio_input: Some(Supported),
            video_input: Some(Supported),
            tool_result_images: Some(Supported),
        },
        "openai" => PartialCapabilities {
            vision: Some(Supported),
            pdf: Some(Supported),
            audio_input: None,
            video_input: Some(Unsupported),
            tool_result_images: Some(Supported),
        },
        "cohere" => PartialCapabilities {
            vision: None,
            pdf: Some(Unsupported),
            audio_input: Some(Unsupported),
            video_input: Some(Unsupported),
            tool_result_images: Some(Unsupported),
        },
        _ => PartialCapabilities::default(),
    }
}

/// Resolve as capabilities de um modelo em camadas: override do consumidor,
/// tabela de providers conhecidos, metadado do gateway e padrão conservador.
#[derive(Clone, Debug, Default)]
pub struct CapabilityResolver {
    overrides: BTreeMap<String, PartialCapabilities>,
    gateway: BTreeMap<String, PartialCapabilities>,
}

fn model_key(provider: &str, model: &str) -> String {
    format!("{provider}:{model}")
}

impl CapabilityResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override do consumidor para `provider:model`.
    pub fn with_override(
        mut self,
        provider: &str,
        model: &str,
        capabilities: PartialCapabilities,
    ) -> Self {
        self.overrides
            .insert(model_key(provider, model), capabilities);
        self
    }

    /// Metadado de modalidades informado por um gateway para `provider:model`.
    pub fn with_gateway_metadata(
        mut self,
        provider: &str,
        model: &str,
        capabilities: PartialCapabilities,
    ) -> Self {
        self.gateway
            .insert(model_key(provider, model), capabilities);
        self
    }

    pub fn resolve(&self, provider: &str, model: &str) -> ModalityCapabilities {
        let key = model_key(provider, model);
        let layers = [
            self.overrides.get(&key).copied().unwrap_or_default(),
            known_provider(provider),
            self.gateway.get(&key).copied().unwrap_or_default(),
        ];
        let pick = |modality: Modality| {
            layers
                .iter()
                .find_map(|layer| layer.get(modality))
                .unwrap_or(CapabilityState::Unknown)
        };
        ModalityCapabilities {
            vision: pick(Modality::Vision),
            pdf: pick(Modality::Pdf),
            audio_input: pick(Modality::AudioInput),
            video_input: pick(Modality::VideoInput),
            tool_result_images: pick(Modality::ToolResultImages),
        }
    }
}

// ---------------------------------------------------------------------------
// Store e preparação do conteúdo
// ---------------------------------------------------------------------------

/// Falhas ao ler um asset.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    #[error("asset indisponível")]
    Unavailable,
}

/// Store de mídia do consumidor. A autorização de acesso é dele: a Engine só
/// pede os bytes de uma referência que ele mesmo entregou.
#[async_trait]
pub trait MediaStore: Send + Sync {
    async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError>;
}

/// Runtime-only media dependencies. This context is never serialized into a
/// chat request or persisted in conversation history.
#[derive(Clone)]
pub struct MediaExecutionContext {
    pub resolver: CapabilityResolver,
    pub store: Arc<dyn MediaStore>,
    pub max_media_bytes: u64,
}

impl std::fmt::Debug for MediaExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaExecutionContext")
            .field("resolver", &self.resolver)
            .field("store", &"[consumer-owned]")
            .field("max_media_bytes", &self.max_media_bytes)
            .finish()
    }
}

impl MediaExecutionContext {
    pub fn new(resolver: CapabilityResolver, store: Arc<dyn MediaStore>) -> Self {
        Self {
            resolver,
            store,
            max_media_bytes: 20 * 1024 * 1024,
        }
    }

    pub fn with_max_media_bytes(mut self, max_media_bytes: u64) -> Self {
        self.max_media_bytes = max_media_bytes;
        self
    }

    /// Prepares media in the current user turn and following tool results.
    pub async fn prepare_request(
        &self,
        provider: &str,
        mut request: super::ChatRequest,
    ) -> PreparedRequest {
        let start = request
            .messages
            .iter()
            .rposition(|message| message.role == super::MessageRole::User)
            .unwrap_or(request.messages.len());
        let capabilities = self.resolver.resolve(provider, &request.model);
        let mut decisions = Vec::new();
        for (message_index, message) in request.messages.iter_mut().enumerate().skip(start) {
            if !matches!(
                message.role,
                super::MessageRole::User | super::MessageRole::Tool
            ) {
                continue;
            }
            let site = if message.role == super::MessageRole::Tool {
                ContentSite::ToolResult
            } else {
                ContentSite::UserMessage
            };
            let super::MessageContent::Parts(parts) = &mut message.content else {
                continue;
            };
            let prepared = prepare_content_with_limit(
                std::mem::take(parts),
                &capabilities,
                self.store.as_ref(),
                &request.model,
                site,
                self.max_media_bytes,
            )
            .await;
            *parts = prepared.parts;
            decisions.extend(prepared.decisions.into_iter().map(|mut decision| {
                decision.message_index = message_index;
                decision
            }));
        }
        PreparedRequest { request, decisions }
    }
}

/// A prepared request and redacted routing evidence for its media parts.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedRequest {
    pub request: super::ChatRequest,
    pub decisions: Vec<MediaDecision>,
}

/// Como uma parte de mídia segue ao modelo.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaMode {
    Native,
    Image,
    Text,
    Reference,
}

/// Registro da decisão tomada para uma parte; não contém o conteúdo.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MediaDecision {
    /// Index in `ChatRequest.messages`; zero for standalone `prepare_content`.
    pub message_index: usize,
    pub part_index: usize,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub mode: MediaMode,
    pub reason: String,
    pub model: String,
}

/// Conteúdo pronto para o provider e as decisões que o produziram.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedContent {
    pub parts: Vec<ContentPart>,
    pub decisions: Vec<MediaDecision>,
}

/// Onde a parte aparece; resultado de tool exige `tool_result_images` para
/// enviar imagem nativa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentSite {
    UserMessage,
    ToolResult,
}

/// Prepara as partes de uma mensagem para um modelo.
///
/// Partes que não são mídia passam sem mudança. Cada parte de mídia é
/// reidratada (digest e tamanho conferidos), e vira nativa, imagens, texto ou
/// referência conforme as capabilities. Asset ausente ou adulterado nunca é
/// erro: a parte vira uma nota e o motivo fica na decisão.
pub async fn prepare_content(
    parts: Vec<ContentPart>,
    capabilities: &ModalityCapabilities,
    store: &dyn MediaStore,
    model: &str,
    site: ContentSite,
) -> PreparedContent {
    prepare_content_with_limit(parts, capabilities, store, model, site, u64::MAX).await
}

/// Prepare media while refusing to read assets that exceed a byte limit.
pub async fn prepare_content_with_limit(
    parts: Vec<ContentPart>,
    capabilities: &ModalityCapabilities,
    store: &dyn MediaStore,
    model: &str,
    site: ContentSite,
    max_media_bytes: u64,
) -> PreparedContent {
    let mut prepared = Vec::with_capacity(parts.len());
    let mut decisions = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        let Some((kind, media)) = media_of(&part) else {
            prepared.push(part);
            continue;
        };
        let outcome = decide(kind, media, capabilities, store, site, max_media_bytes).await;
        decisions.push(MediaDecision {
            message_index: 0,
            part_index: index,
            name: media.media_ref.name.clone(),
            mime_type: media.media_ref.mime_type.clone(),
            size_bytes: media.media_ref.size_bytes,
            sha256: media.media_ref.sha256.clone(),
            mode: outcome.mode,
            reason: outcome.reason,
            model: model.to_owned(),
        });
        prepared.extend(outcome.parts);
    }
    PreparedContent {
        parts: prepared,
        decisions,
    }
}

fn media_of(part: &ContentPart) -> Option<(MediaKind, &MediaPart)> {
    match part {
        ContentPart::Image(media) => Some((MediaKind::Image, media)),
        ContentPart::Audio(media) => Some((MediaKind::Audio, media)),
        ContentPart::Video(media) => Some((MediaKind::Video, media)),
        ContentPart::Document(media) => Some((MediaKind::Document, media)),
        ContentPart::File(media) => Some((MediaKind::File, media)),
        _ => None,
    }
}

struct Outcome {
    mode: MediaMode,
    reason: String,
    parts: Vec<ContentPart>,
}

fn text_part(text: String) -> ContentPart {
    ContentPart::Text { text }
}

async fn read_verified(
    media_ref: &MediaRef,
    store: &dyn MediaStore,
    max_media_bytes: u64,
) -> Result<Vec<u8>, String> {
    if media_ref.size_bytes > max_media_bytes {
        return Err("mídia excede o limite configurado".to_owned());
    }
    let data = store
        .read(media_ref)
        .await
        .map_err(|_| "asset indisponível".to_owned())?;
    if data.len() as u64 > max_media_bytes {
        return Err("mídia excede o limite configurado".to_owned());
    }
    if data.len() as u64 != media_ref.size_bytes {
        return Err("tamanho divergente do registrado".to_owned());
    }
    if sha256_hex(&data) != media_ref.sha256 {
        return Err("digest divergente do registrado".to_owned());
    }
    Ok(data)
}

fn fallback(kind: MediaKind, media: &MediaPart, reason: &str) -> Outcome {
    let has_text = media
        .text
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty());
    Outcome {
        mode: if has_text {
            MediaMode::Text
        } else {
            MediaMode::Reference
        },
        reason: reason.to_owned(),
        parts: vec![text_part(media.textual_form(kind))],
    }
}

fn unavailable(media: &MediaPart, why: &str) -> Outcome {
    Outcome {
        mode: MediaMode::Reference,
        reason: why.to_owned(),
        parts: vec![text_part(format!(
            "[anexo não enviado: {}]",
            media.media_ref.name
        ))],
    }
}

fn inline(kind: MediaKind, media_ref: &MediaRef, data: &[u8]) -> InlineMedia {
    InlineMedia {
        kind,
        mime_type: media_ref.mime_type.clone(),
        name: media_ref.name.clone(),
        data_base64: STANDARD.encode(data),
    }
}

async fn decide(
    kind: MediaKind,
    media: &MediaPart,
    capabilities: &ModalityCapabilities,
    store: &dyn MediaStore,
    site: ContentSite,
    max_media_bytes: u64,
) -> Outcome {
    let vision = capabilities.vision.is_supported();
    let native = match kind {
        MediaKind::Image => {
            vision
                && (site == ContentSite::UserMessage
                    || capabilities.tool_result_images.is_supported())
        }
        MediaKind::Audio => capabilities.audio_input.is_supported(),
        MediaKind::Video => capabilities.video_input.is_supported(),
        MediaKind::Document => {
            capabilities.pdf.is_supported() && media.media_ref.mime_type == "application/pdf"
        }
        MediaKind::Text | MediaKind::File => false,
    };

    if native {
        return match read_verified(&media.media_ref, store, max_media_bytes).await {
            Ok(data) => {
                let item = inline(kind, &media.media_ref, &data);
                let part = if kind == MediaKind::Image {
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: item.data_url(),
                            detail: None,
                        },
                    }
                } else {
                    ContentPart::Inline(item)
                };
                Outcome {
                    mode: MediaMode::Native,
                    reason: "o modelo declara a modalidade".to_owned(),
                    parts: vec![part],
                }
            }
            Err(why) if why == "mídia excede o limite configurado" => fallback(kind, media, &why),
            Err(why) => unavailable(media, &why),
        };
    }

    let derives_images = matches!(kind, MediaKind::Document | MediaKind::Video)
        && vision
        && !media.previews.is_empty()
        && (site == ContentSite::UserMessage || capabilities.tool_result_images.is_supported());
    if derives_images {
        let mut parts = vec![text_part(media.textual_form(kind))];
        for preview in &media.previews {
            match read_verified(preview, store, max_media_bytes).await {
                Ok(data) => parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: inline(MediaKind::Image, preview, &data).data_url(),
                        detail: None,
                    },
                }),
                Err(why) if why == "mídia excede o limite configurado" => {
                    return fallback(kind, media, &why);
                }
                Err(why) => return unavailable(media, &why),
            }
        }
        return Outcome {
            mode: MediaMode::Image,
            reason: "o modelo lê imagem, não o arquivo".to_owned(),
            parts,
        };
    }

    let reason = match kind {
        MediaKind::Text | MediaKind::File => "arquivo sem forma nativa",
        _ => "o modelo não declara a modalidade",
    };
    fallback(kind, media, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000000000000000";
    const PDF: &[u8] = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n";

    #[derive(Default)]
    struct MemoryStore(HashMap<String, Vec<u8>>);

    impl MemoryStore {
        fn put(&mut self, id: &str, data: &[u8]) -> MediaRef {
            self.0.insert(id.to_owned(), data.to_vec());
            reference(id, data, "x")
        }
    }

    #[async_trait]
    impl MediaStore for MemoryStore {
        async fn read(&self, media_ref: &MediaRef) -> Result<Vec<u8>, MediaError> {
            self.0
                .get(&media_ref.asset_id)
                .cloned()
                .ok_or(MediaError::Unavailable)
        }
    }

    fn reference(id: &str, data: &[u8], name: &str) -> MediaRef {
        MediaRef {
            asset_id: id.to_owned(),
            sha256: sha256_hex(data),
            mime_type: classify(data, None).mime_type,
            size_bytes: data.len() as u64,
            name: name.to_owned(),
        }
    }

    fn part(media_ref: MediaRef, text: Option<&str>, previews: Vec<MediaRef>) -> MediaPart {
        MediaPart {
            media_ref,
            text: text.map(str::to_owned),
            previews,
        }
    }

    fn caps(vision: bool, pdf: bool) -> ModalityCapabilities {
        let state = |on| {
            if on {
                CapabilityState::Supported
            } else {
                CapabilityState::Unsupported
            }
        };
        ModalityCapabilities {
            vision: state(vision),
            pdf: state(pdf),
            audio_input: CapabilityState::Unsupported,
            video_input: CapabilityState::Unsupported,
            tool_result_images: state(vision),
        }
    }

    #[test]
    fn classifica_pelos_bytes_e_nao_pelo_mime_declarado() {
        assert_eq!(classify(PNG, Some("text/plain")).kind, MediaKind::Image);
        assert_eq!(classify(PNG, None).mime_type, "image/png");
        assert_eq!(
            classify(PDF, Some("image/png")),
            classified(MediaKind::Document, "application/pdf")
        );
        assert_eq!(
            classify(b"\xFF\xD8\xFF\xE0abcd", None).mime_type,
            "image/jpeg"
        );
        assert_eq!(classify(b"GIF89a....", None).kind, MediaKind::Image);
        assert_eq!(
            classify(b"RIFF\0\0\0\0WEBPVP8 ", None).mime_type,
            "image/webp"
        );
        assert_eq!(
            classify(b"RIFF\0\0\0\0WAVEfmt ", None).kind,
            MediaKind::Audio
        );
        assert_eq!(classify(b"fLaC....", None).mime_type, "audio/flac");
        assert_eq!(classify(b"OggS....", None).mime_type, "audio/ogg");
        assert_eq!(
            classify(b"\0\0\0\x18ftypisom....", None).mime_type,
            "video/mp4"
        );
        assert_eq!(
            classify(b"\0\0\0\x18ftypM4A ....", None).kind,
            MediaKind::Audio
        );
        assert_eq!(
            classify(b"\x1A\x45\xDF\xA3....", None).kind,
            MediaKind::Video
        );
    }

    #[test]
    fn classifica_office_odf_zip_texto_e_binario() {
        let docx = b"PK\x03\x04\0\0\0\0word/document.xml";
        assert_eq!(classify(docx, None).kind, MediaKind::Document);
        assert!(classify(docx, None).mime_type.contains("wordprocessingml"));
        let xlsx = b"PK\x03\x04\0\0\0\0xl/workbook.xml";
        assert!(classify(xlsx, None).mime_type.contains("spreadsheetml"));
        let odt = b"PK\x03\x04\0\0\0\0mimetypeapplication/vnd.oasis.opendocument.text";
        assert_eq!(
            classify(odt, None).mime_type,
            "application/vnd.oasis.opendocument.text"
        );
        assert_eq!(
            classify(b"PK\x03\x04\0\0\0\0outro.txt", None),
            classified(MediaKind::File, "application/zip")
        );
        assert_eq!(
            classify(
                "olá, mundo".as_bytes(),
                Some("text/markdown; charset=utf-8")
            ),
            classified(MediaKind::Text, "text/markdown")
        );
        assert_eq!(classify(b"abc", Some("image/png")).mime_type, "text/plain");
        assert_eq!(
            classify(&[0, 159, 146, 150, 1, 2, 3], None),
            classified(MediaKind::File, "application/octet-stream")
        );
    }

    #[test]
    fn digest_sha256_conhecido() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn resolver_respeita_a_ordem_das_camadas() {
        let resolver = CapabilityResolver::new()
            .with_gateway_metadata(
                "openrouter",
                "m",
                PartialCapabilities {
                    vision: Some(CapabilityState::Supported),
                    pdf: Some(CapabilityState::Supported),
                    ..PartialCapabilities::default()
                },
            )
            .with_override(
                "openrouter",
                "m",
                PartialCapabilities {
                    pdf: Some(CapabilityState::Unsupported),
                    ..PartialCapabilities::default()
                },
            );
        let resolved = resolver.resolve("openrouter", "m");
        assert_eq!(resolved.vision, CapabilityState::Supported);
        assert_eq!(resolved.pdf, CapabilityState::Unsupported, "override vence");
        assert_eq!(resolved.audio_input, CapabilityState::Unknown);
    }

    #[test]
    fn tabela_conhecida_vence_o_metadado_do_gateway() {
        let resolver = CapabilityResolver::new().with_gateway_metadata(
            "anthropic",
            "claude",
            PartialCapabilities {
                pdf: Some(CapabilityState::Unsupported),
                ..PartialCapabilities::default()
            },
        );
        let resolved = resolver.resolve("anthropic", "claude");
        assert_eq!(resolved.pdf, CapabilityState::Supported);
        assert_eq!(resolved.audio_input, CapabilityState::Unsupported);
        assert_eq!(
            CapabilityResolver::new().resolve("desconhecido", "x"),
            ModalityCapabilities::UNKNOWN
        );
    }

    #[test]
    fn openai_chat_completions_declara_pdf_nativo() {
        let capabilities = CapabilityResolver::new().resolve("openai", "model-x");
        assert_eq!(capabilities.pdf, CapabilityState::Supported);
    }

    #[tokio::test]
    async fn imagem_nativa_vira_data_url_e_registra_a_decisao() {
        let mut store = MemoryStore::default();
        let media = part(store.put("a", PNG), None, vec![]);
        let result = prepare_content(
            vec![text_part("olhe".into()), ContentPart::Image(media)],
            &caps(true, false),
            &store,
            "modelo",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(result.parts.len(), 2);
        match &result.parts[1] {
            ContentPart::ImageUrl { image_url } => {
                assert!(image_url.url.starts_with("data:image/png;base64,"));
            }
            other => panic!("esperava imagem, veio {other:?}"),
        }
        assert_eq!(result.decisions.len(), 1);
        assert_eq!(result.decisions[0].mode, MediaMode::Native);
        assert_eq!(result.decisions[0].part_index, 1);
        assert_eq!(result.decisions[0].model, "modelo");
    }

    #[tokio::test]
    async fn imagem_sem_visao_cai_na_forma_textual_ou_na_referencia() {
        let mut store = MemoryStore::default();
        let com_texto = part(store.put("a", PNG), Some("[imagem: gato]"), vec![]);
        let sem_texto = part(reference("b", PNG, "foto.png"), None, vec![]);
        let result = prepare_content(
            vec![ContentPart::Image(com_texto), ContentPart::Image(sem_texto)],
            &caps(false, false),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(result.decisions[0].mode, MediaMode::Text);
        assert_eq!(result.decisions[1].mode, MediaMode::Reference);
        assert!(
            matches!(&result.parts[1], ContentPart::Text { text } if text.contains("foto.png"))
        );
    }

    #[tokio::test]
    async fn imagem_de_resultado_de_tool_exige_a_capability_propria() {
        let mut store = MemoryStore::default();
        let media = part(store.put("a", PNG), None, vec![]);
        let mut capabilities = caps(true, false);
        capabilities.tool_result_images = CapabilityState::Unsupported;
        let result = prepare_content(
            vec![ContentPart::Image(media)],
            &capabilities,
            &store,
            "m",
            ContentSite::ToolResult,
        )
        .await;
        assert_eq!(result.decisions[0].mode, MediaMode::Reference);
    }

    #[tokio::test]
    async fn pdf_nativo_so_quando_o_modelo_le_pdf() {
        let mut store = MemoryStore::default();
        let media = part(store.put("p", PDF), Some("texto extraído"), vec![]);
        let nativo = prepare_content(
            vec![ContentPart::Document(media.clone())],
            &caps(true, true),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(nativo.decisions[0].mode, MediaMode::Native);
        assert!(
            matches!(&nativo.parts[0], ContentPart::Inline(item) if item.mime_type == "application/pdf")
        );

        let texto = prepare_content(
            vec![ContentPart::Document(media)],
            &caps(false, false),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(texto.decisions[0].mode, MediaMode::Text);
        assert!(matches!(&texto.parts[0], ContentPart::Text { text } if text == "texto extraído"));
    }

    #[tokio::test]
    async fn pdf_vira_imagens_quando_o_modelo_so_le_imagem() {
        let mut store = MemoryStore::default();
        let page = store.put("pg1", PNG);
        let media = part(store.put("p", PDF), Some("[documento: a.pdf]"), vec![page]);
        let result = prepare_content(
            vec![ContentPart::Document(media)],
            &caps(true, false),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(result.decisions[0].mode, MediaMode::Image);
        assert_eq!(result.parts.len(), 2);
        assert!(matches!(result.parts[1], ContentPart::ImageUrl { .. }));
    }

    #[tokio::test]
    async fn digest_adulterado_ou_asset_ausente_vira_nota_com_motivo() {
        let mut store = MemoryStore::default();
        let mut media = part(store.put("a", PNG), None, vec![]);
        media.media_ref.sha256 = sha256_hex(b"outro conteudo");
        let adulterado = prepare_content(
            vec![ContentPart::Image(media)],
            &caps(true, false),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(adulterado.decisions[0].mode, MediaMode::Reference);
        assert!(adulterado.decisions[0].reason.contains("digest"));
        assert!(
            matches!(&adulterado.parts[0], ContentPart::Text { text } if text.contains("não enviado"))
        );

        let ausente = prepare_content(
            vec![ContentPart::Image(part(
                reference("nao-existe", PNG, "x.png"),
                None,
                vec![],
            ))],
            &caps(true, false),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert!(ausente.decisions[0].reason.contains("indisponível"));
    }

    #[tokio::test]
    async fn arquivo_generico_nunca_vai_nativo() {
        let store = MemoryStore::default();
        let media = part(
            reference("z", b"PK\x03\x04\0\0\0\0outro", "dados.zip"),
            None,
            vec![],
        );
        let result = prepare_content(
            vec![ContentPart::File(media)],
            &caps(true, true),
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(result.decisions[0].mode, MediaMode::Reference);
        assert!(
            matches!(&result.parts[0], ContentPart::Text { text } if text.contains("dados.zip"))
        );
    }

    #[tokio::test]
    async fn size_limit_is_checked_before_reading_media() {
        let mut store = MemoryStore::default();
        let media = part(store.put("large", PDF), Some("extracted text"), vec![]);
        let result = prepare_content_with_limit(
            vec![ContentPart::Document(media)],
            &caps(true, true),
            &store,
            "m",
            ContentSite::UserMessage,
            4,
        )
        .await;

        assert_eq!(result.decisions[0].mode, MediaMode::Text);
        assert!(result.decisions[0].reason.contains("limite"));
        assert!(matches!(&result.parts[0], ContentPart::Text { text } if text == "extracted text"));
    }

    #[test]
    fn historico_serializado_nao_carrega_bytes() {
        let mut store = MemoryStore::default();
        let media = part(store.put("a", PNG), Some("nota"), vec![]);
        let json = serde_json::to_string(&ContentPart::Image(media.clone())).unwrap();
        assert!(!json.contains("data:"));
        assert!(!json.contains("base64"));
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ContentPart::Image(media));
        assert!(json.contains("\"type\":\"image\""));
    }

    #[tokio::test]
    async fn execution_context_prepares_only_the_current_user_turn() {
        use std::sync::Arc;

        use super::super::{ChatRequest, Message, MessageContent, MessageRole};

        let mut store = MemoryStore::default();
        let mut old = store.put("old", PDF);
        old.name = "old.pdf".into();
        let mut current = store.put("current", PDF);
        current.name = "current.pdf".into();
        let request = ChatRequest::new(
            "gemini-2.5-pro",
            vec![
                Message {
                    role: MessageRole::User,
                    content: MessageContent::Parts(vec![ContentPart::Document(part(
                        old,
                        Some("old text"),
                        vec![],
                    ))]),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                    name: None,
                    is_error: false,
                },
                Message::text(MessageRole::Assistant, "older response"),
                Message {
                    role: MessageRole::User,
                    content: MessageContent::Parts(vec![ContentPart::Document(part(
                        current,
                        None,
                        vec![],
                    ))]),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                    name: None,
                    is_error: false,
                },
            ],
        );
        let context = MediaExecutionContext::new(CapabilityResolver::new(), Arc::new(store));

        let prepared = context.prepare_request("google", request).await;
        assert!(matches!(
            &prepared.request.messages[0].content,
            MessageContent::Parts(parts) if matches!(&parts[0], ContentPart::Document(_))
        ));
        assert!(matches!(
            &prepared.request.messages[2].content,
            MessageContent::Parts(parts) if matches!(&parts[0], ContentPart::Inline(_))
        ));
        assert_eq!(prepared.decisions.len(), 1);
        assert_eq!(prepared.decisions[0].message_index, 2);
        assert_eq!(prepared.decisions[0].name, "current.pdf");
    }

    #[tokio::test]
    async fn partes_que_nao_sao_midia_passam_sem_mudanca() {
        let store = MemoryStore::default();
        let result = prepare_content(
            vec![text_part("oi".into())],
            &ModalityCapabilities::UNKNOWN,
            &store,
            "m",
            ContentSite::UserMessage,
        )
        .await;
        assert_eq!(result.parts, vec![text_part("oi".into())]);
        assert!(result.decisions.is_empty());
    }
}
