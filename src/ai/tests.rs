use super::config::ghost_model;
use super::{Backend, Model, Price, Skill, Usage, chat_reply, cli_reply, Next, parse_markdown, parse_outline, parse_question, parse_reply};

#[test]
fn an_interview_reply_is_a_question_or_done() {
    let raw = "<think>hm</think>```json\n{\"question\": \" What for? \", \"recommended\": \"Work\"}\n```";
    assert_eq!(parse_question(raw), Ok(Next::Ask { question: "What for?".into(), recommended: "Work".into() }));
    assert_eq!(parse_question(r#"{"question": "Level?"}"#), Ok(Next::Ask { question: "Level?".into(), recommended: String::new() }));
    assert_eq!(parse_question(r#"Got it. {"done": true}"#), Ok(Next::Done));
    assert!(parse_question("What is your goal?").is_err());
    assert!(parse_question(r#"{"question": ""}"#).is_err());
}

#[test]
fn only_a_cli_allowed_to_search_browses() {
    let cli = |cmd: &str| Model::new(Backend::cli(cmd).unwrap()).browses();
    assert!(cli("claude -p --allowedTools WebSearch,WebFetch"));
    assert!(cli("claude -p --allowedTools WebFetch"));
    assert!(!cli("claude -p --output-format json"));
    assert!(!Model::new(Backend::Http { url: "http://localhost".into(), model: "llama".into(), key: None }).browses());
}

#[test]
fn reply_parser() {
    for raw in [
        r#"["Borrowing", "Lifetimes"]"#,
        "```json\n[\"Borrowing\", \"Lifetimes\"]\n```",
        "<think>maybe [\"Traits\"]?</think>\n[\"Borrowing\", \"Lifetimes\"]",
        "- Borrowing\n- Lifetimes",
        "Here are some ideas:\n1. Borrowing\n2) Lifetimes\nHope this helps!",
        r#"Sure: ["Borrowing", "borrowing", " ", "Lifetimes"]"#,
    ] {
        let got: Vec<_> = parse_reply(raw).ghosts.iter().map(|t| t.as_str().to_owned()).collect();
        assert_eq!(got, ["Borrowing", "Lifetimes"], "{raw:?}");
    }
}

#[test]
fn reply_parser_reads_the_note() {
    let raw = "<think>{\"x\": 1}</think>```json\n{\"note\": \"Rust is fast.\", \"nodes\": [\"Borrowing\"]}\n```";
    let answer = parse_reply(raw);
    assert_eq!(answer.note.as_deref(), Some("Rust is fast."));
    assert_eq!(answer.ghosts.len(), 1);
}

#[test]
fn outline_parser_skips_think_prose_and_fences() {
    let raw = "<think># Draft</think>Here it is:\n```markdown\n# Rust\n> A language.\n- Cargo\n```\n";
    assert_eq!(parse_outline(raw).unwrap().to_markdown(), "# Rust\n> A language.\n- Cargo\n");
    assert!(parse_outline("- no title").is_err());
}

#[test]
fn markdown_parser_unwraps_a_fence() {
    for raw in ["## A\nb", "<think>x</think>\n```markdown\n## A\nb\n```\n"] {
        assert_eq!(parse_markdown(raw).note.as_deref().map(str::trim), Some("## A\nb"), "{raw:?}");
    }
}

#[test]
fn usage_is_read_and_priced() {
    let price = Price { input: 2.0, output: 10.0, cached: Some(0.5) };
    let chat = serde_json::json!({
        "choices": [{ "message": { "content": "hi" } }],
        "model": "gpt-5",
        "usage": { "prompt_tokens": 1000, "completion_tokens": 100, "prompt_tokens_details": { "cached_tokens": 400 } }
    });
    let (text, usage) = chat_reply(&chat, &price).unwrap();
    assert_eq!(text, "hi");
    // 600 × $2 + 400 × $0.50 + 100 × $10, per million.
    assert_eq!(usage, Usage { input: 1000, output: 100, cached: 400, cost: 0.0024, model: Some("gpt-5".into()) });

    let claude = r#"{"result": "hi", "usage": {"input_tokens": 10, "cache_creation_input_tokens": 5, "cache_read_input_tokens": 90, "output_tokens": 20}, "total_cost_usd": 0.0125,
        "modelUsage": {"claude-haiku-5-5": {"costUSD": 0.001}, "claude-opus-5-5": {"costUSD": 0.0115}}}"#;
    let model = Some("claude-opus-5-5".into());
    assert_eq!(cli_reply(claude.into(), &price).unwrap().1, Usage { input: 105, output: 20, cached: 90, cost: 0.0125, model });
    assert_eq!(cli_reply("plain".into(), &price).unwrap(), ("plain".into(), Usage::default()));
    assert!(cli_reply(r#"{"result": "rate limited", "is_error": true}"#.into(), &price).is_err());
}

#[test]
fn a_skill_reads_its_file_and_is_on_by_default() {
    let path = std::env::temp_dir().join(format!("mindme-skill-{}.md", std::process::id()));
    std::fs::write(&path, "Write short sentences.").unwrap();
    let file = path.to_str().unwrap();
    let skill = Skill::parse(&serde_json::json!({ "name": "Short", "file": file })).unwrap();
    assert_eq!((skill.name.as_str(), skill.text.as_str(), skill.on), ("Short", "Write short sentences.", true));
    assert!(!Skill::parse(&serde_json::json!({ "file": file, "on": false })).unwrap().on);
    let _ = std::fs::remove_file(&path);
    let missing = Skill::parse(&serde_json::json!({ "file": file })).err().unwrap();
    assert!(missing.contains(file), "the error names the file: {missing}");
}

#[test]
fn ghosts_name_a_listed_model() {
    let models: Vec<Model> = ["Big", "Small"].iter().map(|n| Model::parse(&serde_json::json!({ "name": n, "cmd": "true" })).unwrap()).collect();
    assert_eq!(ghost_model(&models, &serde_json::json!({})), Ok(None));
    assert_eq!(ghost_model(&models, &serde_json::json!({ "ghosts": "Small" })), Ok(Some(1)));
    let typo = ghost_model(&models, &serde_json::json!({ "ghosts": "Smal" })).unwrap_err();
    assert!(typo.contains("\"Smal\""), "the error names it: {typo}");
}

#[cfg(unix)]
#[test]
fn cli_closes_stdin_and_returns_stdout() {
    let timeout = super::ASK_TIMEOUT;
    let go = std::sync::atomic::AtomicBool::new(false);
    assert_eq!(super::cli("cat", &[], "hello", timeout, &go).unwrap(), "hello");
    assert!(super::cli("false", &[], "", timeout, &go).is_err());
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert_eq!(super::cli("sleep", &["30".into()], "", timeout, &stop), Err("stopped".into()), "a stop kills the command");
    let long = "x".repeat(1 << 20);
    assert_eq!(super::cli("echo", &["OK".into()], &long, timeout, &go).unwrap(), "OK\n", "a command that never reads its input still answers");
    let started = std::time::Instant::now();
    assert_eq!(super::cli("sleep", &["30".into()], "", std::time::Duration::from_millis(100), &go), Err("timed out".into()));
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "the timeout kills the command");
}

#[test]
#[cfg(unix)]
fn codex_starts_in_an_empty_folder_of_its_own() {
    // A stand-in `codex` that prints where it runs.
    let dir = std::env::temp_dir().join(format!("mindme-fake-codex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let codex = dir.join("codex");
    std::fs::write(&codex, "#!/bin/sh\npwd\n").unwrap();
    std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let go = std::sync::atomic::AtomicBool::new(false);
    let ran_in = super::cli(codex.to_str().unwrap(), &[], "", super::ASK_TIMEOUT, &go).unwrap();
    assert_eq!(std::path::Path::new(ran_in.trim()), std::env::temp_dir().join("mindme-codex").canonicalize().unwrap());
    assert_eq!(super::cli("sh", &["-c".into(), "pwd".into()], "", super::ASK_TIMEOUT, &go).unwrap().trim(), std::env::current_dir().unwrap().to_str().unwrap(), "other commands keep the working folder");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn codex_events_give_the_reply_its_tokens_or_why_it_failed() {
    let price = Price { input: 1.25, output: 10.0, cached: Some(0.125) };
    let answered = [
        r#"{"type":"thread.started","thread_id":"t1"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"thinking"}}"#,
        r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"{\"note\": \"About it.\", \"nodes\": [\"Alpha\"]}"}}"#,
        r#"{"type":"turn.completed","usage":{"input_tokens":2000,"cached_input_tokens":1500,"output_tokens":40}}"#,
    ]
    .join("\n");
    let (text, usage) = cli_reply(answered, &price).unwrap();
    assert_eq!(text, r#"{"note": "About it.", "nodes": ["Alpha"]}"#);
    assert_eq!((usage.input, usage.cached, usage.output), (2000, 1500, 40));
    assert!((usage.cost - (500.0 * 1.25 + 1500.0 * 0.125 + 40.0 * 10.0) / 1e6).abs() < 1e-12);

    // As a logged-out codex answers: it exits 1, with log lines on stderr and the reason in its events.
    let failed = r#"printf '%s\n' '{"type":"thread.started","thread_id":"t2"}' '{"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized"}}'; echo 'ERROR codex_api: failed to connect' >&2; exit 1"#;
    let go = std::sync::atomic::AtomicBool::new(false);
    assert_eq!(super::cli("sh", &["-c".into(), failed.into()], "", super::ASK_TIMEOUT, &go), Err("unexpected status 401 Unauthorized".into()));
}

#[test]
fn an_unnamed_model_gets_a_short_label() {
    let label = |cmd: &str| Model::new(Backend::cli(cmd).unwrap()).name;
    let claude = if std::env::var_os("ANTHROPIC_API_KEY").is_some() { "Claude API" } else { "Claude Pro/Max" };
    assert_eq!(label("claude -p --output-format json --tools WebSearch,WebFetch --allowedTools WebSearch,WebFetch"), claude);
    assert_eq!(label("/usr/bin/ollama run llama3.2"), "Ollama llama3.2");
    assert_eq!(label("llm -m gpt-4o"), "llm");
    let http = |url: &str| Model::new(Backend::Http { url: url.into(), model: "gpt-5".into(), key: None }).name;
    assert_eq!(http("https://api.openai.com/v1/chat/completions"), "OpenAI gpt-5");
    assert_eq!(http("http://localhost:11434/v1/chat/completions"), "Ollama gpt-5");
    assert_eq!(http("https://example.com/v1"), "gpt-5");
    assert!(Model::new(Backend::cli("claude -p --x").unwrap()).command().ends_with("-p --x"), "the hover keeps the whole command");
}

#[test]
fn saving_models_keeps_other_keys_and_hides_the_file() {
    let path = std::env::temp_dir().join(format!("mindme-config-{}.json", std::process::id()));
    std::fs::write(&path, r#"{"footer": ["cost"], "skills": [], "models": [{"name": "Old", "cmd": "llm"}]}"#).unwrap();
    let pasted = Model::parse(&serde_json::json!({"name": "OpenAI gpt-5", "model": "gpt-5", "url": "https://api.openai.com/v1/chat/completions", "key": "sk-test"})).unwrap();
    let mut old = Model::parse(&serde_json::json!({"name": "Old", "cmd": "llm"})).unwrap();
    old.name = "Renamed".into();
    let from_env = Model::new(Backend::cli("claude -p").unwrap());
    // A crash can leave a readable `.tmp` behind.
    std::fs::write(path.with_extension("json.tmp"), "{}").unwrap();
    super::save_models(&path, &[from_env, old, pasted], Some(2)).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["footer"], serde_json::json!(["cost"]), "other keys stay");
    assert_eq!(saved["models"][0], serde_json::json!({"name": "Renamed", "cmd": "llm"}), "a rename is written, the env model is not");
    assert_eq!(saved["models"][1]["key"], "sk-test");
    assert_eq!(saved["ghosts"], "OpenAI gpt-5");
    #[cfg(unix)]
    assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path).unwrap().permissions()) & 0o777, 0o600, "it can hold keys, so only its owner reads it");
    let _ = std::fs::remove_file(&path);
}
