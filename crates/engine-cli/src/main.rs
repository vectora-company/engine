use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use indicatif::{ProgressBar, ProgressStyle};
use rustyline::{DefaultEditor, error::ReadlineError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use termimad::print_text;
use vectora_engine::chat::{
    ChatCompletion, ChatRequest, DecisionQuestion, DecisionQuestionKind, DecisionRequest,
    LayaClient, LayaSettings, LlamaServerClient, LlamaServerSettings, Message, MessageRole,
    OpenAiCompatibleClient, OpenAiCompatibleConfig, ToolArguments, ToolCall, ToolDefinition,
};

const TOOL_NAME: &str = "calculator";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ChatBackend {
    LlamaServer,
    OpenAiCompatible,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DecisionBackend {
    LayaServer,
    LlamaServer,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ChatConfig {
    backend: ChatBackend,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DecisionConfig {
    backend: DecisionBackend,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Config {
    chat: ChatConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decision: Option<DecisionConfig>,
    #[serde(default = "default_tool_turns")]
    max_tool_turns: usize,
}

const fn default_tool_turns() -> usize {
    6
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chat: ChatConfig {
                backend: ChatBackend::LlamaServer,
                base_url: "http://127.0.0.1:8080".into(),
                model: "local-model".into(),
                api_key_env: None,
            },
            decision: Some(DecisionConfig {
                backend: DecisionBackend::LayaServer,
                base_url: "http://127.0.0.1:8081".into(),
                model: "jev-1".into(),
                api_key_env: None,
            }),
            max_tool_turns: default_tool_turns(),
        }
    }
}

fn config_path() -> PathBuf {
    if let Some(path) = env::var_os("ENGINE_CLI_CONFIG") {
        return PathBuf::from(path);
    }
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("vectora").join("engine-cli").join("config.json")
}

fn read_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn write_config(path: &Path, config: &Config) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn api_key(env_name: &Option<String>) -> Result<Option<String>, Box<dyn Error>> {
    env_name
        .as_ref()
        .map(|name| {
            env::var(name).map_err(|_| format!("environment variable {name} is not set").into())
        })
        .transpose()
}

fn make_chat_client(config: &ChatConfig) -> Result<ChatClient, Box<dyn Error>> {
    let key = api_key(&config.api_key_env)?;
    match config.backend {
        ChatBackend::LlamaServer => {
            let mut settings = json!({ "base_url": config.base_url });
            if let Some(key) = key {
                settings["api_key"] = json!(key);
            }
            let client =
                LlamaServerClient::new(serde_json::from_value::<LlamaServerSettings>(settings)?)?;
            Ok(ChatClient::LlamaServer(Box::new(client)))
        }
        ChatBackend::OpenAiCompatible => {
            let mut endpoint = OpenAiCompatibleConfig::new(&config.base_url);
            if let Some(key) = key {
                endpoint = endpoint.api_key(key);
            }
            Ok(ChatClient::OpenAi(Box::new(OpenAiCompatibleClient::new(
                endpoint,
            )?)))
        }
    }
}

enum ChatClient {
    LlamaServer(Box<LlamaServerClient>),
    OpenAi(Box<OpenAiCompatibleClient>),
}

impl ChatClient {
    async fn complete(&self, request: ChatRequest) -> Result<ChatCompletion, Box<dyn Error>> {
        match self {
            Self::LlamaServer(client) => Ok(client.complete(request).await?),
            Self::OpenAi(client) => Ok(client.complete(request).await?),
        }
    }
}

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: TOOL_NAME.into(),
        description: Some("Evaluate a basic arithmetic expression safely.".into()),
        parameters: json!({
            "type": "object",
            "properties": { "expression": { "type": "string" } },
            "required": ["expression"],
            "additionalProperties": false
        }),
    }
}

fn execute_tool(call: &ToolCall) -> Result<String, String> {
    if call.name != TOOL_NAME {
        return Err(format!(
            "tool '{}' is not available in engine-cli",
            call.name
        ));
    }
    let ToolArguments::Object { value } = &call.arguments else {
        return Err("tool arguments are not a valid JSON object".into());
    };
    let expression = value
        .get("expression")
        .and_then(Value::as_str)
        .ok_or_else(|| "expression must be a string".to_owned())?;
    evaluate(expression).map(|number| number.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Expr<'a> {
    input: &'a [u8],
    cursor: usize,
}

fn evaluate(input: &str) -> Result<f64, String> {
    let mut parser = Expr {
        input: input.as_bytes(),
        cursor: 0,
    };
    let value = parser.sum()?;
    parser.whitespace();
    if parser.cursor != parser.input.len() || !value.is_finite() {
        return Err("invalid or non-finite arithmetic expression".into());
    }
    Ok(value)
}

impl Expr<'_> {
    fn whitespace(&mut self) {
        while self
            .input
            .get(self.cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.cursor += 1;
        }
    }
    fn sum(&mut self) -> Result<f64, String> {
        let mut value = self.product()?;
        loop {
            self.whitespace();
            match self.input.get(self.cursor) {
                Some(b'+') => {
                    self.cursor += 1;
                    value += self.product()?;
                }
                Some(b'-') => {
                    self.cursor += 1;
                    value -= self.product()?;
                }
                _ => return Ok(value),
            }
        }
    }
    fn product(&mut self) -> Result<f64, String> {
        let mut value = self.atom()?;
        loop {
            self.whitespace();
            match self.input.get(self.cursor) {
                Some(b'*') => {
                    self.cursor += 1;
                    value *= self.atom()?;
                }
                Some(b'/') => {
                    self.cursor += 1;
                    value /= self.atom()?;
                }
                _ => return Ok(value),
            }
        }
    }
    fn atom(&mut self) -> Result<f64, String> {
        self.whitespace();
        if self.input.get(self.cursor) == Some(&b'-') {
            self.cursor += 1;
            return Ok(-self.atom()?);
        }
        if self.input.get(self.cursor) == Some(&b'(') {
            self.cursor += 1;
            let value = self.sum()?;
            self.whitespace();
            if self.input.get(self.cursor) != Some(&b')') {
                return Err("missing closing parenthesis".into());
            }
            self.cursor += 1;
            return Ok(value);
        }
        let start = self.cursor;
        while self
            .input
            .get(self.cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
        {
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err("expected a number".into());
        }
        std::str::from_utf8(&self.input[start..self.cursor])
            .map_err(|_| "invalid number")?
            .parse()
            .map_err(|_| "invalid number".into())
    }
}

fn append_assistant(messages: &mut Vec<Message>, completion: &ChatCompletion) {
    let mut message = Message::text(MessageRole::Assistant, completion.content.clone());
    message.tool_calls = completion.tool_calls.clone();
    messages.push(message);
}

fn append_tool_results(messages: &mut Vec<Message>, calls: &[ToolCall]) {
    for call in calls {
        let result = execute_tool(call);
        let (content, is_error) = match result {
            Ok(value) => (value, false),
            Err(error) => (error, true),
        };
        messages.push(Message {
            role: MessageRole::Tool,
            content: vectora_engine::chat::MessageContent::Text(content),
            tool_calls: Vec::new(),
            tool_call_id: Some(call.id.clone()),
            name: Some(call.name.clone()),
            is_error,
        });
    }
}

fn spinner() -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}").expect("static spinner template"),
    );
    bar.set_message("Engine thinking");
    bar.enable_steady_tick(Duration::from_millis(90));
    bar
}

