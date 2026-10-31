use super::{Map, NodeId};
use serde_json::{json, Value};

impl Map {
    /// The tree as nested `{"text", "note", "children"}` objects, root first, with no "note" where there is none.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.json(self.nodes[0].id)).unwrap()
    }

    fn json(&self, id: NodeId) -> Value {
        let children: Vec<Value> = self.children(id).map(|c| self.json(c.id)).collect();
        let node = self.node(id).unwrap();
        let mut out = json!({ "text": node.text.as_str(), "children": children });
        if let Some(note) = &node.note {
            out["note"] = note.as_str().into();
        }
        out
    }
}
