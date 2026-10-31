use super::{Cmd, GhostState, Map, NodeId, Text, Turn, GHOSTS};

/// One row in the fog beside the current node.
pub enum Slot<'a> {
    /// Index into the shown ghosts, and its text.
    Ghost(usize, &'a Text),
    Shimmer,
    /// Shown alone when the shown list is empty.
    More,
    /// Shown alone after a failure, with the error text.
    Retry(&'a str),
}

impl Slot<'_> {
    /// What clicking this slot or pressing Enter on it does.
    pub fn pick(&self) -> Option<Cmd> {
        match self {
            Slot::Ghost(i, _) => Some(Cmd::Keep(*i)),
            Slot::More | Slot::Retry(_) => Some(Cmd::Reroll),
            Slot::Shimmer => None,
        }
    }
}

pub struct Placed<'a> {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub text: &'a Text,
}

/// What the shell draws. Where things sit on screen is the shell's business, not the map's.
pub struct Layout<'a> {
    /// Root first, parents before their children.
    pub nodes: Vec<Placed<'a>>,
    /// The current node's slots.
    pub ghosts: Vec<Slot<'a>>,
    pub current: NodeId,
    /// The current node's explanation, if it has one or one is on its way.
    pub note: Option<Note<'a>>,
    /// Learn more on the current node: `Some(None)` while on the way, `Some(Some(error))` when it failed.
    pub learning: Option<Option<&'a str>>,
    /// The current node's discussion, oldest first.
    pub chat: &'a [Turn],
    pub auto: bool,
    pub brief: Option<&'a str>,
}

impl<'a> Layout<'a> {
    pub fn node(&self, id: NodeId) -> Option<&Placed<'a>> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn children(&self, id: NodeId) -> impl Iterator<Item = &Placed<'a>> {
        self.nodes.iter().filter(move |n| n.parent == Some(id))
    }

    /// The parent of `id`, its parent, and so on up to the root.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let parent = move |id| self.node(id).and_then(|n| n.parent);
        std::iter::successors(parent(id), move |&id| parent(id))
    }
}

pub enum Note<'a> {
    Loading,
    Text(&'a str),
}

impl Map {
    pub fn layout(&self) -> Layout<'_> {
        let state = self.ghosts.get(&self.current).map(|g| &g.state);
        // With `auto` on, `step` requests a missing entry this frame.
        let loading = matches!(state, Some(GhostState::Loading)) || (self.auto && state.is_none());
        let slots = match state {
            _ if loading => (0..GHOSTS).map(|_| Slot::Shimmer).collect(),
            None | Some(GhostState::Loading) => Vec::new(),
            Some(GhostState::Ready(shown)) if shown.is_empty() => vec![Slot::More],
            Some(GhostState::Ready(shown)) => shown.iter().enumerate().map(|(i, t)| Slot::Ghost(i, t)).collect(),
            Some(GhostState::Failed(e)) => vec![Slot::Retry(e)],
        };
        let note = match self.node(self.current).and_then(|n| n.note.as_deref()) {
            Some(text) => Some(Note::Text(text)),
            None => loading.then_some(Note::Loading),
        };
        let nodes = self.nodes.iter().map(|n| Placed { id: n.id, parent: n.parent, text: &n.text }).collect();
        let learning = self.learning.get(&self.current).map(Option::as_deref);
        let chat = self.chats.get(&self.current).map_or(&[][..], Vec::as_slice);
        Layout { nodes, ghosts: slots, current: self.current, note, learning, chat, auto: self.auto, brief: self.brief.as_deref() }
    }
}
