//! Force-directed layout, the way Obsidian's graph view moves. Bodies push each other apart, links pull
//! their ends together, and a weak pull keeps everything near the origin. Bodies are boxes the size of their
//! labels, and overlapping boxes push apart. The motion cools and stops, so a settled graph costs nothing.
//! Pure: no drawing and no input.

use eframe::egui::{Pos2, Vec2};
use std::collections::HashMap;
use std::hash::Hash;

/// Space kept between two labels' boxes.
const PAD: f32 = 10.0;
const DAMPING: f32 = 0.6;
/// Heat multiplies every move and decays each iteration. Below `COLD` the graph is settled.
const COOLING: f32 = 0.985;
/// Iterations per frame. More converge further within the 2 second settle budget.
const ITERATIONS: usize = 3;
const COLD: f32 = 0.005;

/// The forces, in the user's units, each 1.0 near Obsidian's own scale. The defaults settle 50 nodes in 2 seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Forces {
    /// Pull toward the middle, 0 to 1.
    pub center: f32,
    /// Push between every two bodies, 0 to 20.
    pub repel: f32,
    /// Pull along links, 0 to 1.
    pub link: f32,
}

impl Default for Forces {
    fn default() -> Forces {
        Forces { center: 0.2, repel: 10.0, link: 0.5 }
    }
}

pub struct Spec<K> {
    pub key: K,
    pub link: Option<(K, f32)>,
    pub size: Vec2,
}

struct Body {
    pos: Pos2,
    vel: Vec2,
    size: Vec2,
}

pub struct Graph<K> {
    bodies: HashMap<K, Body>,
    links: Vec<(K, K, f32)>,
    pinned: Option<K>,
    forces: Forces,
    heat: f32,
}

impl<K> Default for Graph<K> {
    fn default() -> Graph<K> {
        Graph { bodies: HashMap::new(), links: Vec::new(), pinned: None, forces: Forces::default(), heat: 1.0 }
    }
}

impl<K: Copy + Eq + Hash> Graph<K> {
    /// Makes the bodies match `specs`, in order: spawns new keys next to their link and drops missing keys.
    /// Any change, a new box size included, reheats the graph. `place` can choose where a new body spawns instead.
    pub fn sync(&mut self, specs: &[Spec<K>], place: impl Fn(&K, Pos2) -> Option<Pos2>) {
        let before = self.bodies.len();
        self.bodies.retain(|k, _| specs.iter().any(|s| s.key == *k));
        let mut changed = self.bodies.len() != before;
        let sectors = sectors(specs);
        for spec in specs {
            if let Some(body) = self.bodies.get_mut(&spec.key) {
                changed |= (body.size - spec.size).length() > 0.5;
                body.size = spec.size;
                continue;
            }
            let anchor = spec.link.and_then(|(to, _)| self.bodies.get(&to)).map_or(Pos2::ZERO, |b| b.pos);
            let pos = place(&spec.key, anchor).unwrap_or_else(|| {
                let Some((_, len)) = spec.link else { return anchor };
                let (start, width) = sectors[&spec.key];
                anchor + Vec2::angled(start + width / 2.0) * len
            });
            self.bodies.insert(spec.key, Body { pos, vel: Vec2::ZERO, size: spec.size });
            changed = true;
        }
        let links: Vec<_> = specs.iter().filter_map(|s| s.link.map(|(to, len)| (s.key, to, len))).collect();
        changed |= links != self.links;
        self.links = links;
        if changed {
            self.heat = self.heat.max(0.6);
        }
    }

    /// Reheats when the forces change, so the graph moves to its new balance.
    pub fn set_forces(&mut self, forces: Forces) {
        if forces != self.forces {
            self.forces = forces;
            self.heat = self.heat.max(0.6);
        }
    }

    /// Forgets every position, so the next `sync` places every body from scratch.
    pub fn rearrange(&mut self) {
        self.bodies.clear();
        self.heat = 1.0;
    }

    pub fn pos(&self, key: &K) -> Option<Pos2> {
        self.bodies.get(key).map(|b| b.pos)
    }

    /// Holds `key` at `pos` until `release`, and keeps the rest moving around it.
    pub fn drag(&mut self, key: K, pos: Pos2) {
        if let Some(body) = self.bodies.get_mut(&key) {
            body.pos = pos;
            body.vel = Vec2::ZERO;
            self.pinned = Some(key);
            self.heat = self.heat.max(0.3);
        }
    }

    pub fn release(&mut self) {
        self.pinned = None;
    }

    pub fn settled(&self) -> bool {
        self.heat < COLD && self.pinned.is_none()
    }