async fn react_turn(
    client: &ChatClient,
    config: &ChatConfig,
    history: &mut Vec<Message>,
    max_tool_turns: usize,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..=max_tool_turns {
        let mut request = ChatRequest::new(&config.model, history.clone());
        request.tools.push(tool_definition());
        let progress = spinner();
        let completion = client.complete(request).await;
        progress.finish_and_clear();
        let completion = completion?;
        append_assistant(history, &completion);
        if completion.tool_calls.is_empty() {
            if !completion.content.is_empty() {
                print_text(&completion.content);
            }
            return Ok(());
        }
        append_tool_results(history, &completion.tool_calls);
    }
    println!("Tool-call limit reached; send another message to continue.");
    Ok(())
}

async fn decide(config: &DecisionConfig, prompt: &str) -> Result<Value, Box<dyn Error>> {
    let key = api_key(&config.api_key_env)?;
    let request = DecisionRequest {
        model: config.model.clone(),
        state: json!({ "prompt": prompt }),
        questions: BTreeMap::from([(
            "answer".into(),
            DecisionQuestion {
                kind: DecisionQuestionKind::Noul,
                instructions: Some(json!(
                    "Determine whether the prompt can be answered affirmatively."
                )),
                criteria: None,
            },
        )]),
    };
    let progress = spinner();
    let result = match config.backend {
        DecisionBackend::LayaServer => {
            let mut settings = json!({ "base_url": config.base_url });
            if let Some(key) = key {
                settings["api_key"] = json!(key);
            }
            let client = LayaClient::new(serde_json::from_value::<LayaSettings>(settings)?)?;
            client
                .decide(request)
                .await
                .map(|response| serde_json::to_value(response).expect("serializable response"))
        }
        DecisionBackend::LlamaServer => {
            let mut settings = json!({ "base_url": config.base_url });
            if let Some(key) = key {
                settings["api_key"] = json!(key);
            }
            let client =
                LlamaServerClient::new(serde_json::from_value::<LlamaServerSettings>(settings)?)?;
            client
                .decide(request)
                .await
                .map(|response| serde_json::to_value(response).expect("serializable response"))
        }
    };
    progress.finish_and_clear();
    Ok(result?)
}

