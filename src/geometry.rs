use std::collections::HashMap;

use rift_protocol::{
    ContainerNodeType, ContainerTreeNode, LayoutStateData, Point, Rect, Size, WindowData, WindowId,
};

use crate::rift::Gaps;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainerRect {
    pub rect: Rect,
    /// 1 for a direct child of root. Root is depth 0 and is drawn only when it
    /// holds the selection.
    pub depth: usize,
    /// This container holds the layout engine's selection.
    pub selected: bool,
}

/// One rect per container in the workspace, outermost first, so a renderer
/// drawing in order paints inner rects on top of their parents. Root is
/// included only when it holds the selection.
pub fn container_rects(
    layout: &LayoutStateData,
    windows: &[WindowData],
    gaps: Gaps,
) -> Vec<ContainerRect> {
    let frames: HashMap<WindowId, Rect> = windows.iter().map(|w| (w.id, w.frame)).collect();
    let mut out = Vec::new();
    // Root is normally skipped: it spans the whole workspace, so an outline
    // around it carries no information. The exception is when it holds the
    // selection — otherwise ascending to root looks identical to ascending to
    // any other undrawn state, and the command reads as a no-op.
    let root = &layout.container_tree;
    if root.is_selected
        && let Some(rect) = subtree_union(root, &frames)
    {
        out.push(ContainerRect {
            rect: outset(rect, gaps.inner_h / 2.0, gaps.inner_v / 2.0),
            depth: 0,
            selected: true,
        });
    }
    for child in &root.children {
        visit(child, 1, &frames, gaps, &mut out);
    }
    out.sort_by_key(|r| r.depth);
    out
}

fn visit(
    node: &ContainerTreeNode,
    depth: usize,
    frames: &HashMap<WindowId, Rect>,
    gaps: Gaps,
    out: &mut Vec<ContainerRect>,
) {
    if node.node_type == ContainerNodeType::Container
        && let Some(rect) = subtree_union(node, frames)
    {
        out.push(ContainerRect {
            rect: outset(rect, gaps.inner_h / 2.0, gaps.inner_v / 2.0),
            depth,
            selected: holds_selection(node),
        });
    }
    for child in &node.children {
        visit(child, depth + 1, frames, gaps, out);
    }
}

/// Union of the frames of every window leaf in this subtree. `None` when no
/// leaf has a known frame, which is why a container in another space or a tree
/// that has moved on since the frames were read produces no rect rather than a
/// zero-sized one.
fn subtree_union(node: &ContainerTreeNode, frames: &HashMap<WindowId, Rect>) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    collect(node, frames, &mut acc);
    acc
}

