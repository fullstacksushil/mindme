use super::{tidy, Answer, Focus, GhostState, Ghosts, Map, NodeId, Text, Turn, GHOSTS};

/// Discuss sends only this many latest turns, so a long chat does not grow every prompt.
const CHAT_TURNS: usize = 6;

/// A request for ghosts or an in-depth note. Only `Map::step` mints one, at the moment its node goes loading,
/// and only `Map::receive` consumes it. The fields are private, so no caller can apply a reply without the staleness check.
pub struct Ask {
    pub(super) node: NodeId,
    /// Root to the asked node. Never empty: the last entry is the text the request asks about.
    pub(super) path: Vec<Text>,
    pub(super) brief: Option<String>,
    pub(super) kind: Kind,
}

pub(super) enum Kind {
    Ghosts { children: Vec<Text> },
    /// Learn more, with the note so far, so the model adds to it instead of repeating it.
    Learn { noted: Option<String>, focus: Focus },
    /// The discussion so far. The last turn is the message to answer.
    Chat { noted: Option<String>, turns: Vec<Turn> },
}

impl Ask {
    /// The node and what it waits on, so the shell can stop this request's worker.
    pub fn wait(&self) -> (NodeId, super::Wait) {
        let wait = match self.kind {
            Kind::Ghosts { .. } => super::Wait::Ghosts,
            Kind::Learn { .. } => super::Wait::Learn,
            Kind::Chat { .. } => super::Wait::Chat,
        };
        (self.node, wait)
    }

    /// True when the reply is Markdown, a note or a chat reply, not a JSON object of ghosts.
    pub fn markdown(&self) -> bool {
        matches!(self.kind, Kind::Learn { .. } | Kind::Chat { .. })
    }

    /// The graph and the expected return, nothing else.
    pub fn prompt(&self) -> String {
        let join = |texts: &[Text], sep| texts.iter().map(Text::as_str).collect::<Vec<_>>().join(sep);
        let node = self.path.last().map_or("", Text::as_str);
        let mut prompt = format!("Mind map path: {}\n", join(&self.path, " > "));
        if let Some(brief) = &self.brief {
            prompt += &format!("About this map, from an interview:\n{brief}\n");
        }
        if let Kind::Learn { noted: Some(noted), .. } | Kind::Chat { noted: Some(noted), .. } = &self.kind {
            prompt += &format!("Already noted:\n{noted}\n");
        }
        let children = match &self.kind {
            Kind::Ghosts { children } => children,
            Kind::Chat { turns, .. } => {
                let sent: Vec<&Turn> = turns.iter().filter(|t| !matches!(t, Turn::Failed(_))).collect();
                for turn in &sent[sent.len().saturating_sub(CHAT_TURNS)..] {
                    match turn {
                        Turn::You(text) => prompt += &format!("You: {text}\n"),
                        Turn::Ai(text) => prompt += &format!("AI: {text}\n"),
                        Turn::Failed(_) => {}
                    }
                }
                return prompt
                    + &format!("Reply to the last message about \"{node}\", briefly, in Markdown. When it states facts, end with a \"Sources\" list of links you are sure exist. Reply with only the Markdown.");
            }
            Kind::Learn { focus: Focus::MapOut, .. } => return prompt + &topic_prompt(node, None),
            Kind::Learn { focus, .. } => {
                prompt += &match focus {
                    Focus::Depth => format!("Explain \"{node}\" in depth in Markdown: the key ideas, how they fit together, and an example."),
                    Focus::Examples => format!("Give 2 to 3 concrete examples of \"{node}\" as a Markdown \"## Examples\" section."),
                    Focus::MapOut => unreachable!("answered above"),
                };
                return prompt + " Add to what is already noted, never repeat it. End with a \"Sources\" list of links you are sure exist. Reply with only the Markdown.";
            }
        };
        if !children.is_empty() {
            prompt += &format!("Existing children: {}\n", join(children, ", "));
        }
        prompt += &format!(
            "Reply with only a JSON object. \"note\" explains \"{node}\" in 2 to 3 sentences. \"nodes\" is an array of {GHOSTS} new child nodes, 1 to 4 words each.\n"
        );
        // Placeholders, never real words, so the model cannot copy them.
        prompt + "Format: {\"note\": \"...\", \"nodes\": [\"...\", \"...\"]}"
    }
}

/// Asks for a whole map of `topic` at once, in the same Markdown as the map file, so `Map::parse` reads the reply.
pub fn topic_prompt(topic: &str, brief: Option<&str>) -> String {
    let about = brief.map_or(String::new(), |brief| format!("About this map, from an interview:\n{brief}\n"));
    format!(
        "{about}Map out \"{topic}\" completely from its official documentation, as a Markdown outline. \
Cover every major area, 3 levels deep. End every note with a Sources line of links you are sure exist. Reply with only the outline.\nFormat:\n\
# {topic}\n> what it is, 1 to 2 sentences\n> Sources: [title](url), ...\n- Branch, 1 to 4 words\n  > what it is, 1 to 2 sentences\n  - Sub-branch\n    > ...\n"
    )
}

