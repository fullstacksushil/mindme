//! The mind map and every rule of the core loop: the tree, the ghost cache, Markdown, the prompt
//! and the layout. Pure: no egui, no IO, no threads, so the tests use only plain values.

use std::collections::HashMap;

/// Ghosts asked for per request, and the shimmer placeholders shown while they load.
const GHOSTS: usize = 5;

/// Node text: one line, never empty. Text from the model, the edit box and the file all pass through `new`.
#[derive(Clone, Debug, PartialEq)]
pub struct Text(String);

impl Text {
    pub fn new(s: &str) -> Option<Text> {
        let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
        (!one_line.is_empty()).then_some(Text(one_line))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Case-insensitive equality. Models repeat themselves with different casing.
    pub fn same(&self, other: &Text) -> bool {
        self.0.to_lowercase() == other.0.to_lowercase()
    }
}

/// Monotonic and never reused, so a reply for a deleted node can never land on a newer one.
/// Only `Map` mints one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

#[derive(Clone, PartialEq)]
struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    text: Text,
    /// Markdown explaining the node. Saved as `> ` lines under its bullet, so lists inside never read as branches.
    note: Option<String>,
}

/// `rejected` sits beside the state, not inside `Ready`, so More ideas keeps it through `Loading` and `Failed`.
struct Ghosts {
    rejected: Vec<Text>,
    state: GhostState,
}

enum GhostState {
    Loading,
    Ready(Vec<Text>),
    Failed(String),
}

pub struct Map {
    /// Creation order, root first. Children are found by scanning, so sibling order is stable.
    nodes: Vec<Node>,
    next_id: u64,
    /// No entry means never requested.
    ghosts: HashMap<NodeId, Ghosts>,
    /// In-depth explanations asked for: `None` while on the way, the error when one failed.
    learning: HashMap<NodeId, Option<String>>,
    /// The discussion about each node. The last turn being `You` means a reply is on its way.
    /// shortcut: chat is lost on close, persist it if users ask
    chats: HashMap<NodeId, Vec<Turn>>,
    current: NodeId,
    /// Ask the AI whenever a node with no ghosts becomes current. Off, it asks only on `Reroll`.
    auto: bool,
    /// What the start interview learned about the person, sent with every request so answers fit them.
    brief: Option<String>,
    /// What Undo and Redo return to. Only the tree is history: ghosts, chats, folds and settings are not.
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

/// Everything the user can do to the map. Each is a no-op where it does not apply,
/// such as `Back` or `Delete` on the root, or `Reroll` while loading.
pub enum Cmd {
    /// Index into the current node's shown ghosts. Makes the kept node current only with `auto` on.
    Keep(usize),
    /// M, the "more" ghost and the retry button.
    Reroll,
    /// A child of the current node typed by hand, made current.
    Add(Text),
    /// Clipboard text added under the current node. See `Map::paste`.
    Paste(String),
    Back,
    Delete,
    Select(NodeId),
    Rename(NodeId, Text),
    /// Empty text deletes the note.
    SetNote(NodeId, String),
    SetAuto(bool),
    /// Asks the AI to explain the current node in depth, or for examples of it, added under its note.
    LearnMore(Focus),
    /// A message about the current node, answered in its discussion. A no-op while the last one waits.
    Chat(String),
    Undo,
    Redo,
    /// Moves `node` and its branches under `parent`, before its child `before` or after the last child.
    /// The root and a move into its own branches do nothing.
    Move { node: NodeId, parent: NodeId, before: Option<NodeId> },
    /// Gives up the current node's wait. A reply that still arrives is stale and dropped.
    Stop(Wait),
}

/// What a node can be waiting on from the AI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wait {
    /// Its ghosts, which bring the note.
    Ghosts,
    Learn,
    Chat,
}

/// What Learn more asks for. All end with their sources.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    Depth,
    Examples,
    /// Maps out the node: a whole subtree, merged into its branches instead of added to its note.
    MapOut,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Turn {
    You(String),
    Ai(String),
    Failed(String),
}

/// A worker's parsed reply: new child nodes, and an explanation of the asked node when the model gave one.
pub struct Answer {
    pub note: Option<String>,
    pub ghosts: Vec<Text>,
}

mod ask;
mod export;
mod history;
mod layout;
mod markdown;

use ask::Kind;
use history::Snapshot;
pub use ask::{brief, interview_prompt, topic_prompt, Ask, QUESTIONS};
pub use layout::{Layout, Note, Slot};