fn collect(node: &ContainerTreeNode, frames: &HashMap<WindowId, Rect>, acc: &mut Option<Rect>) {
    if node.node_type == ContainerNodeType::Window
        && let Some(id) = node.window_id
        && let Some(r) = frames.get(&id)
    {
        *acc = Some(match *acc {
            None => *r,
            Some(a) => union(a, *r),
        });
    }
    for child in &node.children {
        collect(child, frames, acc);
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let y1 = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    Rect {
        origin: Point { x: x0, y: y0 },
        size: Size { width: x1 - x0, height: y1 - y0 },
    }
}

/// The union spans member window edges, not container edges, so it sits inside
/// the true container bounds by one gap. Half a gap back out keeps a nested
/// outline off its children's edges without overlapping the parent's.
fn outset(r: Rect, dx: f64, dy: f64) -> Rect {
    Rect {
        origin: Point { x: r.origin.x - dx, y: r.origin.y - dy },
        size: Size { width: r.size.width + 2.0 * dx, height: r.size.height + 2.0 * dy },
    }
}

/// True when this node is the layout engine's selection, or is the immediate
/// parent of the selected window. Only the innermost match is flagged, so at
/// most one rect comes back selected. When the selection is a direct child of
/// root, no non-root container matches — root itself is handled separately.
fn holds_selection(node: &ContainerTreeNode) -> bool {
    node.is_selected
        || node
            .children
            .iter()
            .any(|c| c.node_type == ContainerNodeType::Window && c.is_selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> (LayoutStateData, Vec<WindowData>, Gaps) {
        let raw = std::fs::read_to_string(format!("tests/fixtures/{name}.json"))
            .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        (
            serde_json::from_value(v["layout"].clone()).unwrap(),
            serde_json::from_value(v["windows"].clone()).unwrap(),
            Gaps {
                inner_h: v["gaps"]["inner_h"].as_f64().unwrap(),
                inner_v: v["gaps"]["inner_v"].as_f64().unwrap(),
            },
        )
    }

    #[test]
    fn flat_layout_yields_no_container_rects() {
        let (l, w, g) = load("flat");
        assert!(container_rects(&l, &w, g).is_empty(), "root must be skipped");
    }

    #[test]
    fn empty_workspace_yields_nothing() {
        let (l, w, g) = load("empty");
        assert!(container_rects(&l, &w, g).is_empty());
    }

    #[test]
    fn nested_layout_yields_one_rect_per_non_root_container() {
        let (l, w, g) = load("nested3");
        let rects = container_rects(&l, &w, g);
        assert_eq!(rects.len(), 2, "two non-root containers in nested3");
        assert_eq!(rects[0].depth, 1, "outermost first");
        assert_eq!(rects[1].depth, 2);
    }

    #[test]
    fn inner_rect_is_contained_by_its_parent() {
        let (l, w, g) = load("nested3");
        let rects = container_rects(&l, &w, g);
        let outer = rects[0].rect;
        let inner = rects[1].rect;
        assert!(inner.origin.x >= outer.origin.x);
        assert!(inner.origin.y >= outer.origin.y);
        assert!(inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width);
        assert!(inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height);
    }

    #[test]
    fn no_rect_is_selected_when_the_selection_is_a_direct_child_of_root() {
        let (l, w, g) = load("nested3");
        let rects = container_rects(&l, &w, g);
        assert_eq!(rects.iter().filter(|r| r.selected).count(), 0);
    }

    #[test]
    fn exactly_the_innermost_container_is_selected_when_the_selection_is_nested() {
        let (l, w, g) = load("nested3_selected");
        let rects = container_rects(&l, &w, g);
        let selected: Vec<_> = rects.iter().filter(|r| r.selected).collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].depth, 2, "the innermost container, not an ancestor");
    }

    #[test]
    fn root_is_drawn_only_when_it_holds_the_selection() {
        let (mut l, w, g) = load("nested3");

        // As captured, the selection is on a top-level window, so root is not
        // drawn and only the two nested containers come back.
        assert!(!l.container_tree.is_selected);
        let before = container_rects(&l, &w, g);
        assert_eq!(before.len(), 2);
        assert!(before.iter().all(|r| r.depth > 0));

        // After an ascend to root, root must draw — otherwise the command has
        // no visible effect at all.
        clear_selection(&mut l.container_tree);
        l.container_tree.is_selected = true;
        let after = container_rects(&l, &w, g);
        assert_eq!(after.len(), 3);
        let root_rect = after.iter().find(|r| r.depth == 0).expect("root drawn");
        assert!(root_rect.selected);
        // Root spans everything, so nothing else can be wider.
        assert!(after.iter().all(|r| r.rect.size.width <= root_rect.rect.size.width));
    }

    fn clear_selection(n: &mut ContainerTreeNode) {
        n.is_selected = false;
        for c in &mut n.children {
            clear_selection(c);
        }
    }

    #[test]
    fn missing_frames_are_skipped_not_panicked() {
        let (l, _w, g) = load("nested3");
        let rects = container_rects(&l, &[], g);
        assert!(rects.is_empty(), "no frames means no rects, no panic");
    }
}
