use rift_protocol::{ContainerNodeType, ContainerTreeNode, LayoutStateData, Rect};

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
///
/// Rects are the layout engine's own `frame` for each node, so nothing here
/// reconstructs geometry or corrects for gaps. Growing the band outward past
/// the frame is the renderer's `outset`.
pub fn container_rects(layout: &LayoutStateData) -> Vec<ContainerRect> {
    let mut out = Vec::new();
    // Root is normally skipped: it spans the whole workspace, so an outline
    // around it carries no information. The exception is when it holds the
    // selection — otherwise ascending to root looks identical to ascending to
    // any other undrawn state, and the command reads as a no-op.
    let root = &layout.container_tree;
    if root.is_selected && enclosable(root) {
        out.push(ContainerRect { rect: root.frame, depth: 0, selected: true });
    }
    let root_child_count = root.children.len();
    for child in &root.children {
        visit(child, 1, root_child_count, &mut out);
    }
    out.sort_by_key(|r| r.depth);
    out
}

fn visit(node: &ContainerTreeNode, depth: usize, siblings: usize, out: &mut Vec<ContainerRect>) {
    // An only child is allocated exactly what its parent is allocated — rift's
    // propagate_single_child_allocations copies the parent's frame into it
    // verbatim — so its outline cannot be told apart from the parent's and
    // carries no information. Skip it unless it is the selection, which always
    // has to be visible.
    let redundant = siblings == 1 && !node.is_selected;

    if node.node_type == ContainerNodeType::Container && !redundant && enclosable(node) {
        out.push(ContainerRect { rect: node.frame, depth, selected: holds_selection(node) });
    }
    let child_count = node.children.len();
    for child in &node.children {
        visit(child, depth + 1, child_count, out);
    }
}

/// Whether this node has anything for a band to enclose.
///
/// Both halves are load-bearing, and each catches a case the other misses:
///
///   - An empty workspace's root, and an empty only child that inherited the
///     parent's allocation, arrive with a real frame and no windows under
///     them. A layout engine that keeps structural children makes this
///     routine rather than exotic: master-stack's root holds a master and a
///     stack container whether or not either has windows, and BSP emits
///     `Placeholder` leaves for empty slots.
///   - A subtree whose every window is missing from the layout calculation
///     gets no union at all, so it keeps the zero frame rift built it with.
///     Grown by `outset`, that draws a small square at the screen origin.
///
/// This is what `subtree_union` returning `Option<Rect>` used to give for
/// free.
fn enclosable(node: &ContainerTreeNode) -> bool {
    node.frame.size.width > 0.0 && node.frame.size.height > 0.0 && has_window(node)
}

fn has_window(node: &ContainerTreeNode) -> bool {
    node.node_type == ContainerNodeType::Window || node.children.iter().any(has_window)
}

