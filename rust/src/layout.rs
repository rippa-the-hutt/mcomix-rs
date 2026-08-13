//! Smart-scroll layout engine, ported from `mcomix/box.py`, `scrolling.py`
//! and `layout.py`. All geometry is 2D (our viewport is always 2D).

use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// Box (2D)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Box2 {
    pub pos: [i64; 2],
    pub size: [i64; 2],
}

impl Box2 {
    pub fn new(pos: [i64; 2], size: [i64; 2]) -> Box2 {
        Box2 { pos, size }
    }

    pub fn from_size(size: [i64; 2]) -> Box2 {
        Box2 { pos: [0, 0], size }
    }

    pub fn set_position(&self, pos: [i64; 2]) -> Box2 {
        Box2 { pos, size: self.size }
    }

    pub fn translate(&self, delta: [i64; 2]) -> Box2 {
        Box2 {
            pos: [self.pos[0] + delta[0], self.pos[1] + delta[1]],
            size: self.size,
        }
    }

    pub fn translate_opposite(&self, delta: [i64; 2]) -> Box2 {
        Box2 {
            pos: [self.pos[0] - delta[0], self.pos[1] - delta[1]],
            size: self.size,
        }
    }

    /// Square of the Euclidean distance between this Box and a point (0 if the
    /// point lies inside).
    pub fn distance_point_squared(&self, point: [i64; 2]) -> i64 {
        let mut result = 0;
        for i in 0..2 {
            let p = point[i];
            let bs = self.pos[i];
            let be = self.size[i] + bs;
            let r = if p < bs {
                bs - p
            } else if p >= be {
                p - be + 1
            } else {
                continue;
            };
            result += r * r;
        }
        result
    }

    /// Center of the box; half-up/down chosen by `orientation` (1 or -1).
    pub fn get_center(&self, orientation: [i64; 2]) -> [i64; 2] {
        let mut result = [0i64; 2];
        for i in 0..2 {
            result[i] = Self::box_to_center_offset_1d(self.size[i] - 1, orientation[i]) + self.pos[i];
        }
        result
    }

    fn box_to_center_offset_1d(box_size_delta: i64, orientation: i64) -> i64 {
        let mut d = box_size_delta;
        if orientation == -1 {
            d += 1;
        }
        d >> 1
    }

    /// Compare two boxes' distance to the origin implied by `orientation`.
    fn compare_distance_to_origin(box1: &Box2, box2: &Box2, orientation: [i64; 2]) -> i64 {
        for i in 0..2 {
            let o = orientation[i];
            if o == 0 {
                continue;
            }
            let mut b1 = box1.pos[i];
            let mut b2 = box2.pos[i];
            if o < 0 {
                b1 = box1.size[i] - b1;
                b2 = box2.size[i] - b2;
            }
            let d = b1 - b2;
            if d != 0 {
                return d;
            }
        }
        0
    }

    /// Indices of the boxes closest to `point` (ties resolved by orientation).
    pub fn closest_boxes(point: [i64; 2], boxes: &[Box2], orientation: Option<[i64; 2]>) -> Vec<usize> {
        let mut result: Vec<usize> = Vec::new();
        let mut mindist: i64 = -1;
        for (i, b) in boxes.iter().enumerate() {
            let dist = b.distance_point_squared(point);
            let mut keep: u8 = 0; // 0 keep, 1 append, 2 replace
            if result.is_empty() || dist < mindist {
                keep = 2;
            } else if dist == mindist {
                if let Some(orientation) = orientation {
                    let mut done = false;
                    for ri in 0..result.len() {
                        let c = Self::compare_distance_to_origin(b, &boxes[result[ri]], orientation);
                        if c < 0 {
                            keep = 2;
                            done = true;
                            break;
                        }
                        if c == 0 {
                            keep = 1;
                        }
                    }
                    let _ = done;
                } else {
                    keep = 1;
                }
            }
            if keep == 1 {
                result.push(i);
            }
            if keep == 2 {
                mindist = dist;
                result = vec![i];
            }
        }
        result
    }

    /// Index of the box closest to this box's center.
    pub fn current_box_index(&self, orientation: [i64; 2], boxes: &[Box2]) -> usize {
        Self::closest_boxes(self.get_center(orientation), boxes, Some(orientation))[0]
    }

