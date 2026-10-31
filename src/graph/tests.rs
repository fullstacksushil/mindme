use super::*;

/// A tree of `n` nodes where node i links to node (i - 1) / 3.
fn tree(n: u32) -> Vec<Spec<u32>> {
    (0..n).map(|i| Spec { key: i, link: (i > 0).then(|| ((i - 1) / 3, 140.0)), size: Vec2::ZERO }).collect()
}

fn run(graph: &mut Graph<u32>) -> usize {
    let mut steps = 0;
    while !graph.settled() && steps < 1000 {
        graph.step();
        steps += 1;
    }
    steps
}

#[test]
fn fifty_nodes_settle_apart_and_near_their_parents() {
    let mut g = Graph::default();
    let specs = tree(50);
    g.sync(&specs, |_, _| None);
    let steps = run(&mut g);
    assert!(steps <= 120, "settles within 2 seconds at 60 fps, took {steps} steps");
    for a in 0..50 {
        for b in a + 1..50 {
            let d = g.pos(&a).unwrap().distance(g.pos(&b).unwrap());
            assert!(d > 50.0, "{a} and {b} overlap at {d}");
        }
    }
    for spec in &specs {
        if let Some((parent, _)) = spec.link {
            let d = g.pos(&spec.key).unwrap().distance(g.pos(&parent).unwrap());
            assert!(d < 400.0, "{} is {d} from its parent", spec.key);
        }
    }
}

#[test]
fn a_dragged_body_stays_put_and_the_rest_settles_after_release() {
    let mut g = Graph::default();
    g.sync(&tree(10), |_, _| None);
    run(&mut g);
    let target = Pos2::new(500.0, 500.0);
    for _ in 0..30 {
        g.drag(3, target);
        g.step();
    }
    assert_eq!(g.pos(&3), Some(target));
    assert!(!g.settled(), "keeps moving while held");
    g.release();
    assert!(run(&mut g) <= 120);
}

#[test]
fn sync_spawns_next_to_the_link_and_removes_missing_keys() {
    let mut g = Graph::default();
    g.sync(&tree(4), |_, _| None);
    run(&mut g);
    let parent = g.pos(&1).unwrap();
    let mut specs = tree(4);
    specs.push(Spec { key: 99, link: Some((1, 140.0)), size: Vec2::ZERO });
    specs.retain(|s| s.key != 3);
    g.sync(&specs, |_, _| None);
    assert!(g.pos(&99).unwrap().distance(parent) < 150.0);
    assert_eq!(g.pos(&3), None);
    assert!(!g.settled(), "a change reheats the graph");
}

#[test]
fn wide_labels_settle_without_overlapping() {
    let mut g = Graph::default();
    let mut specs = tree(60);
    for s in &mut specs {
        s.size = Vec2::new(170.0, 26.0);
    }
    g.sync(&specs, |_, _| None);
    run(&mut g);
    for a in 0..60u32 {
        for b in a + 1..60 {
            let d = (g.pos(&a).unwrap() - g.pos(&b).unwrap()).abs();
            assert!(d.x >= 170.0 || d.y >= 26.0, "{a} and {b} overlap: {d:?}");
        }
    }
}