/// Most questions the start interview asks.
pub const QUESTIONS: usize = 6;

/// The start interview's next question, from the answers so far. One question at a time, each with a
/// recommended answer, until the model understands what map the person wants.
pub fn interview_prompt(topic: &str, map_out: bool, turns: &[(String, String)]) -> String {
    let map = if map_out { "a whole map made in one go" } else { "a brainstorm grown one idea at a time" };
    let mut prompt = format!(
        "You are interviewing someone before making a mind map of \"{topic}\", {map}. Ask one question at a time about their goal, level, angle and scope, \
each building on the answers so far, until you understand what map they want.\n"
    );
    if !turns.is_empty() {
        prompt += &format!("So far:\n{}\n", brief(turns));
    }
    prompt + "Reply with only a JSON object: {\"question\": \"...\", \"recommended\": \"your recommended answer\"}, or {\"done\": true} once you understand enough."
}

/// The interview as `Q:` and `A:` lines, which is the brief every later request carries.
pub fn brief(turns: &[(String, String)]) -> String {
    turns.iter().map(|(q, a)| format!("Q: {q}\nA: {a}")).collect::<Vec<_>>().join("\n")
}

impl Map {
    /// Applies a worker's reply, or drops it when it is stale. A Learn more reply that lands is one undo step.
    pub fn receive(&mut self, ask: Ask, result: Result<Answer, String>) {
        let Some(node) = self.node(ask.node) else { return };
        // A reply for an older text, or the second of two replies after a rename A → B → A.
        if Some(&node.text) != ask.path.last() {
            return;
        }
        if let Kind::Chat { .. } = ask.kind {
            let Some(turns) = self.chats.get_mut(&ask.node).filter(|t| matches!(t.last(), Some(Turn::You(_)))) else { return };
            turns.push(match result.map(|a| a.note.and_then(tidy)) {
                Ok(Some(reply)) => Turn::Ai(reply),
                Ok(None) => Turn::Failed("empty reply".into()),
                Err(e) => Turn::Failed(e),
            });
            return;
        }
        if let Kind::Learn { focus, .. } = ask.kind {
            if self.learning.get(&ask.node) != Some(&None) {
                return;
            }
            let before = self.nodes.clone();
            if focus == Focus::MapOut {
                match result.and_then(|a| a.note.as_deref().and_then(Map::parse).ok_or("the reply had no '# title' line".into())) {
                    Ok(tree) => {
                        self.learning.remove(&ask.node);
                        self.graft(&tree, tree.current, ask.node);
                        self.record(before, ask.node);
                    }
                    Err(e) => _ = self.learning.insert(ask.node, Some(e)),
                }
                return;
            }
            match result.map(|a| a.note.and_then(tidy)) {
                Ok(more) => {
                    self.learning.remove(&ask.node);
                    let note = &mut self.nodes.iter_mut().find(|n| n.id == ask.node).unwrap().note;
                    *note = match (note.take(), more) {
                        (Some(old), Some(more)) => Some(format!("{old}\n\n{more}")),
                        (old, more) => more.or(old),
                    };
                    self.record(before, ask.node);
                }
                Err(e) => _ = self.learning.insert(ask.node, Some(e)),
            }
            return;
        }
        let children: Vec<Text> = self.children(ask.node).map(|n| n.text.clone()).collect();
        let Some(g) = self.ghosts.get_mut(&ask.node) else { return };
        if !matches!(g.state, GhostState::Loading) {
            return;
        }
        // After More ideas, a reply full of repeats can leave fewer than GHOSTS.
        let fresh = |t: &Text| !children.iter().chain(&g.rejected).any(|o| o.same(t));
        let note = match result {
            Ok(answer) => {
                g.state = GhostState::Ready(answer.ghosts.into_iter().filter(fresh).collect());
                answer.note.and_then(tidy)
            }
            Err(e) => {
                g.state = GhostState::Failed(e);
                None
            }
        };
        // Only fills an empty note, so More ideas and revisits never overwrite a note edited in the file.
        let node = self.nodes.iter_mut().find(|n| n.id == ask.node).unwrap();
        if node.note.is_none() && note.is_some() {
            node.note = note;
            self.keep_filled_note(ask.node);
        }
    }

    /// Sets `node`'s ghosts Loading, keeping its rejected list, and mints the request for them.
    pub(super) fn ask(&mut self, node: NodeId) -> Ask {
        self.ghosts.entry(node).or_insert(Ghosts { rejected: Vec::new(), state: GhostState::Loading }).state = GhostState::Loading;
        let children = self.children(node).map(|n| n.text.clone()).collect();
        Ask { node, path: self.path(node), brief: self.brief.clone(), kind: Kind::Ghosts { children } }
    }
}