fn usage() {
    eprintln!(
        "engine-cli <init|show|validate|set|chat|decide> [args]\n\n  init [path]                 write a starter config\n  show [path]                 print config with secret references only\n  validate [path]             validate config without connecting\n  set <key> <value> [path]    update a dotted JSON config field\n  chat [path]                 start a ReAct chat\n  decide <prompt> [path]      test configured Laya/llama.cpp Decision Model\n\nAPI keys are read from environment variables named by api_key_env."
    );
}

fn run_config_command(command: &str, args: &[String]) -> Result<(), Box<dyn Error>> {
    match command {
        "init" => {
            let path = args.first().map(PathBuf::from).unwrap_or_else(config_path);
            if path.exists() {
                return Err(format!("config already exists: {}", path.display()).into());
            }
            let config = Config::default();
            write_config(&path, &config)?;
            println!("Wrote {}", path.display());
        }
        "show" | "validate" => {
            let path = args.first().map(PathBuf::from).unwrap_or_else(config_path);
            let config = read_config(&path)?;
            validate_config(&config)?;
            if command == "show" {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                println!("Configuration is valid: {}", path.display());
            }
        }
        "set" => {
            if args.len() < 2 {
                return Err("set requires a dotted key and value".into());
            }
            let path = args.get(2).map(PathBuf::from).unwrap_or_else(config_path);
            let mut value: Value = serde_json::from_slice(&fs::read(&path)?)?;
            set_dotted(&mut value, &args[0], &args[1])?;
            let config: Config = serde_json::from_value(value)?;
            validate_config(&config)?;
            write_config(&path, &config)?;
        }
        _ => usage(),
    }
    Ok(())
}

fn validate_config(config: &Config) -> Result<(), Box<dyn Error>> {
    if config.chat.base_url.trim().is_empty()
        || config.chat.model.trim().is_empty()
        || config.max_tool_turns == 0
        || config.max_tool_turns > 32
    {
        return Err(
            "chat endpoint/model must be non-empty and max_tool_turns must be 1..=32".into(),
        );
    }
    if let Some(decision) = &config.decision
        && (decision.base_url.trim().is_empty() || decision.model.trim().is_empty())
    {
        return Err("decision endpoint/model must be non-empty".into());
    }
    Ok(())
}

fn set_dotted(root: &mut Value, dotted: &str, text: &str) -> Result<(), Box<dyn Error>> {
    let keys: Vec<_> = dotted.split('.').collect();
    if keys.is_empty() || keys.iter().any(|key| key.is_empty()) {
        return Err("invalid dotted config key".into());
    }
    let mut current = root;
    for key in &keys[..keys.len() - 1] {
        current = current.get_mut(*key).ok_or("unknown config key")?;
    }
    let target = current
        .get_mut(keys[keys.len() - 1])
        .ok_or("unknown config key")?;
    let replacement = match target {
        Value::String(_) => Value::String(text.to_owned()),
        Value::Number(_) => Value::Number(text.parse::<u64>()?.into()),
        Value::Bool(_) => Value::Bool(text.parse()?),
        Value::Null => Value::String(text.to_owned()),
        _ => return Err("only existing string, number, or boolean fields can be set".into()),
    };
    *target = replacement;
    Ok(())
}