impl Map {
    pub fn new(root: Text) -> Map {
        let id = NodeId(0);
        let root = Node { id, parent: None, text: root, note: None };
        Map { nodes: vec![root], next_id: 1, ghosts: HashMap::new(), learning: HashMap::new(), chats: HashMap::new(), current: id, auto: true, brief: None, undo: Vec::new(), redo: Vec::new() }
    }

    /// An empty brief is none. `-->` would end the comment that holds the brief, so this splits it.
    pub fn set_brief(&mut self, brief: Option<String>) {
        self.brief = brief.and_then(tidy).map(|b| b.replace("-->", "- ->"));
    }

    #[cfg(test)]
    pub fn current(&self) -> NodeId {
        self.current
    }

    /// Applies this frame's commands and returns the requests they need. The shell calls it every frame,
    /// even with no commands. So with `auto` on, a current node with no ghost entry always receives a request.
    /// A new map, a loaded file, a kept ghost, a rename and a jump all reach the AI through this one line.
    pub fn step(&mut self, cmds: Vec<Cmd>) -> Vec<Ask> {
        let mut asks = Vec::new();
        for cmd in cmds {
            // Leaving a node saves its note, so the edited node is not always current.
            let about = if let Cmd::Rename(id, _) | Cmd::SetNote(id, _) = &cmd { *id } else { self.current };
            let travel = matches!(cmd, Cmd::Undo | Cmd::Redo);
            let before = self.nodes.clone();
            asks.extend(self.apply(cmd));
            if !travel {
                self.record(before, about);
            }
        }
        if self.auto && !self.ghosts.contains_key(&self.current) {
            asks.push(self.ask(self.current));
        }
        asks
    }

    /// Runs one command. Only `Reroll` mints an `Ask` here. `step` covers every other request.
    fn apply(&mut self, cmd: Cmd) -> Option<Ask> {
        match cmd {
            Cmd::Keep(i) => {
                let Some(GhostState::Ready(shown)) = self.ghosts.get_mut(&self.current).map(|g| &mut g.state) else {
                    return None;
                };
                if i < shown.len() {
                    let text = shown.remove(i);
                    let kept = self.add(self.current, text);
                    // Off, the parent stays current, so its other ghosts stay showing to keep more.
                    if self.auto {
                        self.current = kept;
                    }
                }
            }
            Cmd::Reroll => {
                if let Some(g) = self.ghosts.get_mut(&self.current) {
                    match &mut g.state {
                        GhostState::Loading => return None,
                        GhostState::Ready(shown) => g.rejected.append(shown),
                        GhostState::Failed(_) => {}
                    }
                }
                return Some(self.ask(self.current));
            }
            Cmd::Add(text) => self.current = self.add(self.current, text),
            Cmd::Paste(text) => self.paste(&text),
            Cmd::Back => self.current = self.node(self.current).and_then(|n| n.parent).unwrap_or(self.current),
            Cmd::Delete => {
                let id = self.current;
                let parent = self.node(id).and_then(|n| n.parent)?;
                let text = self.node(id)?.text.clone();
                let gone = self.subtree_ids(id);
                self.nodes.retain(|n| !gone.contains(&n.id));
                for id in gone {
                    self.forget(id);
                }
                self.current = parent;
                if let Some(GhostState::Ready(shown)) = self.ghosts.get_mut(&parent).map(|g| &mut g.state) {
                    shown.insert(0, text);
                }
            }
            Cmd::Select(id) => {
                if self.node(id).is_some() {
                    self.current = id;
                }
            }
            Cmd::Rename(id, text) => {
                let node = self.nodes.iter_mut().find(|n| n.id == id)?;
                if node.text != text {
                    node.text = text;
                    // The note stays: it is often the person's own writing.
                    self.forget(id);
                }
            }
            Cmd::SetNote(id, note) => self.nodes.iter_mut().find(|n| n.id == id)?.note = tidy(note),
            Cmd::SetAuto(auto) => self.auto = auto,
            Cmd::Stop(wait) => {
                let node = self.current;
                let stopped = || "stopped".to_owned();
                match wait {
                    Wait::Ghosts => {
                        let g = self.ghosts.entry(node).or_insert(Ghosts { rejected: Vec::new(), state: GhostState::Loading });
                        if matches!(g.state, GhostState::Loading) {
                            g.state = GhostState::Failed(stopped());
                        }
                    }
                    Wait::Learn => _ = self.learning.get_mut(&node).filter(|l| l.is_none())?.replace(stopped()),
                    Wait::Chat => self.chats.get_mut(&node).filter(|t| matches!(t.last(), Some(Turn::You(_))))?.push(Turn::Failed(stopped())),
                }
            }
            Cmd::Move { node, parent, before } => self.move_node(node, parent, before)?,
            Cmd::Undo => self.travel(true),
            Cmd::Redo => self.travel(false),
            Cmd::LearnMore(focus) => {
                let node = self.current;
                if self.learning.get(&node) == Some(&None) {
                    return None;
                }
                self.learning.insert(node, None);
                let noted = self.node(node).and_then(|n| n.note.clone()).filter(|_| focus != Focus::MapOut);
                return Some(Ask { node, path: self.path(node), brief: self.brief.clone(), kind: Kind::Learn { noted, focus } });
            }
            Cmd::Chat(text) => {
                let node = self.current;
                let text = tidy(text)?;
                let turns = self.chats.entry(node).or_default();
                if matches!(turns.last(), Some(Turn::You(_))) {
                    return None;
                }
                turns.push(Turn::You(text));
                let turns = turns.clone();
                let noted = self.node(node).and_then(|n| n.note.clone());
                return Some(Ask { node, path: self.path(node), brief: self.brief.clone(), kind: Kind::Chat { noted, turns } });
            }
        }
        None
    }

