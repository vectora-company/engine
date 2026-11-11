use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
};

use futures_util::TryStreamExt;
use serde_json::{Value, json};
use vectora_engine::{
    chat::{
        ChatEventKind, ChatRequest, FallbackChatClient, FallbackChatSettings, Message, MessageRole,
    },
    harness::{
        EvidenceDispositionPolicy, RunIdentity, RunLifecycle, RunStatus, VerificationReport,
        evaluate_verification,
    },
    storage::StorageSelection,
};

const FIXTURE: &str = include_str!("../../../tests/fixtures/conformance-v1.json");

fn fixture_server(response: &Value) -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
    let address = listener.local_addr().expect("fixture server address");
    let status = response["status"].as_u64().expect("HTTP status");
    let content_type = response["content_type"]
        .as_str()
        .expect("content type")
        .to_owned();
    let body = response["body"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| response["body"].to_string());
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept fixture request");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request line");
        let mut content_length = 0;
        loop {
            line.clear();
            reader.read_line(&mut line).expect("read request header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                content_length = value.trim().parse::<usize>().expect("content length");
            }
        }
        let mut request_body = vec![0; content_length];
        reader
            .read_exact(&mut request_body)
            .expect("read request body");
        let captured: Value = serde_json::from_slice(&request_body).expect("request JSON");
        let stream = reader.into_inner();
        let status_text = if status == 200 {
            "OK"
        } else {
            "Too Many Requests"
        };
        write!(
            &stream,
            "HTTP/1.1 {status} {status_text}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write fixture response");
        captured
    });
    (format!("http://{address}/engines/v1"), server)
}

fn fallback_settings(model_ids: &[Value], endpoints: &[String]) -> FallbackChatSettings {
    let candidates = model_ids
        .iter()
        .zip(endpoints)
        .map(|(model_id, endpoint)| {
            json!({
                "model_id": model_id,
                "client": {
                    "protocol": "open_ai_compatible",
                    "settings": {"base_url": endpoint, "api_key": "fixture-token"}
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({"candidates": candidates})).expect("fallback settings")
}

#[test]
fn shared_json_fixture_conforms_to_run_verification_and_storage_contracts() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).expect("valid fixture JSON");

    let identity: RunIdentity =
        serde_json::from_value(fixture["run_lifecycle"]["identity"].clone())
            .expect("run identity fixture");
    let mut run = RunLifecycle::new(identity).expect("planned run");
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        fixture["run_lifecycle"]["planned"]
    );
    run.transition(RunStatus::Running)
        .expect("valid transition");
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        fixture["run_lifecycle"]["running"]
    );

    let report: VerificationReport =
        serde_json::from_value(fixture["verification"]["report"].clone())
            .expect("verification report fixture");
    let policy: EvidenceDispositionPolicy =
        serde_json::from_value(fixture["verification"]["policy"].clone())
            .expect("verification policy fixture");
    let evaluation = evaluate_verification(&report, &policy).expect("verification evaluation");
    assert_eq!(
        serde_json::to_value(evaluation).unwrap(),
        fixture["verification"]["evaluation"]
    );

    let selection: StorageSelection = serde_json::from_value(fixture["storage_selection"].clone())
        .expect("storage selection fixture");
    selection.validate().expect("supported storage selection");
    assert_eq!(
        serde_json::to_value(selection).unwrap(),
        fixture["storage_selection"]
    );
}

#[tokio::test]
async fn shared_fallback_fixture_conforms_to_completion_and_streaming() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid fixture JSON");
    let fallback = &fixture["fallback_chat"];
    let model_ids = fallback["candidate_model_ids"].as_array().unwrap();
    let request = ChatRequest::new(
        "ignored-model",
        vec![Message::text(MessageRole::User, "fixture prompt")],
    );

    let (first_url, first_server) = fixture_server(&fallback["rate_limited_response"]);
    let (second_url, second_server) = fixture_server(&fallback["completion"]["response"]);
    let client =
        FallbackChatClient::new(fallback_settings(model_ids, &[first_url, second_url])).unwrap();
    let completion = client.complete(request.clone()).await.unwrap();
    let first_request = first_server.join().unwrap();
    let second_request = second_server.join().unwrap();
    assert_eq!(
        completion.selected_model_id.as_deref(),
        fallback["completion"]["expected_selected_model_id"].as_str()
    );
    assert_eq!(
        completion.content,
        fallback["completion"]["expected_content"]
    );
    assert_eq!(
        json!([first_request["model"], second_request["model"]]),
        fallback["completion"]["expected_request_models"]
    );

    let (first_url, first_server) = fixture_server(&fallback["rate_limited_response"]);
    let (second_url, second_server) = fixture_server(&fallback["stream"]["response"]);
    let client =
        FallbackChatClient::new(fallback_settings(model_ids, &[first_url, second_url])).unwrap();
    let events = client
        .stream(request)
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let first_request = first_server.join().unwrap();
    let second_request = second_server.join().unwrap();
    let event_types = events
        .iter()
        .map(|event| match &event.event {
            ChatEventKind::ModelSwitched { .. } => "model_switched",
            ChatEventKind::TextDelta { .. } => "text_delta",
            ChatEventKind::Usage { .. } => "usage",
            ChatEventKind::Completed { .. } => "completed",
            ChatEventKind::ReasoningDelta { .. } | ChatEventKind::ToolCallDelta { .. } => {
                "unexpected"
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        json!(event_types),
        fallback["stream"]["expected_event_types"]
    );
    assert_eq!(
        json!([first_request["model"], second_request["model"]]),
        fallback["stream"]["expected_request_models"]
    );
    assert!(matches!(
        &events[0].event,
        ChatEventKind::ModelSwitched {
            from_model_id,
            to_model_id
        } if from_model_id == fallback["stream"]["expected_switch"]["from_model_id"]
                .as_str().unwrap()
            && to_model_id == fallback["stream"]["expected_switch"]["to_model_id"]
                .as_str().unwrap()
    ));
    assert!(matches!(
        &events[1].event,
        ChatEventKind::TextDelta { text }
            if text == fallback["stream"]["expected_text"].as_str().unwrap()
    ));
}
