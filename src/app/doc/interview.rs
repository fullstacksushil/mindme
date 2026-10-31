use super::request::Request;
use super::Goal;
use crate::ai;
use crate::map::{self, Text};

/// The grill-me interview a tab runs before its map. Owns every rule about it. The screen only draws it.
pub(crate) struct Interview {
    topic: Text,
    goal: Goal,
    /// Questions and answers so far, which become the map's brief.
    turns: Vec<(String, String)>,
    /// The question waiting for an answer, and the model's recommended answer. None while the next one is on its way.
    asking: Option<(String, String)>,
    error: Option<String>,
    pub(crate) answer: String,
    /// The model said it knows enough.
    done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    Answer,
    /// Make the map with what the interview has so far.
    Start,
    Retry,
    /// Give up the question on its way.
    Stop,
}

impl Interview {
    pub(super) fn new(topic: Text, goal: Goal) -> Interview {
        Interview { topic, goal, turns: Vec::new(), asking: None, error: None, answer: String::new(), done: false }
    }

    /// Clears the question and error on screen while it is on its way.
    pub(super) fn request(&mut self) -> Request {
        (self.asking, self.error) = (None, None);
        Request::Question { topic: self.topic.clone(), map_out: self.goal == Goal::MapOut, turns: self.turns.clone() }
    }

    pub(super) fn receive(&mut self, next: Result<ai::Next, String>) {
        match next {
            Ok(ai::Next::Ask { question, recommended }) => self.asking = Some((question, recommended)),
            Ok(ai::Next::Done) => self.done = true,
            Err(e) => self.error = Some(e),
        }
    }

    /// Records the typed answer, trimmed, under the question on screen. False when there is no question or the box is blank.
    pub(crate) fn answer(&mut self) -> bool {
        let Some((question, _)) = &self.asking else { return false };
        if self.answer.trim().is_empty() {
            return false;
        }
        self.turns.push((question.clone(), std::mem::take(&mut self.answer).trim().to_owned()));
        true
    }

    pub(crate) fn take_recommended(&mut self) {
        if let Some((_, recommended)) = &self.asking {
            self.answer = recommended.clone();
        }
    }

    pub(super) fn finished(&self, step: Option<Step>) -> bool {
        self.done || step == Some(Step::Start) || (step == Some(Step::Answer) && self.turns.len() >= map::QUESTIONS)
    }

    pub(super) fn into_brief(self) -> (Text, Goal, String) {
        (self.topic, self.goal, map::brief(&self.turns))
    }

    pub(crate) fn topic(&self) -> &Text {
        &self.topic
    }

    pub(crate) fn goal(&self) -> Goal {
        self.goal
    }

    pub(crate) fn turns(&self) -> &[(String, String)] {
        &self.turns
    }

    pub(crate) fn asking(&self) -> Option<&(String, String)> {
        self.asking.as_ref()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked() -> Interview {
        let mut interview = Interview::new(Text::new("Rust").unwrap(), Goal::MapOut);
        interview.receive(Ok(ai::Next::Ask { question: "What for?".into(), recommended: "Games".into() }));
        interview
    }

    #[test]
    fn an_answer_is_recorded_trimmed_and_a_blank_one_is_not() {
        let mut interview = Interview::new(Text::new("Rust").unwrap(), Goal::MapOut);
        interview.answer = "early".into();
        assert!(!interview.answer(), "no question on screen yet");
        let mut interview = asked();
        interview.answer = "  \n ".into();
        assert!(!interview.answer() && interview.turns().is_empty(), "a blank answer does nothing");
        interview.answer = "  An engine \n".into();
        assert!(interview.answer());
        assert_eq!(interview.turns(), [("What for?".to_owned(), "An engine".to_owned())]);
        assert!(interview.answer.is_empty(), "the box is cleared");
        interview.take_recommended();
        assert_eq!(interview.answer, "Games");
    }

    #[test]
    fn the_interview_finishes_on_done_start_or_the_last_answer() {
        let mut interview = asked();
        assert!(!interview.finished(None) && !interview.finished(Some(Step::Retry)));
        assert!(interview.finished(Some(Step::Start)));
        for _ in 1..map::QUESTIONS {
            interview.answer = "yes".into();
            interview.answer();
        }
        assert!(!interview.finished(Some(Step::Answer)), "one question left");
        interview.answer = "yes".into();
        interview.answer();
        assert!(interview.finished(Some(Step::Answer)), "the last question was answered");
        let mut done = asked();
        done.receive(Ok(ai::Next::Done));
        assert!(done.finished(None), "the model knows enough");
    }
}