    /// `node` with every node under it, in file order.
    fn subtree_ids(&self, node: NodeId) -> Vec<NodeId> {
        let mut ids = vec![node];
        // Parents come before their children in `nodes`, so one pass finds them all.
        for n in &self.nodes {
            if n.parent.is_some_and(|p| ids.contains(&p)) {
                ids.push(n.id);
            }
        }
        ids
    }

    fn move_node(&mut self, node: NodeId, parent: NodeId, before: Option<NodeId>) -> Option<()> {
        self.node(node)?.parent?;
        let moving = self.subtree_ids(node);
        let before = before.filter(|b| self.node(*b).is_some_and(|n| n.parent == Some(parent)) && *b != node);
        if self.node(parent).is_none() || moving.contains(&parent) {
            return None;
        }
        // The subtree moves as one block, before `before` or at the end, so parents stay ahead of their children.
        let block: Vec<Node> = moving.iter().filter_map(|id| self.node(*id).cloned()).collect();
        self.nodes.retain(|n| !moving.contains(&n.id));
        let at = before.and_then(|b| self.nodes.iter().position(|n| n.id == b)).unwrap_or(self.nodes.len());
        self.nodes.splice(at..at, block);
        self.nodes.iter_mut().find(|n| n.id == node)?.parent = Some(parent);
        self.current = node;
        Some(())
    }

    /// The texts from the root down to `node`.
    fn path(&self, node: NodeId) -> Vec<Text> {
        let mut path = Vec::new();
        let mut at = Some(node);
        while let Some(n) = at.and_then(|id| self.node(id)) {
            path.push(n.text.clone());
            at = n.parent;
        }
        path.reverse();
        path
    }

    /// Copies `from`'s branches in `tree` under `into`. A branch with the same text as an existing child merges
    /// into it. A note only fills an empty one. So a rebuild never duplicates or overwrites.
    fn graft(&mut self, tree: &Map, from: NodeId, into: NodeId) {
        let note = tree.node(from).and_then(|n| n.note.clone());
        let node = self.nodes.iter_mut().find(|n| n.id == into).unwrap();
        if node.note.is_none() {
            node.note = note;
        }
        for branch in tree.children(from) {
            let existing = self.children(into).find(|n| n.text.same(&branch.text)).map(|n| n.id);
            let id = existing.unwrap_or_else(|| self.add(into, branch.text.clone()));
            self.graft(tree, branch.id, id);
        }
    }

    fn add(&mut self, parent: NodeId, text: Text) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(Node { id, parent: Some(parent), text, note: None });
        id
    }

    fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn children(&self, id: NodeId) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.parent == Some(id))
    }
}

/// Trims a note. An empty note becomes none.
fn tidy(note: String) -> Option<String> {
    let note = note.trim();
    (!note.is_empty()).then(|| note.to_owned())
}

#[cfg(test)]
mod tests;
