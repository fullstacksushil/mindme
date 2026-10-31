//! The playground's model until the visitor adds their own: written suggestions for some nodes of the example
//! map, so trying it needs no AI, no key and no network.

/// A node and the branches it suggests. No notes: the example map already has them.
const IDEAS: &[(&str, &[&str])] = &[
    ("Atomic Habits", &["Keystone habits", "Habits at work", "Common mistakes", "Habits and sleep"]),
    ("Systems over goals", &["Process goals", "Daily routines", "Feedback loops"]),
    ("Identity", &["Self-image", "The labels you use", "Changing old beliefs"]),
    ("1st Law: Make it obvious", &["Visual reminders", "Calendar blocking", "Habit cue cards"]),
    ("Habit stacking", &["A morning stack", "A gratitude stack", "Stacking at work"]),
    ("Environment design", &["Kitchen setup", "Phone placement", "Desk setup", "Digital environment"]),
    ("Temptation bundling", &["Podcast walks", "Shows on the exercise bike", "Coffee while studying"]),
    ("The Two-Minute Rule", &["Read one page", "Put on running shoes", "Open your notes", "Write one sentence"]),
    ("Habit tracking", &["A habit journal", "Calendar streaks", "Automatic tracking", "Track only what matters"]),
    ("Never miss twice", &["Bad days still count", "A recovery plan", "All-or-nothing thinking"]),
    ("The Goldilocks Rule", &["Flow", "Variable rewards", "Raising the difficulty"]),
];

/// The written answer to a ghost request, by the last node in the prompt's path. Other requests, and other nodes,
/// get an error that says how to get real answers.
pub(crate) fn reply(prompt: &str) -> Result<String, String> {
    let node = prompt.lines().next().and_then(|l| l.strip_prefix("Mind map path: ")).and_then(|p| p.rsplit(" > ").next());
    let ideas = IDEAS.iter().find(|(n, _)| Some(*n) == node).map(|(_, ideas)| ideas);
    match ideas {
        Some(ideas) if prompt.contains("\"nodes\" is an array") => Ok(serde_json::json!({ "nodes": ideas }).to_string()),
        _ => Err("The playground has written ideas for a few nodes only. Add your own AI in Settings, Models, to ask about anything.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_ghosts_for_written_nodes_and_says_so_otherwise() {
        let ghosts = |path: &str| format!("Mind map path: {path}\nReply with only a JSON object. \"nodes\" is an array of 4 new child nodes");
        let answer = crate::ai::parse_reply(&reply(&ghosts("Atomic Habits > The Four Laws of Behavior Change > 3rd Law: Make it easy > The Two-Minute Rule")).unwrap());
        assert_eq!(answer.ghosts.iter().map(|g| g.as_str()).collect::<Vec<_>>(), ["Read one page", "Put on running shoes", "Open your notes", "Write one sentence"]);
        assert!(answer.note.is_none(), "the example's own note stays");
        assert!(reply(&ghosts("Atomic Habits > Cue")).unwrap_err().contains("Add your own AI"));
        assert!(reply("Mind map path: Atomic Habits\nExplain \"Atomic Habits\" in depth in Markdown").is_err());
    }

    #[test]
    fn every_written_node_is_in_the_example_and_every_idea_is_new() {
        let map = crate::map::Map::parse(super::super::EXAMPLE).unwrap();
        let md = map.to_markdown();
        for (node, ideas) in IDEAS {
            assert!(md.lines().any(|l| l.trim_start().strip_prefix("- ").or(l.strip_prefix("# ")) == Some(node)), "{node} is not in the example");
            for idea in *ideas {
                assert!(crate::map::Text::new(idea).is_some() && !md.contains(&format!("- {idea}\n")), "{idea}");
            }
        }
    }
}