    /// Align boxes so their centers lie on the same line along `axis`.
    pub fn align_center(boxes: &[Box2], axis: usize, fix: usize, orientation: i64) -> Vec<Box2> {
        if boxes.is_empty() {
            return Vec::new();
        }
        let center_box = &boxes[fix];
        let mut cs = center_box.size[axis];
        if cs % 2 != 0 {
            cs += 1;
        }
        let cp = center_box.pos[axis];
        let mut result = Vec::with_capacity(boxes.len());
        for b in boxes {
            let s = b.size;
            let mut p = b.pos;
            p[axis] = cp + Self::box_to_center_offset_1d(cs - s[axis], orientation);
            result.push(Box2::new(p, s));
        }
        result
    }

    /// Distribute boxes along `axis` so they do not overlap, keeping `fix` fixed.
    pub fn distribute(boxes: &[Box2], axis: usize, fix: usize, spacing: i64) -> Vec<Box2> {
        if boxes.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<Box2> = vec![Box2::new([0, 0], [0, 0]); boxes.len()];
        let initial_sum = boxes[fix].pos[axis];
        let mut partial_sum = initial_sum;
        for bi in fix..boxes.len() {
            let b = &boxes[bi];
            let s = b.size;
            let mut p = b.pos;
            p[axis] = partial_sum;
            result[bi] = Box2::new(p, s);
            partial_sum += s[axis] + spacing;
        }
        partial_sum = initial_sum;
        for bi in (0..fix).rev() {
            let b = &boxes[bi];
            let s = b.size;
            let mut p = b.pos;
            partial_sum -= s[axis] + spacing;
            p[axis] = partial_sum;
            result[bi] = Box2::new(p, s);
        }
        result
    }

    /// The smallest box that contains this box and has at least the viewport
    /// size (i.e. the scrollable "wrapper" box).
    pub fn wrapper_box(&self, viewport_size: [i64; 2], orientation: [i64; 2]) -> Box2 {
        let size = self.size;
        let position = self.pos;
        let mut rs = [0i64; 2];
        let mut rp = [0i64; 2];
        for i in 0..2 {
            let c = size[i];
            let v = viewport_size[i];
            rs[i] = c.max(v);
            rp[i] = Self::box_to_center_offset_1d(c - rs[i], orientation[i]) + position[i];
        }
        Box2::new(rp, rs)
    }

    /// The smallest box containing all boxes.
    pub fn bounding_box(boxes: &[Box2]) -> Box2 {
        if boxes.is_empty() {
            return Box2::new([0, 0], [0, 0]);
        }
        let mut mins = [i64::MAX; 2];
        let mut maxes = [i64::MIN; 2];
        for b in boxes {
            for i in 0..2 {
                mins[i] = mins[i].min(b.pos[i]);
                maxes[i] = maxes[i].max(b.pos[i] + b.size[i]);
            }
        }
        Box2::new(mins, [maxes[0] - mins[0], maxes[1] - mins[1]])
    }

    /// Intersection of two boxes (may have negative size = empty).
    pub fn intersect(a: &Box2, b: &Box2) -> Box2 {
        let mut rp = [0i64; 2];
        let mut rs = [0i64; 2];
        for i in 0..2 {
            let mut ax1 = a.pos[i];
            let mut bx1 = b.pos[i];
            let mut ax2 = ax1 + a.size[i];
            let bx2 = bx1 + b.size[i];
            if ax1 < bx1 {
                ax1 = bx1;
            }
            if ax2 > bx2 {
                ax2 = bx2;
            }
            ax2 -= ax1;
            rp[i] = ax1;
            rs[i] = ax2;
        }
        Box2::new(rp, rs)
    }
}

// ---------------------------------------------------------------------------
// Scrolling
// ---------------------------------------------------------------------------