/// True only when this node *is* the layout engine's selection.
///
/// Deliberately not "or contains the selected window": the bright band means
/// "what the next structural command will act on", and with a window selected
/// that is the window, not any container. Conflating the two also made the
/// first `ascend` invisible — selecting a window and selecting its parent
/// container rendered identically, so the press looked like a no-op.
fn holds_selection(node: &ContainerTreeNode) -> bool {
    node.is_selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(name: &str) -> LayoutStateData {
        let raw = std::fs::read_to_string(format!("tests/fixtures/{name}.json"))
            .unwrap_or_else(|e| panic!("fixture {name}: {e}"));
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        serde_json::from_value(v["layout"].clone()).unwrap()
    }

    #[test]
    fn flat_layout_yields_no_container_rects() {
        let l = load("flat");
        assert!(container_rects(&l).is_empty(), "root must be skipped");
    }

    #[test]
    fn empty_workspace_yields_nothing() {
        // Root is selected here and arrives with the whole tiling area as its
        // frame, so only `enclosable` keeps this empty.
        let l = load("empty");
        assert!(l.container_tree.is_selected);
        assert!(l.container_tree.frame.size.width > 0.0);
        assert!(container_rects(&l).is_empty());
    }

    /// A container with a child but no window under it, shaped like the empty
    /// half of a master-stack root or a BSP split holding only placeholders.
    /// Built by editing a real node so the other eleven fields stay honest.
    fn windowless_container(template: &ContainerTreeNode) -> ContainerTreeNode {
        let mut placeholder = template.clone();
        placeholder.node_type = ContainerNodeType::Placeholder;
        placeholder.window_id = None;
        placeholder.is_selected = false;
        placeholder.children = vec![];
        placeholder.frame = Rect::default();

        let mut container = placeholder.clone();
        container.node_type = ContainerNodeType::Container;
        container.children = vec![placeholder];
        container
    }

    #[test]
    fn structural_children_without_windows_draw_nothing() {
        // master-stack keeps a master and a stack container on root whether or
        // not either holds windows, so a selected empty workspace has a real
        // frame *and* children. Counting children would draw a band around an
        // empty screen, and the two windowless containers would each draw a
        // square at the screen origin from their zero frames.
        let mut l = load("flat");
        let template = l.container_tree.children[0].clone();
        clear_selection(&mut l.container_tree);
        l.container_tree.is_selected = true;
        l.container_tree.children =
            vec![windowless_container(&template), windowless_container(&template)];

        assert!(l.container_tree.frame.size.width > 0.0, "root keeps the tiling area");
        assert!(container_rects(&l).is_empty());
    }

    #[test]
    fn a_windowless_container_beside_real_structure_is_skipped() {
        // Its zero frame would otherwise be grown by `outset` into a small
        // square at the screen origin, next to the real bands.
        let mut l = load("nested3");
        let template = l.container_tree.children[0].clone();
        let before = container_rects(&l);

        l.container_tree.children.push(windowless_container(&template));
        assert_eq!(container_rects(&l), before, "the real structure is unchanged");
    }

    #[test]
    fn a_container_rect_is_the_nodes_own_frame() {
        // The point of the whole module: no union, no gap arithmetic, no
        // outset. Whatever the layout engine allocated is what gets drawn.
        let l = load("nested3");
        let node = &l.container_tree.children[1];
        assert_eq!(node.node_type, ContainerNodeType::Container);
        let rects = container_rects(&l);
        assert_eq!(rects[0].rect, node.frame);
    }

    #[test]
    fn nested_layout_yields_one_rect_per_non_root_container() {
        let l = load("nested3");
        let rects = container_rects(&l);
        assert_eq!(rects.len(), 2, "two non-root containers in nested3");
        assert_eq!(rects[0].depth, 1, "outermost first");
        assert_eq!(rects[1].depth, 2);
    }

    #[test]
    fn inner_rect_is_contained_by_its_parent() {
        let l = load("nested3");
        let rects = container_rects(&l);
        let outer = rects[0].rect;
        let inner = rects[1].rect;
        assert!(inner.origin.x >= outer.origin.x);
        assert!(inner.origin.y >= outer.origin.y);
        assert!(inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width);
        assert!(inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height);
    }

    #[test]
    fn no_rect_is_selected_when_the_selection_is_a_direct_child_of_root() {
        let l = load("nested3");
        let rects = container_rects(&l);
        assert_eq!(rects.iter().filter(|r| r.selected).count(), 0);
    }

    #[test]
    fn a_selected_window_brightens_no_container() {
        // The fixture has a window selected, deep inside the tree. Nothing may
        // be bright: the next structural command acts on that window, not on a
        // container. If its parent were brightened, the first ascend would
        // render identically to this and look like a no-op.
        let l = load("nested3_selected");
        let rects = container_rects(&l);
        assert!(rects.iter().all(|r| !r.selected));
    }

    #[test]
    fn ascending_from_a_window_changes_what_is_bright() {
        let mut l = load("nested3_selected");
        let before = container_rects(&l);

        // Simulate one ascend: the selection moves from the window to its
        // parent container, the innermost one at depth 2.
        clear_selection(&mut l.container_tree);
        select_deepest_container(&mut l.container_tree);
        let after = container_rects(&l);

        assert_ne!(
            before.iter().map(|r| r.selected).collect::<Vec<_>>(),
            after.iter().map(|r| r.selected).collect::<Vec<_>>(),
            "one ascend must be visible"
        );
        let selected: Vec<_> = after.iter().filter(|r| r.selected).collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].depth, 2, "the container the window sat in");
    }

    /// Two passes: find the deepest container's depth, then flag the first
    /// container at that depth. Avoids holding a mutable borrow across a
    /// traversal.
    fn select_deepest_container(n: &mut ContainerTreeNode) {
        fn max_depth(n: &ContainerTreeNode, d: usize) -> usize {
            let here = if n.node_type == ContainerNodeType::Container { d } else { 0 };
            n.children.iter().map(|c| max_depth(c, d + 1)).chain([here]).max().unwrap()
        }
        fn flag(n: &mut ContainerTreeNode, d: usize, want: usize, done: &mut bool) {
            if !*done && d == want && n.node_type == ContainerNodeType::Container {
                n.is_selected = true;
                *done = true;
                return;
            }
            for c in &mut n.children {
                flag(c, d + 1, want, done);
            }
        }
        let want = max_depth(n, 0);
        flag(n, 0, want, &mut false);
    }

    #[test]
    fn an_only_child_container_is_skipped_as_redundant() {
        // Captured from a live layout whose root has exactly one child: rift
        // allocates that child the parent's frame verbatim, so its band is
        // indistinguishable from an outline of everything.
        let l = load("single_child_root");
        assert_eq!(l.container_tree.children.len(), 1);
        assert_eq!(l.container_tree.children[0].frame, l.container_tree.frame);

        let rects = container_rects(&l);
        assert!(
            rects.iter().all(|r| r.depth != 1),
            "the only child of root must not be drawn, got {:?}",
            rects.iter().map(|r| r.depth).collect::<Vec<_>>()
        );
        assert!(rects.iter().any(|r| r.depth == 2), "real structure still drawn");
    }

    #[test]
    fn an_only_child_container_is_drawn_when_selected() {
        // Skipping it would make ascending onto it look like nothing happened.
        let mut l = load("single_child_root");
        clear_selection(&mut l.container_tree);
        l.container_tree.children[0].is_selected = true;

        let rects = container_rects(&l);
        let sel: Vec<_> = rects.iter().filter(|r| r.selected).collect();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].depth, 1);
    }

    #[test]
    fn root_is_drawn_only_when_it_holds_the_selection() {
        let mut l = load("nested3");

        // As captured, the selection is on a top-level window, so root is not
        // drawn and only the two nested containers come back.
        assert!(!l.container_tree.is_selected);
        let before = container_rects(&l);
        assert_eq!(before.len(), 2);
        assert!(before.iter().all(|r| r.depth > 0));

        // After an ascend to root, root must draw — otherwise the command has
        // no visible effect at all.
        clear_selection(&mut l.container_tree);
        l.container_tree.is_selected = true;
        let after = container_rects(&l);
        assert_eq!(after.len(), 3);
        let root_rect = after.iter().find(|r| r.depth == 0).expect("root drawn");
        assert!(root_rect.selected);
        // Root is allocated the whole tiling area, so nothing else can be wider.
        assert!(after.iter().all(|r| r.rect.size.width <= root_rect.rect.size.width));
    }

    fn clear_selection(n: &mut ContainerTreeNode) {
        n.is_selected = false;
        for c in &mut n.children {
            clear_selection(c);
        }
    }
}
