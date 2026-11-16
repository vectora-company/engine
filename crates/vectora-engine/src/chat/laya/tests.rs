use std::collections::BTreeMap;

use super::*;
use crate::chat::{DecisionQuestion, DecisionQuestionKind};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn settings(base_url: String) -> LayaSettings {
    LayaSettings {
        base_url,
        api_key: Some("secret-key".into()),
        headers: BTreeMap::new(),
        connect_timeout_ms: 2_000,
        read_timeout_ms: 2_000,
        total_timeout_ms: 4_000,
        max_response_bytes: 64 * 1024,
        max_request_bytes: 16 * 1024,
    }
}

fn choice_request(option_count: usize) -> DecisionRequest {
    let criteria = (0..option_count)
        .map(|index| {
            (
                format!("option-{index}"),
                serde_json::json!(format!("Option {index}")),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    DecisionRequest {
        model: "multilingual".into(),
        state: serde_json::json!({"ticket": "Cobrança duplicada"}),
        questions: BTreeMap::from([(
            "route".into(),
            DecisionQuestion {
                kind: DecisionQuestionKind::Choice,
                instructions: Some(serde_json::json!("Choose a queue")),
                criteria: Some(serde_json::Value::Object(criteria)),
            },
        )]),
    }
}

async fn receive_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(
            read, 0,
            "client closed before sending complete HTTP request"
        );
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + content_length {
                return bytes;
            }
        }
    }
}

async fn server_once(
    response_status: &str,
    response_body: &str,
) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let status = response_status.to_owned();
    let body = response_body.to_owned();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = receive_request(&mut stream).await;
        let text = String::from_utf8_lossy(&request);
        assert!(text.starts_with("POST /v1/systemone "));
        assert!(
            text.to_ascii_lowercase()
                .contains("authorization: bearer secret-key")
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        request
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn health_uses_native_endpoint_and_parses_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 2048];
        let read = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /health "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-key")
        );
        let body = r#"{"status":"ok"}"#;
        stream.write_all(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()
        ).as_bytes()).await.unwrap();
    });
    let client = LayaClient::new(settings(format!("http://{address}/v1"))).unwrap();
    assert_eq!(client.health().await.unwrap().status, "ok");
    server.await.unwrap();
}

#[test]
fn settings_normalize_v1_and_redact_credentials() {
    let client = LayaClient::new(settings("http://127.0.0.1:8080/v1/".into())).unwrap();
    assert_eq!(client.root_url.as_str(), "http://127.0.0.1:8080/");
    assert!(!format!("{client:?}").contains("secret-key"));
    assert!(LayaClient::new(settings("http://user:pass@localhost".into())).is_err());
}

#[tokio::test]
async fn posts_native_laya_payload_without_openai_model_discovery_and_accepts_full_response() {
    let request = choice_request(64);
    let probabilities = (0..64)
        .map(|index| (format!("option-{index}"), serde_json::json!(0.0156)))
        .collect::<serde_json::Map<_, _>>();
    let response = serde_json::json!({
        "model": "laya-systemone",
        "answers": {"route": {
            "type": "choice", "choice": "option-0", "probabilities": probabilities,
            "confidence": 0.02, "answer_confidence": 0.0156, "action": {"act_probability": 1.0}
        }},
        "usage": {"input_tokens": 83, "output_tokens": 0, "state_tokens": 12, "truncated": false},
        "routing": {"model": "multilingual", "repo": "convaiinnovations/laya/multilingual"}
    });
    let (url, server) = server_once("200 OK", &response.to_string()).await;
    let client = LayaClient::new(settings(url)).unwrap();
    let result = client.decide(request).await.unwrap();
    assert_eq!(result.answers["route"]["choice"], "option-0");
    assert_eq!(result.usage.unwrap().input_tokens, Some(83));
    let sent = String::from_utf8_lossy(&server.await.unwrap()).into_owned();
    assert!(sent.contains("\"model\":\"multilingual\""));
    assert!(sent.contains("\"questions\""));
}

#[tokio::test]
async fn accepts_laya_score_probabilities_and_legend_as_keyed_objects() {
    let response = r#"{"model":"laya","answers":{"urgency":{"type":"score","score":1.4,"legend":{"0":"calm","1":"firm","2":"angry"},"probabilities":{"0":0.1,"1":0.5,"2":0.4},"confidence":0.2}},"usage":{"input_tokens":8,"output_tokens":0},"routing":{}}"#;
    let (url, server) = server_once("200 OK", response).await;
    let client = LayaClient::new(settings(url)).unwrap();
    let request = DecisionRequest {
        model: "typed-decisions".into(),
        state: serde_json::json!("The customer sounds firm."),
        questions: BTreeMap::from([(
            "urgency".into(),
            DecisionQuestion {
                kind: DecisionQuestionKind::Score,
                instructions: Some(serde_json::json!("How urgent?")),
                criteria: Some(serde_json::json!(["calm", "firm", "angry"])),
            },
        )]),
    };
    assert_eq!(
        client.decide(request).await.unwrap().answers["urgency"]["score"],
        1.4
    );
    server.await.unwrap();
}