/// Binary search (like Python's bisect_left): returns the index of `value`,
/// or the bitwise complement of the insertion point when not found.
fn bin_search(lst: &[i64], value: i64) -> isize {
    let mut lo = 0usize;
    let mut hi = lst.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if lst[mid] < value {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < lst.len() && lst[lo] == value {
        lo as isize
    } else {
        !(lo as isize)
    }
}

/// Bresenham-derived grid sums, mirroring `Scrolling._bresenham_sums`.
fn bresenham_sums(num: i64, denom: i64, half_up: bool) -> Vec<i64> {
    assert!(num >= 0, "num < 0");
    assert!(denom >= 1, "denom < 1");
    let quotient = num / denom;
    let remainder = num % denom;
    let needs_up = half_up && remainder != 0 && (denom & 1) == 0;
    let mut up_flag = false;
    let mut error = denom >> 1;
    let mut result = vec![0i64];
    let mut partial_sum = 0i64;
    for _ in 0..denom {
        error -= remainder;
        if error < 0 {
            error += denom;
            partial_sum += quotient + 1;
        } else {
            partial_sum += quotient;
        }
        if up_flag {
            partial_sum -= 1;
            up_flag = false;
        } else if needs_up && error == 0 {
            partial_sum += 1;
            up_flag = true;
        }
        result.push(partial_sum);
    }
    result
}

fn remap_axes(vector: [i64; 2], order: [usize; 2]) -> [i64; 2] {
    [vector[order[0]], vector[order[1]]]
}

fn inverse_axis_map(order: [usize; 2]) -> [usize; 2] {
    let mut inv = [0usize; 2];
    for i in 0..2 {
        inv[order[i]] = i;
    }
    inv
}

pub struct Scrolling;

impl Scrolling {
    /// Smart-scroll one step. Returns the new viewport position, or `None`
    /// when there is no more space (i.e. the page should flip).
    pub fn scroll_smartly(
        content: &Box2,
        viewport: &Box2,
        orientation: [i64; 2],
        max_scroll: [f64; 2],
        axis_map: Option<[usize; 2]>,
    ) -> Option<[i64; 2]> {
        let offset = content.pos;
        let mut content_size = content.size;
        let mut viewport_size = viewport.size;
        let mut viewport_position = [
            viewport.pos[0] - offset[0],
            viewport.pos[1] - offset[1],
        ];
        let mut orientation = orientation;
        let mut max_scroll = max_scroll;
        if let Some(map) = axis_map {
            content_size = remap_axes(content_size, map);
            viewport_size = remap_axes(viewport_size, map);
            viewport_position = remap_axes(viewport_position, map);
            orientation = remap_axes(orientation, map);
            max_scroll = [max_scroll[map[0]], max_scroll[map[1]]];
        }

        let mut result = viewport_position;
        let mut carry = true;
        let mut reset_all_axes = false;

        for i in 0..2 {
            let invisible_size = content_size[i] - viewport_size[i];
            let o = orientation[i];
            if o == 1 {
                if viewport_position[i] < 0 {
                    result[i] = 0;
                    carry = false;
                    if viewport_position[i] <= -viewport_size[i] {
                        reset_all_axes = true;
                        break;
                    }
                }
            } else {
                // o == -1
                if viewport_position[i] > invisible_size {
                    result[i] = invisible_size;
                    carry = false;
                    if viewport_position[i] > content_size[i] {
                        reset_all_axes = true;
                        break;
                    }
                }
            }
        }
        if reset_all_axes {
            for i in 0..2 {
                let invisible_size = content_size[i] - viewport_size[i];
                let o = orientation[i];
                result[i] = if o == 1 { 0 } else { invisible_size };
            }
        }

        if carry {
            for i in 0..2 {
                let invisible_size = content_size[i] - viewport_size[i];
                let o = orientation[i];
                let ms = max_scroll[i].min(invisible_size as f64);
                let steps_to_take: i64;
                if ms != 0.0 {
                    steps_to_take = (invisible_size as f64 / ms).ceil() as i64;
                } else {
                    steps_to_take = 0;
                }
                if ms == 0.0 || steps_to_take >= invisible_size {
                    // Special case: must go forward by at least 1 pixel.
                    if o >= 0 {
                        result[i] += 1;
                        carry = result[i] > invisible_size;
                        if carry {
                            result[i] = 0;
                            continue;
                        }
                    } else {
                        result[i] -= 1;
                        carry = result[i] < 0;
                        if carry {
                            result[i] = invisible_size;
                            continue;
                        }
                    }
                    break;
                }
                let positions = bresenham_sums(invisible_size, steps_to_take, o == -1);
                let mut index = bin_search(&positions, viewport_position[i]) as i64;
                if index < 0 {
                    // Between two grid points: the insertion point is ~index.
                    index = !index;
                    if o >= 0 {
                        index -= 1;
                    }
                }
                index += o;
                carry = index < 0 || index >= positions.len() as i64;
                if carry {
                    // No space left in this dimension: reset it and carry to
                    // the next one.
                    result[i] = if o > 0 { 0 } else { invisible_size };
                } else {
                    result[i] = positions[index as usize];
                    break;
                }
            }
        }

        if carry {
            return None;
        }

        if let Some(map) = axis_map {
            result = remap_axes(result, inverse_axis_map(map));
        }
        Some([result[0] + offset[0], result[1] + offset[1]])
    }

    /// Scroll to a predefined destination (SCROLL_TO_START/END/CENTER).
    pub fn scroll_to_predefined(
        content: &Box2,
        viewport: &Box2,
        orientation: [i64; 2],
        destination: [i64; 2],
    ) -> [i64; 2] {
        let content_position = content.pos;
        let content_size = content.size;
        let viewport_size = viewport.size;
        let mut result = viewport.pos;
        for i in 0..2 {
            let o = orientation[i];
            let mut d = destination[i];
            if d == 0 {
                continue;
            }
            if d < -2 || d > 1 {
                panic!("invalid destination {d} at index {i}");
            }
            if d == -2 {
                d = o; // SCROLL_TO_END (constants: END = -2? see below)
            }
            if d == -1 {
                d = -o; // SCROLL_TO_START
            }
            let c = content_size[i];
            let v = viewport_size[i];
            let invisible_size = c - v;
            result[i] = content_position[i]
                + if d == 0 {
                    // SCROLL_TO_CENTER
                    Box2::box_to_center_offset_1d(invisible_size, o)
                } else if d == 1 {
                    invisible_size
                } else {
                    0
                };
        }
        result
    }
}

// ---------------------------------------------------------------------------
// FiniteLayout
// ---------------------------------------------------------------------------

pub const SCROLL_TO_CENTER: i64 = 0;
pub const SCROLL_TO_START: i64 = -1;
pub const SCROLL_TO_END: i64 = -2;

pub struct FiniteLayout {
    pub content_boxes: Vec<Box2>,
    pub wrapper_boxes: Vec<Box2>,
    pub union_box: Box2,
    pub viewport_box: Box2,
    pub orientation: [i64; 2],
    current_index: i64,
    dirty_current_index: bool,
}

impl FiniteLayout {
    pub fn new(
        content_sizes: &[[i64; 2]],
        viewport_size: [i64; 2],
        orientation: [i64; 2],
        spacing: i64,
        wrap_individually: bool,
        distribution_axis: usize,
        alignment_axis: usize,
    ) -> FiniteLayout {
        let mut fl = FiniteLayout {
            content_boxes: Vec::new(),
            wrapper_boxes: Vec::new(),
            union_box: Box2::new([0, 0], [0, 0]),
            viewport_box: Box2::from_size(viewport_size),
            orientation,
            current_index: -1,
            dirty_current_index: true,
        };
        fl.reset(content_sizes, viewport_size, orientation, spacing, wrap_individually, distribution_axis, alignment_axis);
        fl
    }

    pub fn reset(
        &mut self,
        content_sizes: &[[i64; 2]],
        viewport_size: [i64; 2],
        orientation: [i64; 2],
        spacing: i64,
        wrap_individually: bool,
        distribution_axis: usize,
        alignment_axis: usize,
    ) {
        let mut content_sizes: Vec<[i64; 2]> = content_sizes.to_vec();
        if orientation[distribution_axis] == -1 {
            content_sizes.reverse();
        }
        let mut cb: Vec<Box2> = content_sizes.iter().map(|s| Box2::from_size(*s)).collect();
        cb = Box2::align_center(&cb, alignment_axis, 0, orientation[alignment_axis]);
        cb = Box2::distribute(&cb, distribution_axis, 0, spacing);

        let (wb, bb) = if wrap_individually {
            let w: Vec<Box2> = cb
                .iter()
                .map(|b| b.wrapper_box(viewport_size, orientation))
                .collect();
            let bb = Box2::bounding_box(&w);
            (w, bb)
        } else {
            let bb = Box2::bounding_box(&cb).wrapper_box(viewport_size, orientation);
            (vec![bb], bb)
        };

        // Move to global origin.
        let bbp = bb.pos;
        for b in cb.iter_mut() {
            *b = b.translate_opposite(bbp);
        }
        let mut wb = wb;
        for b in wb.iter_mut() {
            *b = b.translate_opposite(bbp);
        }
        let bb = bb.translate_opposite(bbp);

        if orientation[distribution_axis] == -1 {
            cb.reverse();
            wb.reverse();
        }

        self.content_boxes = cb;
        self.wrapper_boxes = wb;
        self.union_box = bb;
        self.viewport_box = Box2::from_size(viewport_size);
        self.orientation = orientation;
        self.dirty_current_index = true;
    }

    pub fn set_orientation(&mut self, orientation: [i64; 2]) {
        self.orientation = orientation;
    }

    pub fn set_viewport_position(&mut self, position: [i64; 2]) {
        self.viewport_box = self.viewport_box.set_position(position);
        self.dirty_current_index = true;
    }

    pub fn get_current_index(&mut self) -> usize {
        if self.dirty_current_index {
            self.current_index = self.viewport_box.current_box_index(self.orientation, &self.content_boxes) as i64;
            self.dirty_current_index = false;
        }
        self.current_index.max(0) as usize
    }

    /// Smart scroll; returns the new viewport position or None to flip pages.
    pub fn scroll_smartly(
        &mut self,
        max_scroll: [f64; 2],
        backwards: bool,
        axis_map: Option<[usize; 2]>,
    ) -> Option<[i64; 2]> {
        let mut o = self.orientation;
        if backwards {
            o = [-o[0], -o[1]];
        }
        let n = self.content_boxes.len();
        let result = Scrolling::scroll_smartly(
            &self.wrapper_boxes[0],
            &self.viewport_box,
            o,
            max_scroll,
            axis_map,
        );
        match result {
            Some(pos) => {
                self.set_viewport_position(pos);
                Some(pos)
            }
            None => {
                // Determine whether we ran out at the start or end.
                let _ = n;
                None
            }
        }
    }

    pub fn scroll_to_predefined(&mut self, destination: [i64; 2], index: Option<usize>) {
        let current_box = match index {
            None => {
                let idx = self.get_current_index();
                self.wrapper_boxes[idx.min(self.wrapper_boxes.len() - 1)]
            }
            Some(i) => self.wrapper_boxes[i.min(self.wrapper_boxes.len() - 1)],
        };
        let pos = Scrolling::scroll_to_predefined(
            &current_box,
            &self.viewport_box,
            self.orientation,
            destination,
        );
        self.set_viewport_position(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_basics() {
        let b = Box2::new([10, 20], [100, 200]);
        assert_eq!(b.distance_point_squared([10, 20]), 0);
        assert!(b.distance_point_squared([5, 20]) > 0);
        assert_eq!(b.translate([1, 1]).pos, [11, 21]);
        assert_eq!(b.get_center([1, 1]), [59, 119]);
        // odd size, orientation -1 rounds up
        let c = Box2::new([0, 0], [101, 201]);
        assert_eq!(c.get_center([-1, -1]), [50, 100]);
    }

    #[test]
    fn bresenham_sums_grid() {
        // 100 px, 3 steps -> [0, 33, 66, 100] (roughly)
        let s = bresenham_sums(100, 3, false);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], 0);
        assert_eq!(*s.last().unwrap(), 100);
    }

    #[test]
    fn smart_scroll_steps_through_page() {
        // Content 1000x800, viewport 500x400, orientation forward.
        let content = Box2::from_size([1000, 800]);
        let viewport = Box2::from_size([500, 400]);
        // First step moves by ~500 horizontally.
        let p1 = Scrolling::scroll_smartly(&content, &viewport, [1, 1], [500.0, 500.0], None).unwrap();
        assert!(p1[0] > 0);
        // Keep going until exhausted.
        let mut vp = viewport;
        let mut steps = 0;
        loop {
            match Scrolling::scroll_smartly(&content, &vp, [1, 1], [500.0, 500.0], None) {
                Some(p) => {
                    vp = vp.set_position(p);
                    steps += 1;
                    assert!(steps < 20);
                }
                None => break,
            }
        }
        assert!(steps >= 2);
    }

    #[test]
    fn layout_builds_union_and_scrolls() {
        // Two pages side by side (double page), 500 wide each.
        let mut layout = FiniteLayout::new(
            &[[500, 800], [500, 800]],
            [600, 400],
            [1, 1],
            2,
            false,
            0,
            1,
        );
        assert_eq!(layout.content_boxes.len(), 2);
        assert_eq!(layout.union_box.size[0], 1002);
        // Smart scroll through: first step moves right.
        let p1 = layout
            .scroll_smartly([400.0, 400.0], false, None)
            .expect("moves");
        assert!(p1[0] > 0);
    }

    #[test]
    fn scroll_to_predefined_ends() {
        let mut layout = FiniteLayout::new(&[[1000, 800]], [500, 400], [1, 1], 2, false, 0, 1);
        layout.scroll_to_predefined([SCROLL_TO_END, SCROLL_TO_END], None);
        let pos = layout.viewport_box.pos;
        assert_eq!(pos[0], 1000 - 500);
        assert_eq!(pos[1], 800 - 400);
        layout.scroll_to_predefined([SCROLL_TO_START, SCROLL_TO_START], None);
        assert_eq!(layout.viewport_box.pos, [0, 0]);
    }
}