fn main_result() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    if matches!(command, "init" | "show" | "validate" | "set") {
        return run_config_command(command, &args[1..]);
    }
    if command == "help" || command == "--help" || command == "-h" {
        usage();
        return Ok(());
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let result = async {
        let args: Vec<String> = env::args().skip(1).collect();
        let command = args.first().map(String::as_str).unwrap_or("help");
        if matches!(command, "init" | "show" | "validate" | "set" | "help" | "--help" | "-h") {
            return main_result();
        }
        let path = args.last().filter(|arg| Path::new(arg).exists()).map(PathBuf::from).unwrap_or_else(config_path);
        let config = read_config(&path)?;
        validate_config(&config)?;
        match command {
            "chat" => {
                let client = make_chat_client(&config.chat)?;
                let mut editor = DefaultEditor::new()?;
                let mut history = vec![Message::text(MessageRole::System, "You are a concise assistant. Use the calculator tool for arithmetic; never invent tool results.")];
                loop {
                    match editor.readline("you> ") {
                        Ok(line) if line.trim() == "/exit" || line.trim() == "/quit" => break,
                        Ok(line) if line.trim().is_empty() => continue,
                        Ok(line) => { let _ = editor.add_history_entry(line.as_str()); history.push(Message::text(MessageRole::User, line)); react_turn(&client, &config.chat, &mut history, config.max_tool_turns).await?; }
                        Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            "decide" => {
                let prompt = args.get(1).ok_or("decide requires a prompt")?;
                let decision = config.decision.as_ref().ok_or("no decision endpoint configured")?;
                println!("{}", serde_json::to_string_pretty(&decide(decision, prompt).await?)?);
            }
            _ => usage(),
        }
        Ok::<(), Box<dyn Error>>(())
    }.await;
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "engine-cli: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_keep_credentials_out_of_the_file() {
        let value = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(value["chat"]["backend"], "llama_server");
        assert_eq!(value["decision"]["backend"], "laya_server");
        assert_eq!(value["chat"]["api_key_env"], Value::Null);
        assert!(value.get("api_key").is_none());
    }

    #[test]
    fn supports_both_laya_decision_transports() {
        let mut config = Config::default();
        config.decision.as_mut().unwrap().backend = DecisionBackend::LlamaServer;
        validate_config(&config).unwrap();
        config.decision.as_mut().unwrap().backend = DecisionBackend::LayaServer;
        validate_config(&config).unwrap();
    }

    #[test]
    fn config_set_updates_only_existing_fields() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        set_dotted(&mut value, "chat.model", "laya-GGUF").unwrap();
        assert_eq!(value["chat"]["model"], "laya-GGUF");
        assert!(set_dotted(&mut value, "chat.secret", "oops").is_err());
    }

    #[test]
    fn calculator_obeys_precedence_and_rejects_code_and_non_finite_values() {
        assert_eq!(evaluate("2 + 3 * (4 - 1)").unwrap(), 11.0);
        assert_eq!(evaluate("-8 / 2 + 1").unwrap(), -3.0);
        assert!(evaluate("1 / 0").is_err());
        assert!(evaluate("std::process::exit(1)").is_err());
    }

    #[test]
    fn react_history_pairs_tool_result_with_the_models_call_id() {
        let call = ToolCall {
            id: "call-42".into(),
            name: TOOL_NAME.into(),
            arguments: ToolArguments::from_json(r#"{"expression":"2 + 3 * 4"}"#),
        };
        let completion = ChatCompletion {
            contract_version: vectora_engine::chat::CHAT_CONTRACT_VERSION,
            id: Some("completion-1".into()),
            selected_model_id: None,
            content: String::new(),
            reasoning: None,
            tool_calls: vec![call.clone()],
            finish_reason: None,
            usage: None,
            media_decisions: Vec::new(),
        };
        let mut messages = Vec::new();
        append_assistant(&mut messages, &completion);
        append_tool_results(&mut messages, &completion.tool_calls);
        assert_eq!(messages[0].tool_calls[0].id, "call-42");
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-42"));
        assert_eq!(
            messages[1].content,
            vectora_engine::chat::MessageContent::Text("14".into())
        );
    }
}
