use crate::map::{Answer, Map, Text};

/// Turns whatever the model said into a note and ghosts. Lenient on purpose. Local models and CLI tools wrap
/// the JSON in fences, prose or `<think>` blocks. Or they send a bare array, or ignore the format and send a list.
pub fn parse_reply(raw: &str) -> Answer {
    let text = after_think(raw);
    let (note, items) = match json_object(text) {
        Some(found) => found,
        None => (None, json_array(text).unwrap_or_else(|| list_lines(text))),
    };
    let mut ghosts: Vec<Text> = Vec::new();
    for ghost in items.iter().filter_map(|s| Text::new(s)) {
        if !ghosts.iter().any(|g| g.same(&ghost)) {
            ghosts.push(ghost);
        }
    }
    Answer { note, ghosts }
}

pub fn parse_markdown(raw: &str) -> Answer {
    let text = after_think(raw).trim();
    let unfenced = text.strip_prefix("```").and_then(|t| t.strip_suffix("```")).map(|t| t.split_once('\n').map_or("", |(_, body)| body));
    Answer { note: Some(unfenced.unwrap_or(text).to_owned()), ghosts: Vec::new() }
}

pub fn parse_outline(raw: &str) -> Result<Map, String> {
    let outline: String = after_think(raw).lines().filter(|l| !l.trim_start().starts_with("```")).map(|l| format!("{l}\n")).collect();
    Map::parse(&outline).ok_or_else(|| "the reply had no '# title' line".into())
}

#[derive(Debug, PartialEq)]
pub enum Next {
    Ask { question: String, recommended: String },
    Done,
}

pub fn parse_question(raw: &str) -> Result<Next, String> {
    let text = after_think(raw);
    let object = text.find('{').zip(text.rfind('}')).and_then(|(start, end)| serde_json::from_str::<serde_json::Value>(text.get(start..=end)?).ok());
    let Some(value) = object else { return Err("the reply had no JSON object".into()) };
    match value["question"].as_str().map(str::trim).filter(|q| !q.is_empty()) {
        Some(question) => Ok(Next::Ask { question: question.into(), recommended: value["recommended"].as_str().unwrap_or_default().trim().into() }),
        None if value["done"].as_bool() == Some(true) => Ok(Next::Done),
        None => Err("the reply had neither a question nor done".into()),
    }
}

fn json_object(text: &str) -> Option<(Option<String>, Vec<String>)> {
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    let value: serde_json::Value = serde_json::from_str(text.get(start..=end)?).ok()?;
    let nodes = value["nodes"].as_array()?.iter().filter_map(|n| n.as_str().map(String::from)).collect();
    Some((value["note"].as_str().map(String::from), nodes))
}

/// Reasoning models such as qwen3 and deepseek-r1 put a `<think>` block before the answer.
fn after_think(raw: &str) -> &str {
    raw.rsplit_once("</think>").map_or(raw, |(_, after)| after)
}

fn json_array(text: &str) -> Option<Vec<String>> {
    let (start, end) = (text.find('[')?, text.rfind(']')?);
    serde_json::from_str(text.get(start..=end)?).ok()
}

fn list_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.starts_with("```")).collect();
    let marked: Vec<String> = lines.iter().filter_map(|l| strip_marker(l)).map(String::from).collect();
    if marked.is_empty() { lines.into_iter().map(String::from).collect() } else { marked }
}

fn strip_marker(line: &str) -> Option<&str> {
    if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|m| line.strip_prefix(m)) {
        return Some(rest);
    }
    let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
    (rest.len() < line.len()).then(|| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))).flatten()
}
