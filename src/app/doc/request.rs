use super::{Doc, Shared};
use crate::ai;
use crate::map::{self, Answer, Ask, Map, NodeId, Text, Wait};
use eframe::egui;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::Duration;

/// The variant fixes the prompt, the reply's parser and the message it becomes.
pub(super) enum Request {
    /// Ghosts, Learn more or Discuss. The `Ask` travels with the reply so `Map::receive` can run the staleness check.
    Map(Ask),
    /// A whole map of a topic. The brief goes onto the returned map.
    Outline { topic: Text, brief: Option<String> },
    Question { topic: Text, map_out: bool, turns: Vec<(String, String)> },
}

/// What the reply is. Timeout, model and the writing guides follow from it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Wants {
    Ghosts,
    Prose,
    Outline,
    Question,
}

impl Wants {
    /// Prose and outlines can mean reading documentation online.
    fn timeout(self) -> Duration {
        match self {
            Wants::Prose | Wants::Outline => ai::LONG_TIMEOUT,
            Wants::Ghosts | Wants::Question => ai::ASK_TIMEOUT,
        }
    }

    fn guided(self) -> bool {
        matches!(self, Wants::Prose | Wants::Outline)
    }
}

impl Request {
    fn wants(&self) -> Wants {
        match self {
            Request::Map(ask) if ask.markdown() => Wants::Prose,
            Request::Map(_) => Wants::Ghosts,
            Request::Outline { .. } => Wants::Outline,
            Request::Question { .. } => Wants::Question,
        }
    }

    /// The wait a Stop names to kill this request. `None` is the start screen's interview or map out.
    fn waits(&self) -> Option<(NodeId, Wait)> {
        match self {
            Request::Map(ask) => Some(ask.wait()),
            _ => None,
        }
    }

    fn prompt(&self) -> String {
        match self {
            Request::Map(ask) => ask.prompt(),
            Request::Outline { topic, brief } => map::topic_prompt(topic.as_str(), brief.as_deref()),
            Request::Question { topic, map_out, turns } => map::interview_prompt(topic.as_str(), *map_out, turns),
        }
    }

    fn reply(self, raw: Result<String, String>) -> Msg {
        match self {
            Request::Map(ask) => {
                let parse = if ask.markdown() { ai::parse_markdown } else { ai::parse_reply };
                Msg::Map(ask, raw.map(|raw| parse(&raw)))
            }
            Request::Outline { brief, .. } => Msg::Outline(raw.and_then(|raw| ai::parse_outline(&raw)).map(|mut map| {
                map.set_brief(brief);
                map
            })),
            Request::Question { .. } => Msg::Question(raw.and_then(|raw| ai::parse_question(&raw))),
        }
    }
}

/// A reply, with its request's stop flag, so a stopped request's late reply is dropped.
pub(super) type Reply = (Arc<AtomicBool>, Msg);

pub(super) enum Msg {
    Map(Ask, Result<Answer, String>),
    Outline(Result<Map, String>),
    Question(Result<ai::Next, String>),
}

fn with_guides(skills: &[ai::Skill], prompt: String) -> String {
    let guides: Vec<&str> = skills.iter().filter(|s| s.on).map(|s| s.text.trim()).collect();
    if guides.is_empty() {
        return prompt;
    }
    format!("Follow these writing guides in the prose you write. Keep the reply shape asked for below.\n\n{}\n\n{prompt}", guides.join("\n\n"))
}

impl Doc {
    /// The only place a worker starts. Ghosts go to the config's ghost model when it names one.
    pub(super) fn send(&mut self, request: Request, shared: &Shared, ctx: &egui::Context) {
        let wants = request.wants();
        let prompt = if wants.guided() { with_guides(shared.skills, request.prompt()) } else { request.prompt() };
        let model = match wants {
            Wants::Ghosts => shared.ghosts.unwrap_or(self.model),
            _ => self.model,
        };
        let stop = Arc::new(AtomicBool::new(false));
        // A finished worker has dropped its copy.
        self.stops.retain(|(_, s)| Arc::strong_count(s) > 1);
        self.stops.push((request.waits(), stop.clone()));
        let stopped = stop.clone();
        shared.models[model].spawn(prompt, wants.timeout(), ctx, &self.tx, stop, move |raw| (stopped, request.reply(raw)));
        self.in_flight += 1;
    }
}

/// A running worker's stop flag, and the wait it serves.
pub(super) type Running = (Option<(NodeId, Wait)>, Arc<AtomicBool>);

/// Kills the workers running for `wait`. Takes the list, not the tab, so it works while the map is borrowed.
pub(super) fn stop(stops: &[Running], wait: Option<(NodeId, Wait)>) {
    for (_, stop) in stops.iter().filter(|(w, _)| *w == wait) {
        stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Cmd, Focus};

    #[test]
    fn each_kind_of_request_has_its_timeout_and_guides() {
        let t = |s| Text::new(s).unwrap();
        let mut map = Map::new(t("Rust"));
        let ghosts = map.step(vec![]).pop().unwrap();
        let learn = map.step(vec![Cmd::LearnMore(Focus::Depth)]).pop().unwrap();
        let requests = [
            Request::Map(ghosts),
            Request::Map(learn),
            Request::Outline { topic: t("Rust"), brief: None },
            Request::Question { topic: t("Rust"), map_out: false, turns: Vec::new() },
        ];
        let table: Vec<_> = requests.iter().map(|r| (r.wants(), r.wants().timeout(), r.wants().guided())).collect();
        assert_eq!(
            table,
            [
                (Wants::Ghosts, ai::ASK_TIMEOUT, false),
                (Wants::Prose, ai::LONG_TIMEOUT, true),
                (Wants::Outline, ai::LONG_TIMEOUT, true),
                (Wants::Question, ai::ASK_TIMEOUT, false),
            ]
        );
    }

    #[test]
    fn writing_guides_go_ahead_of_prose_prompts_only_while_on() {
        let pirate = |on| ai::Skill { name: "Pirate".into(), text: "Write like a pirate.\n".into(), on };
        assert_eq!(with_guides(&[], "Q".into()), "Q");
        assert_eq!(with_guides(&[pirate(false)], "Q".into()), "Q");
        let guided = with_guides(&[pirate(true), pirate(false)], "Q".into());
        assert!(guided.starts_with("Follow these writing guides") && guided.ends_with("\n\nWrite like a pirate.\n\nQ"), "{guided:?}");
    }
}