#[tokio::test]
async fn accepts_noul_answer_and_preserves_server_fields() {
    let response = r#"{"answers":{"risk":{"noul":0.75,"confidence":0.9,"action":{"act_probability":0.8}}},"usage":{"input_tokens":4,"output_tokens":0}}"#;
    let (url, server) = server_once("200 OK", response).await;
    let client = LayaClient::new(settings(url)).unwrap();
    let request = DecisionRequest {
        model: "typed-decisions".into(),
        state: serde_json::json!("A payment may be fraudulent."),
        questions: BTreeMap::from([(
            "risk".into(),
            DecisionQuestion {
                kind: DecisionQuestionKind::Noul,
                instructions: Some(serde_json::json!("Is this high risk?")),
                criteria: None,
            },
        )]),
    };
    let result = client.decide(request).await.unwrap();
    assert_eq!(result.answers["risk"]["noul"], 0.75);
    assert_eq!(result.answers["risk"]["action"]["act_probability"], 0.8);
    server.await.unwrap();
}

#[tokio::test]
async fn rejects_wrong_answer_ids_and_maps_auth_errors() {
    let (url, server) = server_once("200 OK", r#"{"answers":{"other":{"noul":0.7}}}"#).await;
    let client = LayaClient::new(settings(url)).unwrap();
    assert!(matches!(
        client.decide(choice_request(2)).await,
        Err(ChatError::InvalidResponse)
    ));
    server.await.unwrap();

    let (url, server) = server_once("401 Unauthorized", r#"{"detail":"invalid token"}"#).await;
    let client = LayaClient::new(settings(url)).unwrap();
    assert!(matches!(
        client.decide(choice_request(2)).await,
        Err(ChatError::Authentication)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn rejects_bad_probabilities_and_oversized_responses() {
    let bad = r#"{"answers":{"route":{"choice":"option-0","probabilities":{"option-0":0.2,"option-1":0.2},"confidence":0.5}}}"#;
    let (url, server) = server_once("200 OK", bad).await;
    let client = LayaClient::new(settings(url)).unwrap();
    assert!(matches!(
        client.decide(choice_request(2)).await,
        Err(ChatError::InvalidResponse)
    ));
    server.await.unwrap();

    let (url, server) = server_once(
        "200 OK",
        &format!("{{\"padding\":\"{}\"}}", "x".repeat(200)),
    )
    .await;
    let mut config = settings(url);
    config.max_response_bytes = 64;
    let client = LayaClient::new(config).unwrap();
    assert!(matches!(
        client.decide(choice_request(2)).await,
        Err(ChatError::ResponseTooLarge)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn refuses_inline_media_in_persistible_state_before_network() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let client = LayaClient::new(settings(url)).unwrap();
    let mut request = choice_request(2);
    request.state = serde_json::json!({"image_url": "data:image/png;base64,aGVsbG8="});
    assert!(matches!(
        client.decide(request).await,
        Err(ChatError::InvalidRequest(_))
    ));
}

#[tokio::test]
async fn applies_laya_wire_limits_before_network() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let client = LayaClient::new(settings(url)).unwrap();

    let mut request = choice_request(2);
    request.questions.get_mut("route").unwrap().instructions = None;
    assert!(matches!(
        client.decide(request).await,
        Err(ChatError::InvalidRequest(
            "Laya questions require instructions"
        ))
    ));

    let mut request = choice_request(2);
    request.state = serde_json::json!("x".repeat(50_001));
    assert!(matches!(
        client.decide(request).await,
        Err(ChatError::InvalidRequest(
            "Laya state exceeds 50,000 characters"
        ))
    ));

    let mut request = choice_request(2);
    request.questions.get_mut("route").unwrap().kind = DecisionQuestionKind::Score;
    request.questions.get_mut("route").unwrap().criteria = Some(serde_json::json!(
        (0..33).map(|i| i.to_string()).collect::<Vec<_>>()
    ));
    assert!(matches!(
        client.decide(request).await,
        Err(ChatError::InvalidRequest(
            "Laya score questions accept at most 32 levels"
        ))
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut config = settings(url);
    config.max_request_bytes = 32;
    let client = LayaClient::new(config).unwrap();
    assert!(matches!(
        client.decide(choice_request(2)).await,
        Err(ChatError::InvalidRequest(
            "decision request exceeds the configured body limit"
        ))
    ));
}

#[tokio::test]
async fn optional_real_laya_server_smoke() {
    let Ok(base_url) = std::env::var("VECTORA_TEST_LAYA_URL") else {
        return;
    };
    let mut config = settings(base_url);
    config.api_key = std::env::var("VECTORA_TEST_LAYA_API_KEY").ok();
    let client = LayaClient::new(config).unwrap();
    assert_eq!(client.health().await.unwrap().status, "ok");
    let mut request = choice_request(2);
    request.model = std::env::var("VECTORA_TEST_LAYA_MODEL").unwrap_or_else(|_| "jev-1".into());
    let response = client.decide(request.clone()).await.unwrap();
    assert!(laya_answer_matches(
        request.questions.get("route").unwrap(),
        &response.answers["route"]
    ));
}
