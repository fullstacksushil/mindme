use super::{GhostState, Map, Node, NodeId};

/// shortcut: each step keeps a whole copy of the tree, store diffs if maps grow to thousands of nodes
const STEPS: usize = 100;

/// The tree before a step, and the node to make current when Undo or Redo returns to it.
pub(super) struct Snapshot {
    nodes: Vec<Node>,
    current: NodeId,
}

impl Map {
    /// Records a step when the tree changed from `before`. `about` is the node it changed, current again on undo.
    pub(super) fn record(&mut self, before: Vec<Node>, about: NodeId) {
        if before == self.nodes {
            return;
        }
        if self.undo.len() == STEPS {
            self.undo.remove(0);
        }
        self.undo.push(Snapshot { nodes: before, current: about });
        self.redo.clear();
    }

    /// Undo when `back`, redo otherwise. The reverse step returns to where you were before this move.
    pub(super) fn travel(&mut self, back: bool) {
        let Some(to) = (if back { self.undo.pop() } else { self.redo.pop() }) else { return };
        let from = Snapshot { nodes: std::mem::replace(&mut self.nodes, to.nodes), current: self.current };
        self.current = to.current;
        for old in &from.nodes {
            if self.node(old.id).map(|n| &n.text) != Some(&old.text) {
                self.forget(old.id);
            }
        }
        // Undoing a Delete restores a node whose text is showing as a ghost.
        for (id, g) in &mut self.ghosts {
            if let GhostState::Ready(shown) = &mut g.state {
                shown.retain(|t| !self.nodes.iter().any(|n| n.parent == Some(*id) && n.text.same(t)));
            }
        }
        if back { self.redo.push(from) } else { self.undo.push(from) }
    }

    /// Drops what answered a node's old text, once it is gone or renamed: its ghosts, Learn more and discussion.
    pub(super) fn forget(&mut self, id: NodeId) {
        self.ghosts.remove(&id);
        self.learning.remove(&id);
        self.chats.remove(&id);
    }

    /// A ghost reply filled `id`'s empty note, which is not a step, so undoing another step must not lose it.
    pub(super) fn keep_filled_note(&mut self, id: NodeId) {
        let Some(filled) = self.node(id).cloned() else { return };
        for n in self.undo.iter_mut().chain(&mut self.redo).flat_map(|s| &mut s.nodes) {
            if n.id == id && n.text == filled.text && n.note.is_none() {
                n.note = filled.note.clone();
            }
        }
    }
}