    /// Advances the simulation one frame. Does nothing once settled.
    pub fn step(&mut self) {
        for _ in 0..ITERATIONS {
            if !self.settled() {
                self.iterate();
            }
        }
    }

    fn iterate(&mut self) {
        let keys: Vec<K> = self.bodies.keys().copied().collect();
        let pos: Vec<Pos2> = keys.iter().map(|k| self.bodies[k].pos).collect();
        let size: Vec<Vec2> = keys.iter().map(|k| self.bodies[k].size).collect();
        let (gravity, repel, spring) = (self.forces.center * 0.01, self.forces.repel * 2000.0, self.forces.link * 0.1);
        let mut force = vec![Vec2::ZERO; keys.len()];
        // shortcut: all pairs is O(n²), fine below the spec's 500 nodes, use a grid or Barnes-Hut past that.
        for i in 0..keys.len() {
            for j in i + 1..keys.len() {
                let mut d = pos[i] - pos[j];
                if d.length_sq() < 1.0 {
                    // Two bodies on one spot have no direction to push apart in. Invent one.
                    d = Vec2::angled(i as f32 + j as f32);
                }
                let push = d.normalized() * repel / d.length_sq().max(100.0);
                force[i] += push;
                force[j] -= push;
            }
            force[i] -= pos[i].to_vec2() * gravity;
        }
        let index: HashMap<K, usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
        for (a, b, len) in &self.links {
            let (Some(&i), Some(&j)) = (index.get(a), index.get(b)) else { continue };
            let d = pos[j] - pos[i];
            let pull = d.normalized() * (d.length() - len) * spring;
            force[i] += pull;
            force[j] -= pull;
        }
        for (i, key) in keys.iter().enumerate() {
            if self.pinned == Some(*key) {
                continue;
            }
            let body = self.bodies.get_mut(key).unwrap();
            body.vel = (body.vel + force[i]) * DAMPING;
            body.pos += body.vel * self.heat;
        }
        self.separate(&keys, &size);
        self.heat *= COOLING;
    }

    /// Moves overlapping boxes straight apart, whatever the heat, so no two labels stay on top of each other
    /// when the graph cools. Each moves half the overlap along the axis that overlaps less.
    fn separate(&mut self, keys: &[K], size: &[Vec2]) {
        let mut pos: Vec<Pos2> = keys.iter().map(|k| self.bodies[k].pos).collect();
        let held: Vec<bool> = keys.iter().map(|k| self.pinned == Some(*k)).collect();
        for i in 0..keys.len() {
            for j in i + 1..keys.len() {
                let d = pos[i] - pos[j];
                let overlap = (size[i] + size[j]) / 2.0 + Vec2::splat(PAD) - d.abs();
                // A held body passes over the others, so it can be dropped onto one.
                if overlap.x <= 0.0 || overlap.y <= 0.0 || held[i] || held[j] {
                    continue;
                }
                // Labels are wide, so the shallower way out is mostly up or down.
                let push = if overlap.x < overlap.y { Vec2::new(overlap.x * d.x.signum(), 0.0) } else { Vec2::new(0.0, overlap.y * d.y.signum()) };
                pos[i] += push * 0.5;
                pos[j] -= push * 0.5;
            }
        }
        for (key, pos) in keys.iter().zip(pos) {
            self.bodies.get_mut(key).unwrap().pos = pos;
        }
    }
}

/// Gives every body a slice of the circle inside its parent's slice. The slice is as wide as the body's share
/// of the bodies below the parent. So a whole tree spawns as a fan, with each subtree facing its own way. `(start, width)` in radians.
/// Parents come before their children in `specs`.
fn sectors<K: Copy + Eq + Hash>(specs: &[Spec<K>]) -> HashMap<K, (f32, f32)> {
    let mut below: HashMap<K, f32> = specs.iter().map(|s| (s.key, 1.0)).collect();
    for spec in specs.iter().rev() {
        if let Some((to, _)) = spec.link {
            let own = below[&spec.key];
            *below.entry(to).or_insert(1.0) += own;
        }
    }
    let mut sectors = HashMap::new();
    // The next free angle inside each parent's slice.
    let mut cursor: HashMap<K, f32> = HashMap::new();
    for spec in specs {
        let sector = match spec.link.and_then(|(to, _)| sectors.get(&to).copied().map(|s| (to, s))) {
            Some((to, (start, width))) => {
                let share = width * below[&spec.key] / (below[&to] - 1.0);
                let at = cursor.entry(to).or_insert(start);
                let sector = (*at, share);
                *at += share;
                sector
            }
            None => (0.0, std::f32::consts::TAU),
        };
        sectors.insert(spec.key, sector);
    }
    sectors
}

#[cfg(test)]
mod tests;
