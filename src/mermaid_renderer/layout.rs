use crate::mermaid_renderer::config::{LayoutConfig, PieRenderMode, TreemapRenderMode};
use crate::mermaid_renderer::ir::{Direction, Graph};
use crate::mermaid_renderer::text_metrics;
use crate::mermaid_renderer::theme::{Theme, adjust_color, parse_color_to_hsl};
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct TextBlock {
    pub lines: Vec<String>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub shape: crate::mermaid_renderer::ir::NodeShape,
    pub style: crate::mermaid_renderer::ir::NodeStyle,
    pub link: Option<crate::mermaid_renderer::ir::NodeLink>,
    pub anchor_subgraph: Option<usize>,
    pub hidden: bool,
}

#[derive(Debug, Clone)]
pub struct EdgeLayout {
    pub from: String,
    pub to: String,
    pub label: Option<TextBlock>,
    pub start_label: Option<TextBlock>,
    pub end_label: Option<TextBlock>,
    pub points: Vec<(f32, f32)>,
    pub directed: bool,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_start_kind: Option<crate::mermaid_renderer::ir::EdgeArrowhead>,
    pub arrow_end_kind: Option<crate::mermaid_renderer::ir::EdgeArrowhead>,
    pub start_decoration: Option<crate::mermaid_renderer::ir::EdgeDecoration>,
    pub end_decoration: Option<crate::mermaid_renderer::ir::EdgeDecoration>,
    pub style: crate::mermaid_renderer::ir::EdgeStyle,
    pub override_style: crate::mermaid_renderer::ir::EdgeStyleOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EdgeSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
struct EdgePortInfo {
    start_side: EdgeSide,
    end_side: EdgeSide,
    start_offset: f32,
    end_offset: f32,
}

#[derive(Debug, Clone)]
struct PortCandidate {
    edge_idx: usize,
    is_start: bool,
    other_pos: f32,
}

#[derive(Debug, Clone)]
pub struct SubgraphLayout {
    pub label: String,
    pub label_block: TextBlock,
    pub nodes: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub style: crate::mermaid_renderer::ir::NodeStyle,
}

#[derive(Debug, Clone)]
pub struct Lifeline {
    pub id: String,
    pub x: f32,
    pub y1: f32,
    pub y2: f32,
}

#[derive(Debug, Clone)]
pub struct SequenceLabel {
    pub x: f32,
    pub y: f32,
    pub text: TextBlock,
}

#[derive(Debug, Clone)]
pub struct SequenceFrameLayout {
    pub kind: crate::mermaid_renderer::ir::SequenceFrameKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label_box: (f32, f32, f32, f32),
    pub label: SequenceLabel,
    pub section_labels: Vec<SequenceLabel>,
    pub dividers: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SequenceBoxLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: Option<TextBlock>,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SequenceNoteLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub position: crate::mermaid_renderer::ir::SequenceNotePosition,
    pub participants: Vec<String>,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct StateNoteLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: TextBlock,
    pub position: crate::mermaid_renderer::ir::StateNotePosition,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct SequenceActivationLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub participant: String,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct SequenceNumberLayout {
    pub x: f32,
    pub y: f32,
    pub value: usize,
}

#[derive(Debug, Clone)]
pub struct PieSliceLayout {
    pub label: TextBlock,
    pub value: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct PieLegendItem {
    pub x: f32,
    pub y: f32,
    pub label: TextBlock,
    pub color: String,
    pub marker_size: f32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct PieTitleLayout {
    pub x: f32,
    pub y: f32,
    pub text: TextBlock,
}

#[derive(Debug, Clone)]
pub struct SankeyNodeLayout {
    pub id: String,
    pub label: String,
    pub total: f32,
    pub rank: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct SankeyLinkLayout {
    pub source: String,
    pub target: String,
    pub value: f32,
    pub thickness: f32,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub color_start: String,
    pub color_end: String,
    pub gradient_id: String,
}

#[derive(Debug, Clone)]
pub struct SankeyLayout {
    pub width: f32,
    pub height: f32,
    pub node_width: f32,
    pub nodes: Vec<SankeyNodeLayout>,
    pub links: Vec<SankeyLinkLayout>,
}

#[derive(Debug, Clone)]
pub struct GitGraphBranchLabelLayout {
    pub bg_x: f32,
    pub bg_y: f32,
    pub bg_width: f32,
    pub bg_height: f32,
    pub text_x: f32,
    pub text_y: f32,
    pub text_width: f32,
    pub text_height: f32,
}

#[derive(Debug, Clone)]
pub struct GitGraphBranchLayout {
    pub name: String,
    pub index: usize,
    pub pos: f32,
    pub label: GitGraphBranchLabelLayout,
}

#[derive(Debug, Clone)]
pub struct GitGraphTransform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub rotate_deg: f32,
    pub rotate_cx: f32,
    pub rotate_cy: f32,
}

#[derive(Debug, Clone)]
pub struct GitGraphCommitLabelLayout {
    pub text: String,
    pub text_x: f32,
    pub text_y: f32,
    pub bg_x: f32,
    pub bg_y: f32,
    pub bg_width: f32,
    pub bg_height: f32,
    pub transform: Option<GitGraphTransform>,
}

#[derive(Debug, Clone)]
pub struct GitGraphTagLayout {
    pub text: String,
    pub text_x: f32,
    pub text_y: f32,
    pub points: Vec<(f32, f32)>,
    pub hole_x: f32,
    pub hole_y: f32,
    pub transform: Option<GitGraphTransform>,
}

#[derive(Debug, Clone)]
pub struct GitGraphCommitLayout {
    pub id: String,
    pub seq: usize,
    pub branch_index: usize,
    pub x: f32,
    pub y: f32,
    pub axis_pos: f32,
    pub commit_type: crate::mermaid_renderer::ir::GitGraphCommitType,
    pub custom_type: Option<crate::mermaid_renderer::ir::GitGraphCommitType>,
    pub tags: Vec<GitGraphTagLayout>,
    pub label: Option<GitGraphCommitLabelLayout>,
}

#[derive(Debug, Clone)]
pub struct GitGraphArrowLayout {
    pub path: String,
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct GitGraphLayout {
    pub branches: Vec<GitGraphBranchLayout>,
    pub commits: Vec<GitGraphCommitLayout>,
    pub arrows: Vec<GitGraphArrowLayout>,
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub max_pos: f32,
    pub direction: Direction,
}

#[derive(Debug, Clone)]
pub struct ErrorLayout {
    pub viewbox_width: f32,
    pub viewbox_height: f32,
    pub render_width: f32,
    pub render_height: f32,
    pub message: String,
    pub version: String,
    pub text_x: f32,
    pub text_y: f32,
    pub text_size: f32,
    pub version_x: f32,
    pub version_y: f32,
    pub version_size: f32,
    pub icon_scale: f32,
    pub icon_tx: f32,
    pub icon_ty: f32,
}

#[derive(Debug, Clone)]
pub struct XYChartBarLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub value: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct XYChartLineLayout {
    pub points: Vec<(f32, f32)>,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct XYChartLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub x_axis_label: Option<TextBlock>,
    pub x_axis_label_y: f32,
    pub y_axis_label: Option<TextBlock>,
    pub y_axis_label_x: f32,
    pub x_axis_categories: Vec<(String, f32)>,
    pub y_axis_ticks: Vec<(String, f32)>,
    pub bars: Vec<XYChartBarLayout>,
    pub lines: Vec<XYChartLineLayout>,
    pub plot_x: f32,
    pub plot_y: f32,
    pub plot_width: f32,
    pub plot_height: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineEventLayout {
    pub time: TextBlock,
    pub events: Vec<TextBlock>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub circle_y: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineSectionLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub events: Vec<TimelineEventLayout>,
    pub sections: Vec<TimelineSectionLayout>,
    pub line_y: f32,
    pub line_start_x: f32,
    pub line_end_x: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct JourneyActorLayout {
    pub name: String,
    pub color: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct JourneyTaskLayout {
    pub id: String,
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub score: Option<f32>,
    pub score_color: String,
    pub score_y: f32,
    pub actors: Vec<String>,
    pub actor_y: Option<f32>,
    pub section_idx: usize,
}

#[derive(Debug, Clone)]
pub struct JourneySectionLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct JourneyLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub actors: Vec<JourneyActorLayout>,
    pub actor_label_y: f32,
    pub tasks: Vec<JourneyTaskLayout>,
    pub sections: Vec<JourneySectionLayout>,
    pub baseline: Option<(f32, f32, f32)>,
    pub score_radius: f32,
    pub actor_radius: f32,
    pub actor_gap: f32,
    pub card_gap_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub kind: crate::mermaid_renderer::ir::DiagramKind,
    pub nodes: BTreeMap<String, NodeLayout>,
    pub edges: Vec<EdgeLayout>,
    pub subgraphs: Vec<SubgraphLayout>,
    pub lifelines: Vec<Lifeline>,
    pub sequence_footboxes: Vec<NodeLayout>,
    pub sequence_boxes: Vec<SequenceBoxLayout>,
    pub sequence_frames: Vec<SequenceFrameLayout>,
    pub sequence_notes: Vec<SequenceNoteLayout>,
    pub sequence_activations: Vec<SequenceActivationLayout>,
    pub sequence_numbers: Vec<SequenceNumberLayout>,
    pub state_notes: Vec<StateNoteLayout>,
    pub pie_slices: Vec<PieSliceLayout>,
    pub pie_legend: Vec<PieLegendItem>,
    pub pie_center: (f32, f32),
    pub pie_radius: f32,
    pub pie_title: Option<PieTitleLayout>,
    pub quadrant: Option<QuadrantLayout>,
    pub gantt: Option<GanttLayout>,
    pub sankey: Option<SankeyLayout>,
    pub gitgraph: Option<GitGraphLayout>,
    pub c4: Option<C4Layout>,
    pub xychart: Option<XYChartLayout>,
    pub timeline: Option<TimelineLayout>,
    pub journey: Option<JourneyLayout>,
    pub error: Option<ErrorLayout>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct C4Layout {
    pub shapes: Vec<C4ShapeLayout>,
    pub boundaries: Vec<C4BoundaryLayout>,
    pub rels: Vec<C4RelLayout>,
    pub viewbox_x: f32,
    pub viewbox_y: f32,
    pub viewbox_width: f32,
    pub viewbox_height: f32,
    pub use_max_width: bool,
}

#[derive(Debug, Clone)]
pub struct C4TextLayout {
    pub text: String,
    pub lines: Vec<String>,
    pub width: f32,
    pub height: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct C4ShapeLayout {
    pub id: String,
    pub kind: crate::mermaid_renderer::ir::C4ShapeKind,
    pub bg_color: Option<String>,
    pub border_color: Option<String>,
    pub font_color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub type_label: C4TextLayout,
    pub label: C4TextLayout,
    pub type_or_techn: Option<C4TextLayout>,
    pub descr: Option<C4TextLayout>,
    pub image_y: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct C4BoundaryLayout {
    pub id: String,
    pub label: C4TextLayout,
    pub boundary_type: Option<C4TextLayout>,
    pub descr: Option<C4TextLayout>,
    pub bg_color: Option<String>,
    pub border_color: Option<String>,
    pub font_color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct C4RelLayout {
    pub kind: crate::mermaid_renderer::ir::C4RelKind,
    pub from: String,
    pub to: String,
    pub label: C4TextLayout,
    pub techn: Option<C4TextLayout>,
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub offset_x: f32,
    pub offset_y: f32,
    pub line_color: Option<String>,
    pub text_color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QuadrantLayout {
    pub title: Option<TextBlock>,
    pub title_y: f32,
    pub x_axis_left: Option<TextBlock>,
    pub x_axis_right: Option<TextBlock>,
    pub y_axis_bottom: Option<TextBlock>,
    pub y_axis_top: Option<TextBlock>,
    pub quadrant_labels: [Option<TextBlock>; 4],
    pub points: Vec<QuadrantPointLayout>,
    pub grid_x: f32,
    pub grid_y: f32,
    pub grid_width: f32,
    pub grid_height: f32,
}

#[derive(Debug, Clone)]
pub struct QuadrantPointLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct GanttLayout {
    pub title: Option<TextBlock>,
    pub sections: Vec<GanttSectionLayout>,
    pub tasks: Vec<GanttTaskLayout>,
    pub time_start: f32,
    pub time_end: f32,
    pub chart_x: f32,
    pub chart_y: f32,
    pub chart_width: f32,
    pub chart_height: f32,
    pub row_height: f32,
    pub label_x: f32,
    pub label_width: f32,
    pub section_label_x: f32,
    pub section_label_width: f32,
    pub task_label_x: f32,
    pub task_label_width: f32,
    pub title_y: f32,
    pub ticks: Vec<GanttTick>,
}

#[derive(Debug, Clone)]
pub struct GanttSectionLayout {
    pub label: TextBlock,
    pub y: f32,
    pub height: f32,
    pub color: String,
    pub band_color: String,
}

#[derive(Debug, Clone)]
pub struct GanttTaskLayout {
    pub label: TextBlock,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: String,
    pub start: f32,
    pub duration: f32,
    pub status: Option<crate::mermaid_renderer::ir::GanttStatus>,
}

#[derive(Debug, Clone)]
pub struct GanttTick {
    pub x: f32,
    pub label: String,
}

#[derive(Debug, Clone)]
struct Obstacle {
    id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    members: Option<HashSet<String>>,
}

fn is_horizontal(direction: Direction) -> bool {
    matches!(direction, Direction::LeftRight | Direction::RightLeft)
}

fn side_is_vertical(side: EdgeSide) -> bool {
    matches!(side, EdgeSide::Left | EdgeSide::Right)
}

fn edge_sides(
    from: &NodeLayout,
    to: &NodeLayout,
    direction: Direction,
) -> (EdgeSide, EdgeSide, bool) {
    let from_cx = from.x + from.width / 2.0;
    let from_cy = from.y + from.height / 2.0;
    let to_cx = to.x + to.width / 2.0;
    let to_cy = to.y + to.height / 2.0;
    let dx = to_cx - from_cx;
    let dy = to_cy - from_cy;
    let x_overlap = (from.x.max(to.x) - (from.x + from.width).min(to.x + to.width)).abs() < 1e-3
        || from.x < to.x + to.width && to.x < from.x + from.width;
    let y_overlap = (from.y.max(to.y) - (from.y + from.height).min(to.y + to.height)).abs() < 1e-3
        || from.y < to.y + to.height && to.y < from.y + from.height;

    let ratio = dx.abs() / (dy.abs().max(1e-3));
    let horiz_pref = ratio > 1.35 || (y_overlap && ratio > 0.9);
    let vert_pref = ratio < (1.0 / 1.35) || (x_overlap && ratio < 1.1);
    let use_horizontal = if horiz_pref && !vert_pref {
        true
    } else if vert_pref && !horiz_pref {
        false
    } else {
        is_horizontal(direction)
    };

    if use_horizontal {
        let is_backward = to.x + to.width < from.x;
        if dx >= 0.0 {
            (EdgeSide::Right, EdgeSide::Left, is_backward)
        } else {
            (EdgeSide::Left, EdgeSide::Right, is_backward)
        }
    } else {
        let is_backward = to.y + to.height < from.y;
        if dy >= 0.0 {
            (EdgeSide::Bottom, EdgeSide::Top, is_backward)
        } else {
            (EdgeSide::Top, EdgeSide::Bottom, is_backward)
        }
    }
}

fn edge_axis_is_horizontal(side: EdgeSide) -> bool {
    side_is_vertical(side)
}

fn side_slot(side: EdgeSide) -> usize {
    match side {
        EdgeSide::Left => 0,
        EdgeSide::Right => 1,
        EdgeSide::Top => 2,
        EdgeSide::Bottom => 3,
    }
}

fn side_load_for_node(
    side_loads: &HashMap<String, [usize; 4]>,
    node_id: &str,
    side: EdgeSide,
) -> usize {
    side_loads
        .get(node_id)
        .map(|slots| slots[side_slot(side)])
        .unwrap_or(0)
}

fn bump_side_load(side_loads: &mut HashMap<String, [usize; 4]>, node_id: &str, side: EdgeSide) {
    let slots = side_loads.entry(node_id.to_string()).or_insert([0; 4]);
    slots[side_slot(side)] += 1;
}

fn edge_sides_balanced(
    from_id: &str,
    to_id: &str,
    from: &NodeLayout,
    to: &NodeLayout,
    direction: Direction,
    node_degrees: &HashMap<String, usize>,
    side_loads: &HashMap<String, [usize; 4]>,
) -> (EdgeSide, EdgeSide, bool) {
    let primary = edge_sides(from, to, direction);
    let from_degree = node_degrees.get(from_id).copied().unwrap_or(0);
    let to_degree = node_degrees.get(to_id).copied().unwrap_or(0);
    if from_degree < 6 && to_degree < 6 {
        return primary;
    }

    let from_cx = from.x + from.width / 2.0;
    let from_cy = from.y + from.height / 2.0;
    let to_cx = to.x + to.width / 2.0;
    let to_cy = to.y + to.height / 2.0;
    let dx = to_cx - from_cx;
    let dy = to_cy - from_cy;

    // For hub-to-leaf edges, side balancing can over-disperse ports and
    // introduce fan crossing. Prefer the diagram's main direction axis.
    if (from_degree >= 10 && to_degree <= 4) || (to_degree >= 10 && from_degree <= 4) {
        if is_horizontal(direction) {
            let is_backward = to.x + to.width < from.x;
            if dx >= 0.0 {
                return (EdgeSide::Right, EdgeSide::Left, is_backward);
            }
            return (EdgeSide::Left, EdgeSide::Right, is_backward);
        }
        let is_backward = to.y + to.height < from.y;
        if dy >= 0.0 {
            return (EdgeSide::Bottom, EdgeSide::Top, is_backward);
        }
        return (EdgeSide::Top, EdgeSide::Bottom, is_backward);
    }

    let horizontal = if dx >= 0.0 {
        (EdgeSide::Right, EdgeSide::Left, to.x + to.width < from.x)
    } else {
        (EdgeSide::Left, EdgeSide::Right, to.x > from.x + from.width)
    };
    let vertical = if dy >= 0.0 {
        (EdgeSide::Bottom, EdgeSide::Top, to.y + to.height < from.y)
    } else {
        (EdgeSide::Top, EdgeSide::Bottom, to.y > from.y + from.height)
    };

    let mut options = vec![primary];
    if !options
        .iter()
        .any(|(start, end, _)| *start == horizontal.0 && *end == horizontal.1)
    {
        options.push(horizontal);
    }
    if !options
        .iter()
        .any(|(start, end, _)| *start == vertical.0 && *end == vertical.1)
    {
        options.push(vertical);
    }

    let primary_axis = edge_axis_is_horizontal(primary.0);
    let primary_from_anchor = anchor_point_for_node(from, primary.0, 0.0);
    let primary_to_anchor = anchor_point_for_node(to, primary.1, 0.0);
    let primary_manhattan = (primary_to_anchor.0 - primary_from_anchor.0).abs()
        + (primary_to_anchor.1 - primary_from_anchor.1).abs();
    let mut best = primary;
    let mut best_score = f32::MAX;
    let mut best_tiebreak = f32::MAX;
    for (start_side, end_side, is_backward) in options {
        let from_load = side_load_for_node(side_loads, from_id, start_side) as f32;
        let to_load = side_load_for_node(side_loads, to_id, end_side) as f32;
        let load_score = from_load * from_load + to_load * to_load + (from_load + to_load) * 0.5;
        let from_anchor = anchor_point_for_node(from, start_side, 0.0);
        let to_anchor = anchor_point_for_node(to, end_side, 0.0);
        let manhattan = (to_anchor.0 - from_anchor.0).abs() + (to_anchor.1 - from_anchor.1).abs();
        if !(start_side == primary.0 && end_side == primary.1)
            && manhattan > primary_manhattan * 1.35 + 120.0
        {
            continue;
        }
        let axis_penalty = if edge_axis_is_horizontal(start_side) == primary_axis {
            0.0
        } else {
            5.0
        };
        let primary_penalty = if start_side == primary.0 && end_side == primary.1 {
            0.0
        } else {
            2.0
        };
        let backward_penalty = if is_backward && !primary.2 { 4.0 } else { 0.0 };
        let score =
            load_score * 9.0 + manhattan * 0.22 + axis_penalty + primary_penalty + backward_penalty;
        let tiebreak = manhattan + from_load + to_load;
        if score < best_score || ((score - best_score).abs() < 1e-4 && tiebreak < best_tiebreak) {
            best = (start_side, end_side, is_backward);
            best_score = score;
            best_tiebreak = tiebreak;
        }
    }

    best
}

fn compute_c4_layout(graph: &Graph, config: &LayoutConfig) -> Layout {
    let c4 = &graph.c4;
    let fast_metrics = config.fast_text_metrics;
    let mut conf = config.c4.clone();
    if let Some(val) = c4.c4_shape_in_row_override {
        conf.c4_shape_in_row = val;
    }
    if let Some(val) = c4.c4_boundary_in_row_override {
        conf.c4_boundary_in_row = val;
    }
    let conf = &conf;
    let mut shapes_out = Vec::new();
    let mut boundaries_out = Vec::new();
    let mut rels_out = Vec::new();

    let mut shapes_by_boundary: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut shape_map: std::collections::HashMap<String, &crate::mermaid_renderer::ir::C4Shape> =
        std::collections::HashMap::new();
    for shape in &c4.shapes {
        shapes_by_boundary
            .entry(shape.parent_boundary.clone())
            .or_default()
            .push(shape.id.clone());
        shape_map.insert(shape.id.clone(), shape);
    }

    let mut boundaries_by_parent: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut boundary_map: std::collections::HashMap<
        String,
        &crate::mermaid_renderer::ir::C4Boundary,
    > = std::collections::HashMap::new();
    for boundary in &c4.boundaries {
        boundaries_by_parent
            .entry(boundary.parent_boundary.clone())
            .or_default()
            .push(boundary.id.clone());
        boundary_map.insert(boundary.id.clone(), boundary);
    }

    let root_boundaries = boundaries_by_parent.get("").cloned().unwrap_or_default();

    let mut global_max_x = conf.diagram_margin_x;
    let mut global_max_y = conf.diagram_margin_y;

    let mut screen_bounds = C4Bounds::new(conf);
    let width_limit = 1920.0;
    screen_bounds.set_data(
        conf.diagram_margin_x,
        conf.diagram_margin_x,
        conf.diagram_margin_y,
        conf.diagram_margin_y,
        width_limit,
    );

    layout_c4_boundaries(
        &mut screen_bounds,
        &root_boundaries,
        &mut shapes_out,
        &mut boundaries_out,
        &mut global_max_x,
        &mut global_max_y,
        &shapes_by_boundary,
        &shape_map,
        &boundaries_by_parent,
        &boundary_map,
        conf,
        fast_metrics,
    );

    for rel in &c4.rels {
        let Some(from_shape) = shapes_out.iter().find(|s| s.id == rel.from) else {
            continue;
        };
        let Some(to_shape) = shapes_out.iter().find(|s| s.id == rel.to) else {
            continue;
        };
        let (start, end) = c4_intersect_points(from_shape, to_shape);
        let label_font_size = conf.message_font_size;
        let rel_font_family = conf.message_font_family.as_str();
        let label_layout = c4_text_layout(
            &rel.label,
            label_font_size,
            0.0,
            conf.wrap,
            estimate_text_width(&rel.label, label_font_size, rel_font_family, fast_metrics),
            c4_text_line_height(conf, label_font_size),
            rel_font_family,
            fast_metrics,
        );
        let techn_layout = rel.techn.as_ref().map(|t| {
            c4_text_layout(
                t,
                label_font_size,
                0.0,
                conf.wrap,
                estimate_text_width(t, label_font_size, rel_font_family, fast_metrics),
                c4_text_line_height(conf, label_font_size),
                rel_font_family,
                fast_metrics,
            )
        });
        rels_out.push(C4RelLayout {
            kind: rel.kind,
            from: rel.from.clone(),
            to: rel.to.clone(),
            label: label_layout,
            techn: techn_layout,
            start,
            end,
            offset_x: rel.offset_x,
            offset_y: rel.offset_y,
            line_color: rel.line_color.clone(),
            text_color: rel.text_color.clone(),
        });
    }

    let width = (global_max_x - conf.diagram_margin_x + 2.0 * conf.diagram_margin_x).max(1.0);
    let height = (global_max_y - conf.diagram_margin_y + 2.0 * conf.diagram_margin_y).max(1.0);
    let viewbox_x = 0.0;
    let viewbox_y = -conf.diagram_margin_y;
    let viewbox_width = width;
    let viewbox_height = height;

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: Some(C4Layout {
            shapes: shapes_out,
            boundaries: boundaries_out,
            rels: rels_out,
            viewbox_x,
            viewbox_y,
            viewbox_width,
            viewbox_height,
            use_max_width: conf.use_max_width,
        }),
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

#[derive(Debug, Clone)]
struct C4BoundsData {
    startx: f32,
    stopx: f32,
    starty: f32,
    stopy: f32,
    width_limit: f32,
}

#[derive(Debug, Clone)]
struct C4BoundsNext {
    startx: f32,
    stopx: f32,
    starty: f32,
    stopy: f32,
    cnt: usize,
}

#[derive(Debug, Clone)]
struct C4Bounds {
    data: C4BoundsData,
    next: C4BoundsNext,
    conf: crate::mermaid_renderer::config::C4Config,
}

impl C4Bounds {
    fn new(conf: &crate::mermaid_renderer::config::C4Config) -> Self {
        Self {
            data: C4BoundsData {
                startx: 0.0,
                stopx: 0.0,
                starty: 0.0,
                stopy: 0.0,
                width_limit: 0.0,
            },
            next: C4BoundsNext {
                startx: 0.0,
                stopx: 0.0,
                starty: 0.0,
                stopy: 0.0,
                cnt: 0,
            },
            conf: conf.clone(),
        }
    }

    fn set_data(&mut self, startx: f32, stopx: f32, starty: f32, stopy: f32, width_limit: f32) {
        self.data.startx = startx;
        self.data.stopx = stopx;
        self.data.starty = starty;
        self.data.stopy = stopy;
        self.data.width_limit = width_limit;
        self.next.startx = startx;
        self.next.stopx = stopx;
        self.next.starty = starty;
        self.next.stopy = stopy;
        self.next.cnt = 0;
    }

    fn bump_last_margin(&mut self, margin: f32) {
        self.data.stopx += margin;
        self.data.stopy += margin;
    }

    fn insert(&mut self, width: f32, height: f32, margin: f32) -> (f32, f32) {
        self.next.cnt += 1;
        let mut startx = if (self.next.startx - self.next.stopx).abs() < f32::EPSILON {
            self.next.stopx + margin
        } else {
            self.next.stopx + margin * 2.0
        };
        let mut stopx = startx + width;
        let mut starty = self.next.starty + margin * 2.0;
        let mut stopy = starty + height;

        if startx >= self.data.width_limit
            || stopx >= self.data.width_limit
            || self.next.cnt > self.conf.c4_shape_in_row
        {
            startx = self.next.startx + margin + self.conf.next_line_padding_x;
            starty = self.next.stopy + margin * 2.0;
            stopx = startx + width;
            stopy = starty + height;
            self.next.starty = self.next.stopy;
            self.next.stopy = stopy;
            self.next.stopx = stopx;
            self.next.cnt = 1;
        }

        self.data.startx = if self.data.startx == 0.0 {
            startx
        } else {
            self.data.startx.min(startx)
        };
        self.data.starty = if self.data.starty == 0.0 {
            starty
        } else {
            self.data.starty.min(starty)
        };
        self.data.stopx = self.data.stopx.max(stopx);
        self.data.stopy = self.data.stopy.max(stopy);

        self.next.startx = self.next.startx.min(startx);
        self.next.starty = self.next.starty.min(starty);
        self.next.stopx = self.next.stopx.max(stopx);
        self.next.stopy = self.next.stopy.max(stopy);

        (startx, starty)
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_c4_boundaries(
    parent_bounds: &mut C4Bounds,
    boundary_ids: &[String],
    shapes_out: &mut Vec<C4ShapeLayout>,
    boundaries_out: &mut Vec<C4BoundaryLayout>,
    global_max_x: &mut f32,
    global_max_y: &mut f32,
    shapes_by_boundary: &std::collections::HashMap<String, Vec<String>>,
    shape_map: &std::collections::HashMap<String, &crate::mermaid_renderer::ir::C4Shape>,
    boundaries_by_parent: &std::collections::HashMap<String, Vec<String>>,
    boundary_map: &std::collections::HashMap<String, &crate::mermaid_renderer::ir::C4Boundary>,
    conf: &crate::mermaid_renderer::config::C4Config,
    fast_metrics: bool,
) {
    if boundary_ids.is_empty() {
        return;
    }
    let mut current_bounds = C4Bounds::new(conf);
    let limit_div = conf.c4_boundary_in_row.max(1).min(boundary_ids.len());
    current_bounds.data.width_limit = parent_bounds.data.width_limit / limit_div as f32;

    for (idx, boundary_id) in boundary_ids.iter().enumerate() {
        let Some(boundary) = boundary_map.get(boundary_id) else {
            continue;
        };
        let mut y = 0.0;
        let boundary_text_wrap = conf.wrap;
        let label_font_size = conf.boundary_font_size + 2.0;
        let boundary_font_family = conf.boundary_font_family.as_str();
        let label_layout = c4_text_layout(
            &boundary.label,
            label_font_size,
            y + 8.0,
            boundary_text_wrap,
            current_bounds.data.width_limit,
            c4_text_line_height(conf, label_font_size),
            boundary_font_family,
            fast_metrics,
        );
        y = label_layout.y + label_layout.height;
        let mut boundary_type_layout = None;
        if !boundary.boundary_type.is_empty() {
            let type_text = format!("[{}]", boundary.boundary_type);
            let type_layout = c4_text_layout(
                &type_text,
                conf.boundary_font_size,
                y + 5.0,
                boundary_text_wrap,
                current_bounds.data.width_limit,
                c4_text_line_height(conf, conf.boundary_font_size),
                boundary_font_family,
                fast_metrics,
            );
            y = type_layout.y + type_layout.height;
            boundary_type_layout = Some(type_layout);
        }
        let mut boundary_descr_layout = None;
        if let Some(descr) = &boundary.descr {
            let descr_layout = c4_text_layout(
                descr,
                (conf.boundary_font_size - 2.0).max(1.0),
                y + 20.0,
                boundary_text_wrap,
                current_bounds.data.width_limit,
                c4_text_line_height(conf, (conf.boundary_font_size - 2.0).max(1.0)),
                boundary_font_family,
                fast_metrics,
            );
            y = descr_layout.y + descr_layout.height;
            boundary_descr_layout = Some(descr_layout);
        }

        if idx == 0 || idx % conf.c4_boundary_in_row == 0 {
            let startx = parent_bounds.data.startx + conf.diagram_margin_x;
            let starty = parent_bounds.data.stopy + conf.diagram_margin_y + y;
            current_bounds.set_data(
                startx,
                startx,
                starty,
                starty,
                current_bounds.data.width_limit,
            );
        } else {
            let startx =
                if (current_bounds.data.stopx - current_bounds.data.startx).abs() > f32::EPSILON {
                    current_bounds.data.stopx + conf.diagram_margin_x
                } else {
                    current_bounds.data.startx
                };
            let starty = current_bounds.data.starty;
            current_bounds.set_data(
                startx,
                startx,
                starty,
                starty,
                current_bounds.data.width_limit,
            );
        }

        if let Some(shape_ids) = shapes_by_boundary.get(boundary_id) {
            layout_c4_shapes(
                &mut current_bounds,
                shape_ids,
                shapes_out,
                shape_map,
                conf,
                fast_metrics,
            );
        }

        if let Some(child_boundaries) = boundaries_by_parent.get(boundary_id) {
            layout_c4_boundaries(
                &mut current_bounds,
                child_boundaries,
                shapes_out,
                boundaries_out,
                global_max_x,
                global_max_y,
                shapes_by_boundary,
                shape_map,
                boundaries_by_parent,
                boundary_map,
                conf,
                fast_metrics,
            );
        }

        if boundary_id != "global" {
            let boundary_layout = C4BoundaryLayout {
                id: boundary_id.clone(),
                label: label_layout,
                boundary_type: boundary_type_layout,
                descr: boundary_descr_layout,
                bg_color: boundary.bg_color.clone(),
                border_color: boundary.border_color.clone(),
                font_color: boundary.font_color.clone(),
                x: current_bounds.data.startx,
                y: current_bounds.data.starty,
                width: current_bounds.data.stopx - current_bounds.data.startx,
                height: current_bounds.data.stopy - current_bounds.data.starty,
            };
            boundaries_out.push(boundary_layout);
        }

        parent_bounds.data.stopy = parent_bounds
            .data
            .stopy
            .max(current_bounds.data.stopy + conf.c4_shape_margin);
        parent_bounds.data.stopx = parent_bounds
            .data
            .stopx
            .max(current_bounds.data.stopx + conf.c4_shape_margin);
        *global_max_x = (*global_max_x).max(parent_bounds.data.stopx);
        *global_max_y = (*global_max_y).max(parent_bounds.data.stopy);
    }
}

fn layout_c4_shapes(
    bounds: &mut C4Bounds,
    shape_ids: &[String],
    shapes_out: &mut Vec<C4ShapeLayout>,
    shape_map: &std::collections::HashMap<String, &crate::mermaid_renderer::ir::C4Shape>,
    conf: &crate::mermaid_renderer::config::C4Config,
    fast_metrics: bool,
) {
    for shape_id in shape_ids {
        let Some(shape) = shape_map.get(shape_id) else {
            continue;
        };
        let type_font_size = (c4_shape_font_size(conf, shape.kind) - 2.0).max(1.0);
        let type_font_family = c4_shape_font_family(conf, shape.kind);
        let type_label_text = format!("<<{}>>", shape.kind.as_str());
        let type_width = estimate_text_width(
            &type_label_text,
            type_font_size,
            type_font_family,
            fast_metrics,
        );
        let type_height = type_font_size + 2.0;
        let type_layout = C4TextLayout {
            text: type_label_text.clone(),
            lines: vec![type_label_text],
            width: type_width,
            height: type_height,
            y: conf.c4_shape_padding,
        };
        let mut y = type_layout.y + type_layout.height - 4.0;

        let mut image_y = None;
        if matches!(
            shape.kind,
            crate::mermaid_renderer::ir::C4ShapeKind::Person
                | crate::mermaid_renderer::ir::C4ShapeKind::ExternalPerson
        ) {
            image_y = Some(y);
            y += conf.person_icon_size;
        } else if shape.sprite.is_some() {
            image_y = Some(y);
            y += conf.person_icon_size;
        }

        let label_font_size = c4_shape_font_size(conf, shape.kind) + 2.0;
        let label_font_family = c4_shape_font_family(conf, shape.kind);
        let text_limit_width = conf.width - conf.c4_shape_padding * 2.0;
        let label_layout = c4_text_layout(
            &shape.label,
            label_font_size,
            y + 8.0,
            conf.wrap,
            text_limit_width,
            c4_text_line_height(conf, label_font_size),
            label_font_family,
            fast_metrics,
        );
        y = label_layout.y + label_layout.height;

        let mut type_or_techn_layout = None;
        let type_or_techn_text = shape
            .techn
            .as_ref()
            .or(shape.type_label.as_ref())
            .map(|t| format!("[{}]", t));
        if let Some(text) = type_or_techn_text {
            let font_size = c4_shape_font_size(conf, shape.kind);
            let font_family = c4_shape_font_family(conf, shape.kind);
            let layout = c4_text_layout(
                &text,
                font_size,
                y + 5.0,
                conf.wrap,
                text_limit_width,
                c4_text_line_height(conf, font_size),
                font_family,
                fast_metrics,
            );
            y = layout.y + layout.height;
            type_or_techn_layout = Some(layout);
        }

        let mut descr_layout = None;
        let mut rect_height = y;
        let mut rect_width = label_layout.width;
        if let Some(descr) = &shape.descr {
            let font_size = c4_shape_font_size(conf, shape.kind);
            let font_family = c4_shape_font_family(conf, shape.kind);
            let layout = c4_text_layout(
                descr,
                font_size,
                y + 20.0,
                conf.wrap,
                text_limit_width,
                c4_text_line_height(conf, font_size),
                font_family,
                fast_metrics,
            );
            y = layout.y + layout.height;
            rect_width = rect_width.max(layout.width);
            let lines = layout.lines.len() as f32;
            rect_height = y - lines * 5.0;
            descr_layout = Some(layout);
        }
        rect_width += conf.c4_shape_padding;
        let width = conf.width.max(rect_width);
        let height = conf.height.max(rect_height);
        let margin = conf.c4_shape_margin;
        let (x, y_pos) = bounds.insert(width, height, margin);

        shapes_out.push(C4ShapeLayout {
            id: shape.id.clone(),
            kind: shape.kind,
            bg_color: shape.bg_color.clone(),
            border_color: shape.border_color.clone(),
            font_color: shape.font_color.clone(),
            x,
            y: y_pos,
            width,
            height,
            margin,
            type_label: type_layout,
            label: label_layout,
            type_or_techn: type_or_techn_layout,
            descr: descr_layout,
            image_y,
        });
    }
    bounds.bump_last_margin(conf.c4_shape_margin);
}

fn c4_shape_font_size(
    conf: &crate::mermaid_renderer::config::C4Config,
    kind: crate::mermaid_renderer::ir::C4ShapeKind,
) -> f32 {
    match kind {
        crate::mermaid_renderer::ir::C4ShapeKind::Person => conf.person_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalPerson => conf.external_person_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::System => conf.system_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::SystemDb => conf.system_db_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::SystemQueue => conf.system_queue_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystem => conf.external_system_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystemDb => {
            conf.external_system_db_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystemQueue => {
            conf.external_system_queue_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::Container => conf.container_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ContainerDb => conf.container_db_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ContainerQueue => conf.container_queue_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainer => {
            conf.external_container_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainerDb => {
            conf.external_container_db_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainerQueue => {
            conf.external_container_queue_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::Component => conf.component_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ComponentDb => conf.component_db_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ComponentQueue => conf.component_queue_font_size,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponent => {
            conf.external_component_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponentDb => {
            conf.external_component_db_font_size
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponentQueue => {
            conf.external_component_queue_font_size
        }
    }
}

fn c4_shape_font_family(
    conf: &crate::mermaid_renderer::config::C4Config,
    kind: crate::mermaid_renderer::ir::C4ShapeKind,
) -> &str {
    match kind {
        crate::mermaid_renderer::ir::C4ShapeKind::Person => &conf.person_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalPerson => {
            &conf.external_person_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::System => &conf.system_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::SystemDb => &conf.system_db_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::SystemQueue => &conf.system_queue_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystem => {
            &conf.external_system_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystemDb => {
            &conf.external_system_db_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalSystemQueue => {
            &conf.external_system_queue_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::Container => &conf.container_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ContainerDb => &conf.container_db_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ContainerQueue => {
            &conf.container_queue_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainer => {
            &conf.external_container_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainerDb => {
            &conf.external_container_db_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalContainerQueue => {
            &conf.external_container_queue_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::Component => &conf.component_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ComponentDb => &conf.component_db_font_family,
        crate::mermaid_renderer::ir::C4ShapeKind::ComponentQueue => {
            &conf.component_queue_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponent => {
            &conf.external_component_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponentDb => {
            &conf.external_component_db_font_family
        }
        crate::mermaid_renderer::ir::C4ShapeKind::ExternalComponentQueue => {
            &conf.external_component_queue_font_family
        }
    }
}

fn c4_text_line_height(conf: &crate::mermaid_renderer::config::C4Config, font_size: f32) -> f32 {
    let mut height = font_size + conf.text_line_height;
    if font_size <= conf.text_line_height_small_threshold {
        height += conf.text_line_height_small_add;
    }
    height.max(1.0)
}

fn c4_text_layout(
    text: &str,
    font_size: f32,
    y: f32,
    wrap: bool,
    max_width: f32,
    line_height: f32,
    font_family: &str,
    fast_metrics: bool,
) -> C4TextLayout {
    let mut lines = Vec::new();
    for raw in split_lines(text) {
        if wrap {
            lines.extend(wrap_text_to_width(
                &raw,
                max_width,
                font_size,
                font_family,
                fast_metrics,
            ));
        } else {
            lines.push(raw);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let width = lines
        .iter()
        .map(|line| estimate_text_width(line, font_size, font_family, fast_metrics))
        .fold(0.0, f32::max);
    let height = line_height * lines.len().max(1) as f32;
    C4TextLayout {
        text: text.to_string(),
        lines,
        width,
        height,
        y,
    }
}

fn wrap_text_to_width(
    text: &str,
    max_width: f32,
    font_size: f32,
    font_family: &str,
    fast_metrics: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if estimate_text_width(&candidate, font_size, font_family, fast_metrics) <= max_width
            || current.is_empty()
        {
            current = candidate;
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

fn estimate_text_width(text: &str, font_size: f32, font_family: &str, fast_metrics: bool) -> f32 {
    if fast_metrics && text.is_ascii() {
        return text.chars().map(c4_char_width_factor).sum::<f32>() * font_size;
    }
    text_metrics::measure_text_width(text, font_size, font_family)
        .unwrap_or_else(|| text.chars().map(c4_char_width_factor).sum::<f32>() * font_size)
}

fn c4_char_width_factor(ch: char) -> f32 {
    match ch {
        '<' | '>' => 0.247,
        '_' => 0.455,
        _ => char_width_factor(ch),
    }
}

fn c4_intersect_points(
    from_node: &C4ShapeLayout,
    to_node: &C4ShapeLayout,
) -> ((f32, f32), (f32, f32)) {
    let end_center = (
        to_node.x + to_node.width / 2.0,
        to_node.y + to_node.height / 2.0,
    );
    let start = c4_intersect_point(from_node, end_center);
    let start_center = (
        from_node.x + from_node.width / 2.0,
        from_node.y + from_node.height / 2.0,
    );
    let end = c4_intersect_point(to_node, start_center);
    (start, end)
}

fn c4_intersect_point(node: &C4ShapeLayout, end: (f32, f32)) -> (f32, f32) {
    let (x1, y1) = (node.x, node.y);
    let (x2, y2) = end;
    let from_center_x = x1 + node.width / 2.0;
    let from_center_y = y1 + node.height / 2.0;
    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let tan_dyx = if dx.abs() < f32::EPSILON {
        0.0
    } else {
        dy / dx
    };
    let from_dyx = node.height / node.width;
    if (y1 - y2).abs() < f32::EPSILON && x1 < x2 {
        return (x1 + node.width, from_center_y);
    }
    if (y1 - y2).abs() < f32::EPSILON && x1 > x2 {
        return (x1, from_center_y);
    }
    if (x1 - x2).abs() < f32::EPSILON && y1 < y2 {
        return (from_center_x, y1 + node.height);
    }
    if (x1 - x2).abs() < f32::EPSILON && y1 > y2 {
        return (from_center_x, y1);
    }
    if x1 > x2 && y1 < y2 {
        if from_dyx >= tan_dyx {
            (x1, from_center_y + tan_dyx * node.width / 2.0)
        } else {
            (
                from_center_x - dx / dy * node.height / 2.0,
                y1 + node.height,
            )
        }
    } else if x1 < x2 && y1 < y2 {
        if from_dyx >= tan_dyx {
            (x1 + node.width, from_center_y + tan_dyx * node.width / 2.0)
        } else {
            (
                from_center_x + dx / dy * node.height / 2.0,
                y1 + node.height,
            )
        }
    } else if x1 < x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            (x1 + node.width, from_center_y - tan_dyx * node.width / 2.0)
        } else {
            (from_center_x + node.height / 2.0 * dx / dy, y1)
        }
    } else if x1 > x2 && y1 > y2 {
        if from_dyx >= tan_dyx {
            (x1, from_center_y - node.width / 2.0 * tan_dyx)
        } else {
            (from_center_x - node.height / 2.0 * dx / dy, y1)
        }
    } else {
        (from_center_x, from_center_y)
    }
}

fn is_region_subgraph(sub: &crate::mermaid_renderer::ir::Subgraph) -> bool {
    sub.label.trim().is_empty()
        && sub
            .id
            .as_deref()
            .map(|id| id.starts_with("__region_"))
            .unwrap_or(false)
}

pub fn compute_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    match graph.kind {
        crate::mermaid_renderer::ir::DiagramKind::Sequence
        | crate::mermaid_renderer::ir::DiagramKind::ZenUML => {
            compute_sequence_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Pie => {
            if config.pie.render_mode == PieRenderMode::Error {
                compute_pie_error_layout(graph, config)
            } else {
                compute_pie_layout(graph, theme, config)
            }
        }
        crate::mermaid_renderer::ir::DiagramKind::Quadrant => {
            compute_quadrant_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Gantt => {
            compute_gantt_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Kanban => {
            compute_kanban_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Block => {
            compute_block_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Sankey => {
            compute_sankey_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Architecture => {
            compute_architecture_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Radar => {
            compute_radar_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Treemap => {
            if config.treemap.render_mode == TreemapRenderMode::Error {
                compute_error_layout(graph, config)
            } else {
                compute_treemap_layout(graph, theme, config)
            }
        }
        crate::mermaid_renderer::ir::DiagramKind::GitGraph => {
            compute_gitgraph_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::C4 => compute_c4_layout(graph, config),
        crate::mermaid_renderer::ir::DiagramKind::Mindmap => {
            compute_mindmap_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::XYChart => {
            compute_xychart_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Timeline => {
            compute_timeline_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Journey => {
            compute_journey_layout(graph, theme, config)
        }
        crate::mermaid_renderer::ir::DiagramKind::Class
        | crate::mermaid_renderer::ir::DiagramKind::State
        | crate::mermaid_renderer::ir::DiagramKind::Er
        | crate::mermaid_renderer::ir::DiagramKind::Requirement
        | crate::mermaid_renderer::ir::DiagramKind::Packet
        | crate::mermaid_renderer::ir::DiagramKind::Flowchart => {
            compute_flowchart_layout(graph, theme, config)
        }
    }
}

fn compute_error_layout(graph: &Graph, config: &LayoutConfig) -> Layout {
    let viewbox_width = config.treemap.error_viewbox_width.max(1.0);
    let viewbox_height = config.treemap.error_viewbox_height.max(1.0);
    let render_width = config.treemap.error_render_width.max(1.0);
    let derived_height = render_width * viewbox_height / viewbox_width;
    let render_height = match config.treemap.error_render_height {
        Some(height) => height,
        None => derived_height.round(),
    }
    .max(1.0);
    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: Some(ErrorLayout {
            viewbox_width,
            viewbox_height,
            render_width,
            render_height,
            message: config.treemap.error_message.clone(),
            version: config.treemap.error_version.clone(),
            text_x: config.treemap.error_text_x,
            text_y: config.treemap.error_text_y,
            text_size: config.treemap.error_text_size,
            version_x: config.treemap.error_version_x,
            version_y: config.treemap.error_version_y,
            version_size: config.treemap.error_version_size,
            icon_scale: config.treemap.icon_scale,
            icon_tx: config.treemap.icon_tx,
            icon_ty: config.treemap.icon_ty,
        }),
        width: render_width,
        height: render_height,
    }
}

fn compute_pie_error_layout(graph: &Graph, config: &LayoutConfig) -> Layout {
    let viewbox_width = config.pie.error_viewbox_width.max(1.0);
    let viewbox_height = config.pie.error_viewbox_height.max(1.0);
    let render_width = config.pie.error_render_width.max(1.0);
    let derived_height = render_width * viewbox_height / viewbox_width;
    let render_height = match config.pie.error_render_height {
        Some(height) => height,
        None => derived_height.round(),
    }
    .max(1.0);
    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: Some(ErrorLayout {
            viewbox_width,
            viewbox_height,
            render_width,
            render_height,
            message: config.pie.error_message.clone(),
            version: config.pie.error_version.clone(),
            text_x: config.pie.error_text_x,
            text_y: config.pie.error_text_y,
            text_size: config.pie.error_text_size,
            version_x: config.pie.error_version_x,
            version_y: config.pie.error_version_y,
            version_size: config.pie.error_version_size,
            icon_scale: config.pie.icon_scale,
            icon_tx: config.pie.icon_tx,
            icon_ty: config.pie.icon_ty,
        }),
        width: render_width,
        height: render_height,
    }
}

#[derive(Clone)]
struct MindmapPalette {
    section_fills: Vec<String>,
    section_labels: Vec<String>,
    section_lines: Vec<String>,
    root_fill: String,
    root_text: String,
}

#[derive(Clone)]
struct MindmapNodeInfo {
    level: usize,
    section: Option<usize>,
    children: Vec<String>,
}

fn mindmap_palette(theme: &Theme, config: &LayoutConfig) -> MindmapPalette {
    let mindmap = &config.mindmap;
    let section_fills = if mindmap.section_colors.is_empty() {
        vec!["#ECECFF".to_string()]
    } else {
        mindmap.section_colors.clone()
    };
    let section_labels = if mindmap.section_label_colors.is_empty() {
        vec![theme.primary_text_color.clone()]
    } else {
        mindmap.section_label_colors.clone()
    };
    let section_lines = if mindmap.section_line_colors.is_empty() {
        vec![theme.primary_border_color.clone()]
    } else {
        mindmap.section_line_colors.clone()
    };
    let root_fill = mindmap
        .root_fill
        .clone()
        .unwrap_or_else(|| theme.git_colors[0].clone());
    let root_text = mindmap
        .root_text
        .clone()
        .unwrap_or_else(|| theme.git_branch_label_colors[0].clone());
    MindmapPalette {
        section_fills,
        section_labels,
        section_lines,
        root_fill,
        root_text,
    }
}

fn pick_palette_color(values: &[String], idx: usize) -> String {
    if values.is_empty() {
        return String::new();
    }
    let index = idx % values.len();
    values[index].clone()
}

fn mindmap_node_size(
    shape: crate::mermaid_renderer::ir::NodeShape,
    label: &TextBlock,
    config: &LayoutConfig,
) -> (f32, f32) {
    let mindmap = &config.mindmap;
    match shape {
        crate::mermaid_renderer::ir::NodeShape::MindmapDefault => (
            label.width + mindmap.padding * 4.0,
            label.height + mindmap.padding,
        ),
        crate::mermaid_renderer::ir::NodeShape::Rectangle => {
            let pad = mindmap.rect_padding;
            (label.width + pad * 2.0, label.height + pad * 2.0)
        }
        crate::mermaid_renderer::ir::NodeShape::RoundRect => {
            let pad = mindmap.rounded_padding;
            (label.width + pad * 2.0, label.height + pad * 2.0)
        }
        crate::mermaid_renderer::ir::NodeShape::Circle
        | crate::mermaid_renderer::ir::NodeShape::DoubleCircle => {
            let pad = mindmap.circle_padding;
            let size = label.width.max(label.height) + pad * 2.0;
            (size, size)
        }
        crate::mermaid_renderer::ir::NodeShape::Hexagon => {
            let pad_x = mindmap.rect_padding * mindmap.hexagon_padding_multiplier;
            let pad_y = mindmap.rect_padding;
            (label.width + pad_x * 2.0, label.height + pad_y * 2.0)
        }
        _ => {
            let pad = mindmap.rect_padding;
            (label.width + pad * 2.0, label.height + pad * 2.0)
        }
    }
}

fn mindmap_subtree_height(
    node_id: &str,
    info: &HashMap<String, MindmapNodeInfo>,
    nodes: &BTreeMap<String, NodeLayout>,
    memo: &mut HashMap<String, f32>,
    spacing: f32,
) -> f32 {
    if let Some(value) = memo.get(node_id) {
        return *value;
    }
    let Some(node) = nodes.get(node_id) else {
        return 0.0;
    };
    let mut height = node.height;
    if let Some(node_info) = info.get(node_id)
        && !node_info.children.is_empty()
    {
        let mut total = 0.0;
        for child in &node_info.children {
            total += mindmap_subtree_height(child, info, nodes, memo, spacing);
        }
        if node_info.children.len() > 1 {
            total += spacing * (node_info.children.len() as f32 - 1.0);
        }
        height = height.max(total);
    }
    memo.insert(node_id.to_string(), height);
    height
}

fn assign_mindmap_positions(
    node_id: &str,
    direction: f32,
    center_x: f32,
    center_y: f32,
    info: &HashMap<String, MindmapNodeInfo>,
    nodes: &mut BTreeMap<String, NodeLayout>,
    subtree_heights: &HashMap<String, f32>,
    horizontal_gap: f32,
    vertical_gap: f32,
) {
    let parent_width = if let Some(node) = nodes.get_mut(node_id) {
        node.x = center_x - node.width / 2.0;
        node.y = center_y - node.height / 2.0;
        node.width
    } else {
        return;
    };
    let Some(node_info) = info.get(node_id) else {
        return;
    };
    if node_info.children.is_empty() {
        return;
    }
    let mut total = 0.0;
    for child in &node_info.children {
        total += subtree_heights.get(child).copied().unwrap_or(0.0);
    }
    if node_info.children.len() > 1 {
        total += vertical_gap * (node_info.children.len() as f32 - 1.0);
    }
    let mut cursor = center_y - total / 2.0;
    for child_id in &node_info.children {
        let child_height = subtree_heights.get(child_id).copied().unwrap_or(0.0);
        let child_width = nodes.get(child_id).map(|node| node.width).unwrap_or(0.0);
        let child_center_y = cursor + child_height / 2.0;
        let child_center_x =
            center_x + direction * (parent_width / 2.0 + child_width / 2.0 + horizontal_gap);
        assign_mindmap_positions(
            child_id,
            direction,
            child_center_x,
            child_center_y,
            info,
            nodes,
            subtree_heights,
            horizontal_gap,
            vertical_gap,
        );
        cursor += child_height + vertical_gap;
    }
}

fn place_mindmap_children(
    children: &[String],
    direction: f32,
    parent_center: (f32, f32),
    parent_width: f32,
    info: &HashMap<String, MindmapNodeInfo>,
    nodes: &mut BTreeMap<String, NodeLayout>,
    subtree_heights: &HashMap<String, f32>,
    horizontal_gap: f32,
    vertical_gap: f32,
) {
    if children.is_empty() {
        return;
    }
    let mut total = 0.0;
    for child in children {
        total += subtree_heights.get(child).copied().unwrap_or(0.0);
    }
    if children.len() > 1 {
        total += vertical_gap * (children.len() as f32 - 1.0);
    }
    let mut cursor = parent_center.1 - total / 2.0;
    for child_id in children {
        let child_height = subtree_heights.get(child_id).copied().unwrap_or(0.0);
        let child_width = nodes.get(child_id).map(|node| node.width).unwrap_or(0.0);
        let child_center_y = cursor + child_height / 2.0;
        let child_center_x =
            parent_center.0 + direction * (parent_width / 2.0 + child_width / 2.0 + horizontal_gap);
        assign_mindmap_positions(
            child_id,
            direction,
            child_center_x,
            child_center_y,
            info,
            nodes,
            subtree_heights,
            horizontal_gap,
            vertical_gap,
        );
        cursor += child_height + vertical_gap;
    }
}

fn compute_mindmap_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let palette = mindmap_palette(theme, config);
    let mut nodes: BTreeMap<String, NodeLayout> = BTreeMap::new();
    let mut info_map: HashMap<String, MindmapNodeInfo> = HashMap::new();

    for node in &graph.mindmap.nodes {
        let label_text = graph
            .nodes
            .get(&node.id)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| node.label.clone());
        let mut label = measure_label(&label_text, theme, config);
        label.width *= config.mindmap.text_width_scale;
        if config.mindmap.use_max_width {
            label.width = label.width.min(config.mindmap.max_node_width);
        }
        let shape = graph
            .nodes
            .get(&node.id)
            .map(|n| n.shape)
            .unwrap_or(crate::mermaid_renderer::ir::NodeShape::MindmapDefault);
        let (width, height) = mindmap_node_size(shape, &label, config);
        let mut style = resolve_node_style(node.id.as_str(), graph);
        let is_root = node.level == 0;
        if is_root {
            if style.fill.is_none() {
                style.fill = Some(palette.root_fill.clone());
            }
            if style.text_color.is_none() {
                style.text_color = Some(palette.root_text.clone());
            }
        } else if let Some(section) = node.section {
            let index = section + 1;
            if style.fill.is_none() {
                style.fill = Some(pick_palette_color(&palette.section_fills, index));
            }
            if style.text_color.is_none() {
                style.text_color = Some(pick_palette_color(&palette.section_labels, index));
            }
            if style.line_color.is_none() {
                style.line_color = Some(pick_palette_color(&palette.section_lines, index));
            }
        }
        if style.stroke.is_none() {
            style.stroke = Some("none".to_string());
        }
        if style.stroke_width.is_none() {
            style.stroke_width = Some(0.0);
        }

        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width,
                height,
                label,
                shape,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );

        info_map.insert(
            node.id.clone(),
            MindmapNodeInfo {
                level: node.level,
                section: node.section,
                children: node.children.clone(),
            },
        );
    }

    let root_id = graph
        .mindmap
        .root_id
        .clone()
        .or_else(|| graph.mindmap.nodes.first().map(|node| node.id.clone()));
    let mut subtree_heights: HashMap<String, f32> = HashMap::new();

    let mut horizontal_gap = config.mindmap.rank_spacing * config.mindmap.rank_spacing_multiplier;
    let mut vertical_gap = config.mindmap.node_spacing * config.mindmap.node_spacing_multiplier;
    let node_count = graph.mindmap.nodes.len();
    let density_scale = if node_count >= 10 {
        0.7
    } else if node_count >= 6 {
        0.8
    } else {
        1.0
    };
    horizontal_gap = (horizontal_gap * density_scale).max(theme.font_size * 1.1);
    vertical_gap = (vertical_gap * density_scale).max(theme.font_size * 0.9);

    if let Some(root_id) = root_id.as_ref() {
        mindmap_subtree_height(
            root_id,
            &info_map,
            &nodes,
            &mut subtree_heights,
            vertical_gap,
        );
        let root_center = (0.0_f32, 0.0_f32);
        if let Some(root_node) = nodes.get_mut(root_id) {
            root_node.x = root_center.0 - root_node.width / 2.0;
            root_node.y = root_center.1 - root_node.height / 2.0;
        }
        let mut left_children: Vec<String> = Vec::new();
        let mut right_children: Vec<String> = Vec::new();
        if let Some(info) = info_map.get(root_id) {
            for child_id in &info.children {
                let section = info_map
                    .get(child_id)
                    .and_then(|child| child.section)
                    .unwrap_or(0);
                if section.is_multiple_of(2) {
                    right_children.push(child_id.clone());
                } else {
                    left_children.push(child_id.clone());
                }
            }
        }
        let root_width = nodes.get(root_id).map(|n| n.width).unwrap_or(0.0);

        place_mindmap_children(
            &right_children,
            1.0,
            root_center,
            root_width,
            &info_map,
            &mut nodes,
            &subtree_heights,
            horizontal_gap,
            vertical_gap,
        );
        place_mindmap_children(
            &left_children,
            -1.0,
            root_center,
            root_width,
            &info_map,
            &mut nodes,
            &subtree_heights,
            horizontal_gap,
            vertical_gap,
        );
    }

    let mut edges = Vec::new();
    for edge in &graph.edges {
        let Some(from_layout) = nodes.get(&edge.from) else {
            continue;
        };
        let Some(to_layout) = nodes.get(&edge.to) else {
            continue;
        };
        let from_center = (
            from_layout.x + from_layout.width / 2.0,
            from_layout.y + from_layout.height / 2.0,
        );
        let to_center = (
            to_layout.x + to_layout.width / 2.0,
            to_layout.y + to_layout.height / 2.0,
        );
        let mut override_style = crate::mermaid_renderer::ir::EdgeStyleOverride::default();
        if let Some(child_info) = info_map.get(&edge.to)
            && let Some(section) = child_info.section
        {
            let index = section + 1;
            override_style.stroke = Some(pick_palette_color(&palette.section_fills, index));
        }
        let parent_level = info_map.get(&edge.from).map(|info| info.level).unwrap_or(0);
        let edge_depth = parent_level + 1;
        override_style.stroke_width = Some(
            config.mindmap.edge_depth_base_width
                + config.mindmap.edge_depth_step * (edge_depth as f32 + 1.0),
        );
        edges.push(EdgeLayout {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label: None,
            start_label: None,
            end_label: None,
            points: vec![from_center, to_center],
            directed: false,
            arrow_start: false,
            arrow_end: false,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
            override_style,
        });
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node in nodes.values() {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    let width = if min_x == f32::MAX {
        1.0
    } else {
        (max_x - min_x).max(1.0)
    };
    let height = if min_y == f32::MAX {
        1.0
    } else {
        (max_y - min_y).max(1.0)
    };

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_gitgraph_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let gg = &config.gitgraph;
    let is_vertical = matches!(graph.direction, Direction::TopDown | Direction::BottomTop);
    let is_bottom_top = graph.direction == Direction::BottomTop;
    let mut branches = graph.gitgraph.branches.clone();
    if branches.is_empty() {
        branches.push(crate::mermaid_renderer::ir::GitGraphBranch {
            name: gg.main_branch_name.clone(),
            order: Some(gg.main_branch_order),
            insertion_index: 0,
        });
    }

    let mut branch_entries: Vec<(crate::mermaid_renderer::ir::GitGraphBranch, f32)> = branches
        .into_iter()
        .map(|branch| {
            let order = branch
                .order
                .unwrap_or_else(|| default_branch_order(branch.insertion_index));
            (branch, order)
        })
        .collect();
    branch_entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

    let mut branch_pos: HashMap<String, (f32, usize, f32, f32)> = HashMap::new();
    let mut branch_layouts = Vec::new();
    let mut pos = 0.0f32;
    for (index, (branch, _order)) in branch_entries.iter().enumerate() {
        let measure_font_size = if gg.branch_label_font_size > 0.0 {
            gg.branch_label_font_size
        } else {
            theme.font_size
        };
        let (label_width, label_height) = measure_gitgraph_text(
            &branch.name,
            measure_font_size,
            gg.branch_label_line_height,
            gg.text_width_scale,
            theme.font_family.as_str(),
            config.fast_text_metrics,
        );
        let spacing_rotate_extra = if gg.rotate_commit_label {
            gg.branch_spacing_rotate_extra
        } else {
            0.0
        };
        let label_rotate_extra = if gg.rotate_commit_label {
            gg.branch_label_rotate_extra
        } else {
            0.0
        };
        let bg_width = label_width + gg.branch_label_bg_pad_x;
        let bg_height = label_height + gg.branch_label_bg_pad_y;
        let (bg_final_x, bg_final_y, text_x, text_y) = if is_vertical {
            let bg_x = pos - label_width / 2.0 - gg.branch_label_tb_bg_offset_x;
            let text_x = pos - label_width / 2.0 - gg.branch_label_tb_text_offset_x;
            let base_y = if is_bottom_top {
                0.0
            } else {
                gg.branch_label_tb_offset_y
            };
            (bg_x, base_y, text_x, base_y)
        } else {
            let bg_x = -label_width - gg.branch_label_bg_offset_x - label_rotate_extra;
            let bg_y = -label_height / 2.0 + gg.branch_label_bg_offset_y;
            let bg_final_x = bg_x + gg.branch_label_translate_x;
            let bg_final_y = bg_y + (pos - label_height / 2.0);
            let text_x = -label_width - gg.branch_label_text_offset_x - label_rotate_extra;
            let text_y = pos - label_height / 2.0 + gg.branch_label_text_offset_y;
            (bg_final_x, bg_final_y, text_x, text_y)
        };
        let label = GitGraphBranchLabelLayout {
            bg_x: bg_final_x,
            bg_y: bg_final_y,
            bg_width,
            bg_height,
            text_x,
            text_y,
            text_width: label_width,
            text_height: label_height,
        };
        branch_layouts.push(GitGraphBranchLayout {
            name: branch.name.clone(),
            index,
            pos,
            label,
        });
        branch_pos.insert(branch.name.clone(), (pos, index, label_width, label_height));
        let width_extra = if is_vertical { label_width / 2.0 } else { 0.0 };
        pos += gg.branch_spacing + spacing_rotate_extra + width_extra;
    }

    let mut commits = graph.gitgraph.commits.clone();
    commits.sort_by_key(|commit| commit.seq);
    let mut commit_layouts = Vec::new();
    let mut commit_pos: HashMap<String, (f32, f32)> = HashMap::new();
    let mut pos = if is_vertical { gg.default_pos } else { 0.0 };
    let mut max_pos = pos;
    let is_parallel = gg.parallel_commits;
    let mut commit_order: Vec<&crate::mermaid_renderer::ir::GitGraphCommit> =
        commits.iter().collect();
    if is_bottom_top && is_parallel {
        gitgraph_set_parallel_bt_pos(
            &commit_order,
            gg.default_pos,
            gg.commit_step,
            gg.layout_offset,
            &branch_pos,
            &mut commit_pos,
        );
    }
    if is_bottom_top {
        commit_order.reverse();
    }

    for commit in commit_order {
        if is_parallel {
            pos = gitgraph_calculate_position(
                commit,
                graph.direction,
                gg.default_pos,
                gg.commit_step,
                &commit_pos,
            );
        }
        let (x, y, pos_with_offset) = gitgraph_commit_position(
            commit,
            pos,
            is_parallel,
            graph.direction,
            gg.layout_offset,
            &branch_pos,
        );
        let axis_pos = pos;
        let (_branch_axis_pos, branch_index, _bw, _bh) = branch_pos
            .get(&commit.branch)
            .cloned()
            .unwrap_or((0.0, 0, 0.0, 0.0));

        let show_label = gg.show_commit_label
            && commit.commit_type != crate::mermaid_renderer::ir::GitGraphCommitType::CherryPick
            && (commit.commit_type != crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                || commit.custom_id);
        let label = if show_label {
            let (label_width, label_height) = measure_gitgraph_text(
                &commit.id,
                gg.commit_label_font_size,
                gg.commit_label_line_height,
                gg.text_width_scale,
                theme.font_family.as_str(),
                config.fast_text_metrics,
            );
            let (text_x, text_y, bg_x, bg_y, transform) = if is_vertical {
                let text_x = x - (label_width + gg.commit_label_tb_text_extra);
                let text_y = y + label_height + gg.commit_label_tb_text_offset_y;
                let bg_x = x - (label_width + gg.commit_label_tb_bg_extra);
                let bg_y = y + gg.commit_label_tb_bg_offset_y;
                let transform = if gg.rotate_commit_label {
                    Some(GitGraphTransform {
                        translate_x: 0.0,
                        translate_y: 0.0,
                        rotate_deg: gg.commit_label_rotate_angle,
                        rotate_cx: x,
                        rotate_cy: y,
                    })
                } else {
                    None
                };
                (text_x, text_y, bg_x, bg_y, transform)
            } else {
                let text_x = pos_with_offset - label_width / 2.0;
                let text_y = y + gg.commit_label_offset_y;
                let bg_x = pos_with_offset - label_width / 2.0 - gg.commit_label_padding;
                let bg_y = y + gg.commit_label_bg_offset_y;
                let transform = if gg.rotate_commit_label {
                    let rotate_x = gg.commit_label_rotate_translate_x_base
                        - (label_width + gg.commit_label_rotate_translate_x_width_offset)
                            * gg.commit_label_rotate_translate_x_scale;
                    let rotate_y = gg.commit_label_rotate_translate_y_base
                        + label_width * gg.commit_label_rotate_translate_y_scale;
                    Some(GitGraphTransform {
                        translate_x: rotate_x,
                        translate_y: rotate_y,
                        rotate_deg: gg.commit_label_rotate_angle,
                        rotate_cx: axis_pos,
                        rotate_cy: y,
                    })
                } else {
                    None
                };
                (text_x, text_y, bg_x, bg_y, transform)
            };
            let bg_width = label_width + 2.0 * gg.commit_label_padding;
            let bg_height = label_height + 2.0 * gg.commit_label_padding;
            Some(GitGraphCommitLabelLayout {
                text: commit.id.clone(),
                text_x,
                text_y,
                bg_x,
                bg_y,
                bg_width,
                bg_height,
                transform,
            })
        } else {
            None
        };

        let mut tag_layouts = Vec::new();
        if !commit.tags.is_empty() {
            let mut max_width = 0.0f32;
            let mut max_height = 0.0f32;
            let mut tag_defs = Vec::new();
            let mut y_offset = 0.0f32;
            for tag_value in commit.tags.iter().rev() {
                let (w, h) = measure_gitgraph_text(
                    tag_value,
                    gg.tag_label_font_size,
                    gg.tag_label_line_height,
                    gg.text_width_scale,
                    theme.font_family.as_str(),
                    config.fast_text_metrics,
                );
                max_width = max_width.max(w);
                max_height = max_height.max(h);
                tag_defs.push((tag_value.clone(), w, y_offset));
                y_offset += gg.tag_spacing_y;
            }
            let half_h = max_height / 2.0;
            for (text, text_width, tag_offset) in tag_defs {
                if is_vertical {
                    let y_origin = axis_pos + tag_offset;
                    let px = gg.tag_padding_x;
                    let py = gg.tag_padding_y;
                    let text_translate_delta =
                        gg.tag_text_rotate_translate - gg.tag_rotate_translate;
                    let text_x = x + gg.tag_text_offset_x_tb + text_translate_delta;
                    let text_y = y_origin + gg.tag_text_offset_y_tb + text_translate_delta;
                    let points = vec![
                        (x, y_origin + py),
                        (x, y_origin - py),
                        (x + gg.layout_offset, y_origin - half_h - py),
                        (
                            x + gg.layout_offset + max_width + px,
                            y_origin - half_h - py,
                        ),
                        (
                            x + gg.layout_offset + max_width + px,
                            y_origin + half_h + py,
                        ),
                        (x + gg.layout_offset, y_origin + half_h + py),
                    ];
                    let hole_x = x + px / 2.0;
                    let hole_y = y_origin;
                    tag_layouts.push(GitGraphTagLayout {
                        text,
                        text_x,
                        text_y,
                        points,
                        hole_x,
                        hole_y,
                        transform: Some(GitGraphTransform {
                            translate_x: gg.tag_rotate_translate,
                            translate_y: gg.tag_rotate_translate,
                            rotate_deg: gg.tag_rotate_angle,
                            rotate_cx: x,
                            rotate_cy: axis_pos,
                        }),
                    });
                } else {
                    let text_x = pos_with_offset - text_width / 2.0;
                    let text_y = y - gg.tag_text_offset_y - tag_offset;
                    let ly = y - gg.tag_polygon_offset_y - tag_offset;
                    let px = gg.tag_padding_x;
                    let py = gg.tag_padding_y;
                    let points = vec![
                        (axis_pos - max_width / 2.0 - px / 2.0, ly + py),
                        (axis_pos - max_width / 2.0 - px / 2.0, ly - py),
                        (pos_with_offset - max_width / 2.0 - px, ly - half_h - py),
                        (pos_with_offset + max_width / 2.0 + px, ly - half_h - py),
                        (pos_with_offset + max_width / 2.0 + px, ly + half_h + py),
                        (pos_with_offset - max_width / 2.0 - px, ly + half_h + py),
                    ];
                    let hole_x = axis_pos - max_width / 2.0 + px / 2.0;
                    let hole_y = ly;
                    tag_layouts.push(GitGraphTagLayout {
                        text,
                        text_x,
                        text_y,
                        points,
                        hole_x,
                        hole_y,
                        transform: None,
                    });
                }
            }
        }

        commit_layouts.push(GitGraphCommitLayout {
            id: commit.id.clone(),
            seq: commit.seq,
            branch_index,
            x,
            y,
            axis_pos,
            commit_type: commit.commit_type,
            custom_type: commit.custom_type,
            tags: tag_layouts,
            label,
        });

        if is_vertical {
            commit_pos.insert(commit.id.clone(), (x, pos_with_offset));
        } else {
            commit_pos.insert(commit.id.clone(), (pos_with_offset, y));
        }
        pos = if is_bottom_top && is_parallel {
            pos + gg.commit_step
        } else {
            pos + gg.commit_step + gg.layout_offset
        };
        if pos > max_pos {
            max_pos = pos;
        }
    }

    if is_bottom_top {
        for branch in &mut branch_layouts {
            branch.label.bg_y = max_pos + gg.branch_label_bt_offset_y;
            branch.label.text_y = max_pos + gg.branch_label_bt_offset_y;
        }
    }

    let mut arrows = Vec::new();
    let mut lanes = Vec::new();
    for commit in &graph.gitgraph.commits {
        if commit.parents.is_empty() {
            continue;
        }
        for parent in &commit.parents {
            if let (Some((p1x, p1y)), Some((p2x, p2y))) =
                (commit_pos.get(parent), commit_pos.get(&commit.id))
            {
                let commit_a = commit_by_id(&graph.gitgraph.commits, parent);
                let commit_b = commit_by_id(&graph.gitgraph.commits, &commit.id);
                if let (Some(commit_a), Some(commit_b)) = (commit_a, commit_b) {
                    let path = gitgraph_arrow_path(
                        graph.direction,
                        commit_a,
                        commit_b,
                        (*p1x, *p1y),
                        (*p2x, *p2y),
                        &graph.gitgraph.commits,
                        gg,
                        &mut lanes,
                    );
                    let mut color_index =
                        branch_pos.get(&commit_b.branch).map(|v| v.1).unwrap_or(0);
                    if commit_b.commit_type
                        == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                        && commit_a.id != commit_b.parents.first().cloned().unwrap_or_default()
                    {
                        color_index = branch_pos
                            .get(&commit_a.branch)
                            .map(|v| v.1)
                            .unwrap_or(color_index);
                    }
                    arrows.push(GitGraphArrowLayout { path, color_index });
                }
            }
        }
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for branch in &branch_layouts {
        let (x1, y1, x2, y2) = if is_vertical {
            let start = if is_bottom_top {
                max_pos
            } else {
                gg.default_pos
            };
            let end = if is_bottom_top {
                gg.default_pos
            } else {
                max_pos
            };
            (branch.pos, start, branch.pos, end)
        } else {
            (0.0, branch.pos, max_pos, branch.pos)
        };
        update_bounds_line(
            &mut min_x, &mut min_y, &mut max_x, &mut max_y, x1, y1, x2, y2,
        );
        update_bounds_rect(
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
            branch.label.bg_x,
            branch.label.bg_y,
            branch.label.bg_width,
            branch.label.bg_height,
            None,
        );
    }

    for commit in &commit_layouts {
        let radius = if commit.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
        {
            gg.merge_radius_outer
        } else {
            gg.commit_radius
        };
        update_bounds_rect(
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
            commit.x - radius,
            commit.y - radius,
            radius * 2.0,
            radius * 2.0,
            None,
        );
        if let Some(label) = &commit.label {
            update_bounds_rect(
                &mut min_x,
                &mut min_y,
                &mut max_x,
                &mut max_y,
                label.bg_x,
                label.bg_y,
                label.bg_width,
                label.bg_height,
                label.transform.as_ref(),
            );
        }
        for tag in &commit.tags {
            update_bounds_points(
                &mut min_x,
                &mut min_y,
                &mut max_x,
                &mut max_y,
                &tag.points,
                tag.transform.as_ref(),
            );
        }
    }

    if !min_x.is_finite() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 1.0;
        max_y = 1.0;
    }

    min_x -= gg.diagram_padding;
    min_y -= gg.diagram_padding;
    max_x += gg.diagram_padding;
    max_y += gg.diagram_padding;

    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: Some(GitGraphLayout {
            branches: branch_layouts,
            commits: commit_layouts,
            arrows,
            width,
            height,
            offset_x: -min_x,
            offset_y: -min_y,
            max_pos,
            direction: graph.direction,
        }),
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn default_branch_order(index: usize) -> f32 {
    if index == 0 {
        return 0.0;
    }
    let mut denom = 1.0f32;
    let mut value = index;
    while value > 0 {
        denom *= 10.0;
        value /= 10;
    }
    (index as f32) / denom
}

fn measure_gitgraph_text(
    text: &str,
    font_size: f32,
    line_height: f32,
    width_scale: f32,
    font_family: &str,
    fast_metrics: bool,
) -> (f32, f32) {
    let lines = split_lines(text);
    let max_width = lines
        .iter()
        .map(|line| text_width(line, font_size, font_family, fast_metrics))
        .fold(0.0, f32::max);
    let width = max_width * width_scale;
    let height = lines.len() as f32 * font_size * line_height;
    (width, height)
}

fn commit_by_id<'a>(
    commits: &'a [crate::mermaid_renderer::ir::GitGraphCommit],
    id: &str,
) -> Option<&'a crate::mermaid_renderer::ir::GitGraphCommit> {
    commits.iter().find(|commit| commit.id == id)
}

fn gitgraph_find_closest_parent(
    parents: &[String],
    commit_pos: &HashMap<String, (f32, f32)>,
    dir: Direction,
) -> Option<String> {
    let mut chosen: Option<String> = None;
    let mut target = if dir == Direction::BottomTop {
        f32::INFINITY
    } else {
        0.0
    };
    for parent in parents {
        if let Some((x, y)) = commit_pos.get(parent) {
            let pos = if matches!(dir, Direction::TopDown | Direction::BottomTop) {
                *y
            } else {
                *x
            };
            let accept = if dir == Direction::BottomTop {
                pos <= target
            } else {
                pos >= target
            };
            if accept {
                target = pos;
                chosen = Some(parent.clone());
            }
        }
    }
    chosen
}

fn gitgraph_find_closest_parent_bt(
    parents: &[String],
    commit_pos: &HashMap<String, (f32, f32)>,
) -> Option<String> {
    let mut chosen: Option<String> = None;
    let mut max_pos = f32::INFINITY;
    for parent in parents {
        if let Some((_x, y)) = commit_pos.get(parent)
            && *y <= max_pos
        {
            max_pos = *y;
            chosen = Some(parent.clone());
        }
    }
    chosen
}

fn gitgraph_find_closest_parent_pos(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    commit_pos: &HashMap<String, (f32, f32)>,
) -> Option<f32> {
    let closest_parent =
        gitgraph_find_closest_parent(&commit.parents, commit_pos, Direction::BottomTop)?;
    commit_pos.get(&closest_parent).map(|(_x, y)| *y)
}

fn gitgraph_calculate_commit_position(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    commit_step: f32,
    commit_pos: &HashMap<String, (f32, f32)>,
) -> f32 {
    let closest_parent_pos = gitgraph_find_closest_parent_pos(commit, commit_pos).unwrap_or(0.0);
    closest_parent_pos + commit_step
}

fn gitgraph_set_commit_position(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    cur_pos: f32,
    layout_offset: f32,
    branch_pos: &HashMap<String, (f32, usize, f32, f32)>,
    commit_pos: &mut HashMap<String, (f32, f32)>,
) -> (f32, f32) {
    let x = branch_pos
        .get(&commit.branch)
        .map(|value| value.0)
        .unwrap_or(0.0);
    let y = cur_pos + layout_offset;
    commit_pos.insert(commit.id.clone(), (x, y));
    (x, y)
}

fn gitgraph_set_root_position(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    cur_pos: f32,
    default_pos: f32,
    branch_pos: &HashMap<String, (f32, usize, f32, f32)>,
    commit_pos: &mut HashMap<String, (f32, f32)>,
) {
    let x = branch_pos
        .get(&commit.branch)
        .map(|value| value.0)
        .unwrap_or(0.0);
    let y = cur_pos + default_pos;
    commit_pos.insert(commit.id.clone(), (x, y));
}

fn gitgraph_set_parallel_bt_pos(
    commits: &[&crate::mermaid_renderer::ir::GitGraphCommit],
    default_pos: f32,
    commit_step: f32,
    layout_offset: f32,
    branch_pos: &HashMap<String, (f32, usize, f32, f32)>,
    commit_pos: &mut HashMap<String, (f32, f32)>,
) {
    let mut cur_pos = default_pos;
    let mut max_position = default_pos;
    let mut roots = Vec::new();
    for commit in commits {
        if !commit.parents.is_empty() {
            cur_pos = gitgraph_calculate_commit_position(commit, commit_step, commit_pos);
            max_position = max_position.max(cur_pos);
        } else {
            roots.push(*commit);
        }
        gitgraph_set_commit_position(commit, cur_pos, layout_offset, branch_pos, commit_pos);
    }
    cur_pos = max_position;
    for commit in roots {
        gitgraph_set_root_position(commit, cur_pos, default_pos, branch_pos, commit_pos);
    }
    for commit in commits {
        if !commit.parents.is_empty()
            && let Some(closest_parent) =
                gitgraph_find_closest_parent_bt(&commit.parents, commit_pos)
            && let Some((_x, y)) = commit_pos.get(&closest_parent)
        {
            cur_pos = *y - commit_step;
            if cur_pos <= max_position {
                max_position = cur_pos;
            }
            let x = branch_pos
                .get(&commit.branch)
                .map(|value| value.0)
                .unwrap_or(0.0);
            let y = cur_pos - layout_offset;
            commit_pos.insert(commit.id.clone(), (x, y));
        }
    }
}

fn gitgraph_calculate_position(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    dir: Direction,
    default_pos: f32,
    commit_step: f32,
    commit_pos: &HashMap<String, (f32, f32)>,
) -> f32 {
    let default_commit_pos = (0.0, 0.0);
    if !commit.parents.is_empty() {
        if let Some(parent) = gitgraph_find_closest_parent(&commit.parents, commit_pos, dir) {
            let parent_pos = commit_pos
                .get(&parent)
                .cloned()
                .unwrap_or(default_commit_pos);
            if dir == Direction::TopDown {
                return parent_pos.1 + commit_step;
            } else if dir == Direction::BottomTop {
                let current = commit_pos
                    .get(&commit.id)
                    .cloned()
                    .unwrap_or(default_commit_pos);
                return current.1 - commit_step;
            } else {
                return parent_pos.0 + commit_step;
            }
        }
    } else if dir == Direction::TopDown {
        return default_pos;
    } else if dir == Direction::BottomTop {
        let current = commit_pos
            .get(&commit.id)
            .cloned()
            .unwrap_or(default_commit_pos);
        return current.1 - commit_step;
    } else {
        return 0.0;
    }
    0.0
}

fn gitgraph_commit_position(
    commit: &crate::mermaid_renderer::ir::GitGraphCommit,
    pos: f32,
    is_parallel: bool,
    dir: Direction,
    layout_offset: f32,
    branch_pos: &HashMap<String, (f32, usize, f32, f32)>,
) -> (f32, f32, f32) {
    let pos_with_offset = if dir == Direction::BottomTop && is_parallel {
        pos
    } else {
        pos + layout_offset
    };
    let branch_axis_pos = branch_pos
        .get(&commit.branch)
        .map(|value| value.0)
        .unwrap_or(0.0);
    let (x, y) = if matches!(dir, Direction::TopDown | Direction::BottomTop) {
        (branch_axis_pos, pos_with_offset)
    } else {
        (pos_with_offset, branch_axis_pos)
    };
    (x, y, pos_with_offset)
}

fn update_bounds_line(
    min_x: &mut f32,
    min_y: &mut f32,
    max_x: &mut f32,
    max_y: &mut f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
) {
    *min_x = min_x.min(x1.min(x2));
    *min_y = min_y.min(y1.min(y2));
    *max_x = max_x.max(x1.max(x2));
    *max_y = max_y.max(y1.max(y2));
}

fn update_bounds_rect(
    min_x: &mut f32,
    min_y: &mut f32,
    max_x: &mut f32,
    max_y: &mut f32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    transform: Option<&GitGraphTransform>,
) {
    let corners = [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ];
    update_bounds_points(min_x, min_y, max_x, max_y, &corners, transform);
}

fn update_bounds_points(
    min_x: &mut f32,
    min_y: &mut f32,
    max_x: &mut f32,
    max_y: &mut f32,
    points: &[(f32, f32)],
    transform: Option<&GitGraphTransform>,
) {
    for (x, y) in points {
        let (px, py) = apply_transform_point(*x, *y, transform);
        *min_x = min_x.min(px);
        *min_y = min_y.min(py);
        *max_x = max_x.max(px);
        *max_y = max_y.max(py);
    }
}

fn apply_transform_point(x: f32, y: f32, transform: Option<&GitGraphTransform>) -> (f32, f32) {
    if let Some(transform) = transform {
        let mut px = x + transform.translate_x;
        let mut py = y + transform.translate_y;
        if transform.rotate_deg.abs() > f32::EPSILON {
            let angle = transform.rotate_deg.to_radians();
            let cos = angle.cos();
            let sin = angle.sin();
            let dx = px - transform.rotate_cx;
            let dy = py - transform.rotate_cy;
            px = transform.rotate_cx + dx * cos - dy * sin;
            py = transform.rotate_cy + dx * sin + dy * cos;
        }
        (px, py)
    } else {
        (x, y)
    }
}

fn gitgraph_arrow_path(
    dir: Direction,
    commit_a: &crate::mermaid_renderer::ir::GitGraphCommit,
    commit_b: &crate::mermaid_renderer::ir::GitGraphCommit,
    p1: (f32, f32),
    p2: (f32, f32),
    commits: &[crate::mermaid_renderer::ir::GitGraphCommit],
    config: &crate::mermaid_renderer::config::GitGraphConfig,
    lanes: &mut Vec<f32>,
) -> String {
    let (p1x, p1y) = p1;
    let (p2x, p2y) = p2;
    let arrow_needs_reroute = should_reroute_arrow(dir, commit_a, commit_b, p1, p2, commits);
    let (arc, arc2, radius, offset) = if arrow_needs_reroute {
        let radius = config.arrow_reroute_radius;
        (
            format!("A {radius} {radius}, 0, 0, 0,"),
            format!("A {radius} {radius}, 0, 0, 1,"),
            radius,
            radius,
        )
    } else {
        let radius = config.arrow_radius;
        (
            format!("A {radius} {radius}, 0, 0, 0,"),
            format!("A {radius} {radius}, 0, 0, 1,"),
            radius,
            radius,
        )
    };

    let mut line_def = String::new();
    if arrow_needs_reroute {
        let line_y = if p1y < p2y {
            find_lane(p1y, p2y, lanes, config, 0)
        } else {
            find_lane(p2y, p1y, lanes, config, 0)
        };
        let line_x = if p1x < p2x {
            find_lane(p1x, p2x, lanes, config, 0)
        } else {
            find_lane(p2x, p1x, lanes, config, 0)
        };
        match dir {
            Direction::TopDown => {
                if p1x < p2x {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc2} {line_x} {y1} L {line_x} {y2} {arc} {x2} {p2y} L {p2x} {p2y}",
                        x1 = line_x - radius,
                        y1 = p1y + offset,
                        y2 = p2y - radius,
                        x2 = line_x + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {line_x} {y1} L {line_x} {y2} {arc2} {x2} {p2y} L {p2x} {p2y}",
                        x1 = line_x + radius,
                        y1 = p1y + offset,
                        y2 = p2y - radius,
                        x2 = line_x - offset
                    );
                }
            }
            Direction::BottomTop => {
                if p1x < p2x {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {line_x} {y1} L {line_x} {y2} {arc2} {x2} {p2y} L {p2x} {p2y}",
                        x1 = line_x - radius,
                        y1 = p1y - offset,
                        y2 = p2y + radius,
                        x2 = line_x + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc2} {line_x} {y1} L {line_x} {y2} {arc} {x2} {p2y} L {p2x} {p2y}",
                        x1 = line_x + radius,
                        y1 = p1y - offset,
                        y2 = p2y + radius,
                        x2 = line_x - offset
                    );
                }
            }
            _ => {
                if p1y < p2y {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc} {x1} {line_y} L {x2} {line_y} {arc2} {p2x} {y2} L {p2x} {p2y}",
                        y1 = line_y - radius,
                        x1 = p1x + offset,
                        x2 = p2x - radius,
                        y2 = line_y + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc2} {x1} {line_y} L {x2} {line_y} {arc} {p2x} {y2} L {p2x} {p2y}",
                        y1 = line_y + radius,
                        x1 = p1x + offset,
                        x2 = p2x - radius,
                        y2 = line_y - offset
                    );
                }
            }
        }
        return line_def;
    }

    match dir {
        Direction::TopDown => {
            if p1x < p2x {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y - radius,
                        x1 = p1x + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc2} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x - radius,
                        y1 = p1y + offset
                    );
                }
            }
            if p1x > p2x {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc2} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y - radius,
                        x1 = p1x - offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x + radius,
                        y1 = p1y + offset
                    );
                }
            }
            if (p1x - p2x).abs() < f32::EPSILON {
                line_def = format!("M {p1x} {p1y} L {p2x} {p2y}");
            }
        }
        Direction::BottomTop => {
            if p1x < p2x {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc2} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y + radius,
                        x1 = p1x + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x - radius,
                        y1 = p1y - offset
                    );
                }
            }
            if p1x > p2x {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y + radius,
                        x1 = p1x - offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x - radius,
                        y1 = p1y - offset
                    );
                }
            }
            if (p1x - p2x).abs() < f32::EPSILON {
                line_def = format!("M {p1x} {p1y} L {p2x} {p2y}");
            }
        }
        _ => {
            if p1y < p2y {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc2} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x - radius,
                        y1 = p1y + offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y - radius,
                        x1 = p1x + offset
                    );
                }
            }
            if p1y > p2y {
                if commit_b.commit_type == crate::mermaid_renderer::ir::GitGraphCommitType::Merge
                    && commit_a.id
                        != commit_b
                            .parents
                            .first()
                            .cloned()
                            .unwrap_or_else(String::new)
                {
                    line_def = format!(
                        "M {p1x} {p1y} L {x1} {p1y} {arc} {p2x} {y1} L {p2x} {p2y}",
                        x1 = p2x - radius,
                        y1 = p1y - offset
                    );
                } else {
                    line_def = format!(
                        "M {p1x} {p1y} L {p1x} {y1} {arc2} {x1} {p2y} L {p2x} {p2y}",
                        y1 = p2y + radius,
                        x1 = p1x + offset
                    );
                }
            }
            if (p1y - p2y).abs() < f32::EPSILON {
                line_def = format!("M {p1x} {p1y} L {p2x} {p2y}");
            }
        }
    }

    if line_def.is_empty() {
        line_def = format!("M {p1x} {p1y} L {p2x} {p2y}");
    }
    line_def
}

fn should_reroute_arrow(
    dir: Direction,
    commit_a: &crate::mermaid_renderer::ir::GitGraphCommit,
    commit_b: &crate::mermaid_renderer::ir::GitGraphCommit,
    p1: (f32, f32),
    p2: (f32, f32),
    commits: &[crate::mermaid_renderer::ir::GitGraphCommit],
) -> bool {
    let commit_b_is_furthest = match dir {
        Direction::TopDown | Direction::BottomTop => p1.0 < p2.0,
        _ => p1.1 < p2.1,
    };
    let branch_to_get_curve = if commit_b_is_furthest {
        &commit_b.branch
    } else {
        &commit_a.branch
    };
    commits.iter().any(|commit| {
        commit.seq > commit_a.seq
            && commit.seq < commit_b.seq
            && &commit.branch == branch_to_get_curve
    })
}

fn find_lane(
    y1: f32,
    y2: f32,
    lanes: &mut Vec<f32>,
    config: &crate::mermaid_renderer::config::GitGraphConfig,
    depth: usize,
) -> f32 {
    let candidate = y1 + (y2 - y1).abs() / 2.0;
    if depth > config.lane_max_depth {
        return candidate;
    }
    let ok = lanes
        .iter()
        .all(|lane| (lane - candidate).abs() >= config.lane_spacing);
    if ok {
        lanes.push(candidate);
        return candidate;
    }
    let diff = (y1 - y2).abs();
    find_lane(y1, y2 - diff / 5.0, lanes, config, depth + 1)
}

fn compute_pie_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let pie_cfg = &config.pie;
    let mut slices = Vec::new();
    let mut legend = Vec::new();
    let title_block = graph.pie_title.as_ref().map(|title| {
        measure_label_with_font_size(
            title,
            theme.pie_title_text_size,
            config,
            false,
            theme.font_family.as_str(),
        )
    });

    let palette = pie_palette(theme);
    let total: f32 = graph
        .pie_slices
        .iter()
        .map(|slice| slice.value.max(0.0))
        .sum();
    let fallback_total = graph.pie_slices.len().max(1) as f32;
    let total = if total > 0.0 { total } else { fallback_total };

    #[derive(Clone)]
    struct PieDatum {
        index: usize,
        label: String,
        value: f32,
    }

    let mut filtered: Vec<PieDatum> = Vec::new();
    for (idx, slice) in graph.pie_slices.iter().enumerate() {
        let value = slice.value.max(0.0);
        let percent = if total > 0.0 {
            value / total * 100.0
        } else {
            0.0
        };
        if percent >= pie_cfg.min_percent {
            filtered.push(PieDatum {
                index: idx,
                label: slice.label.clone(),
                value,
            });
        }
    }
    filtered.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.index.cmp(&b.index))
    });

    let mut color_map: HashMap<String, String> = HashMap::new();
    let mut color_index: usize = 0;
    let mut resolve_color = |label: &str| -> String {
        if let Some(color) = color_map.get(label) {
            return color.clone();
        }
        let color = palette[color_index % palette.len()].clone();
        color_index += 1;
        color_map.insert(label.to_string(), color.clone());
        color
    };

    let mut angle = 0.0_f32;
    for datum in &filtered {
        let span = if total > 0.0 {
            datum.value / total * std::f32::consts::PI * 2.0
        } else {
            std::f32::consts::PI * 2.0 / fallback_total
        };
        let label = measure_label_with_font_size(
            &datum.label,
            theme.pie_section_text_size,
            config,
            false,
            theme.font_family.as_str(),
        );
        let color = resolve_color(&datum.label);
        slices.push(PieSliceLayout {
            label,
            value: datum.value,
            start_angle: angle,
            end_angle: angle + span,
            color,
        });
        angle += span;
    }

    let mut legend_width: f32 = 0.0;
    let mut legend_items: Vec<(TextBlock, String)> = Vec::new();
    for slice in &graph.pie_slices {
        let percent = if total > 0.0 {
            slice.value.max(0.0) / total * 100.0
        } else {
            0.0
        };
        let value_text = format_pie_value(slice.value);
        let label_text = if graph.pie_show_data {
            format!("{} — {} ({:.0}%)", slice.label, value_text, percent)
        } else {
            format!("{} ({:.0}%)", slice.label, percent)
        };
        let label = measure_label_with_font_size(
            &label_text,
            theme.pie_legend_text_size,
            config,
            false,
            theme.font_family.as_str(),
        );
        legend_width = legend_width.max(label.width);
        let color = resolve_color(&slice.label);
        legend_items.push((label, color));
    }

    let legend_item_height = pie_cfg.legend_rect_size + pie_cfg.legend_spacing;
    let legend_offset = legend_item_height * legend_items.len() as f32 / 2.0;

    let height = pie_cfg.height.max(1.0);
    let pie_width = height;
    let radius = (pie_width.min(height) / 2.0 - pie_cfg.margin).max(1.0);
    let center_x = pie_width / 2.0;
    let center_y = height / 2.0;
    let legend_x = center_x + radius + pie_cfg.margin * 0.6;

    for (idx, (label, color)) in legend_items.into_iter().enumerate() {
        let vertical = idx as f32 * legend_item_height - legend_offset;
        legend.push(PieLegendItem {
            x: legend_x,
            y: center_y + vertical,
            label,
            color,
            marker_size: pie_cfg.legend_rect_size,
            value: graph.pie_slices[idx].value,
        });
    }

    let width = legend_x
        + pie_cfg.legend_rect_size
        + pie_cfg.legend_spacing
        + legend_width
        + pie_cfg.margin * 0.4;
    let title_layout = title_block.map(|text| PieTitleLayout {
        x: center_x,
        y: center_y - (height - 50.0) / 2.0,
        text,
    });

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: slices,
        pie_legend: legend,
        pie_center: (center_x, center_y),
        pie_radius: radius,
        pie_title: title_layout,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width: width.max(200.0),
        height: height.max(1.0),
    }
}

fn compute_quadrant_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let padding = theme.font_size * 1.6;
    let grid_size = 360.0;
    // Measure title
    let title = graph
        .quadrant
        .title
        .as_ref()
        .map(|t| measure_label(t, theme, config));
    let title_height = title.as_ref().map(|t| t.height + padding).unwrap_or(0.0);

    // Measure axis labels
    let x_left = graph
        .quadrant
        .x_axis_left
        .as_ref()
        .map(|t| measure_label(t, theme, config));
    let x_right = graph
        .quadrant
        .x_axis_right
        .as_ref()
        .map(|t| measure_label(t, theme, config));
    let y_bottom = graph
        .quadrant
        .y_axis_bottom
        .as_ref()
        .map(|t| measure_label(t, theme, config));
    let y_top = graph
        .quadrant
        .y_axis_top
        .as_ref()
        .map(|t| measure_label(t, theme, config));

    // Measure quadrant labels
    let q_labels: [Option<TextBlock>; 4] = [
        graph.quadrant.quadrant_labels[0]
            .as_ref()
            .map(|t| measure_label(t, theme, config)),
        graph.quadrant.quadrant_labels[1]
            .as_ref()
            .map(|t| measure_label(t, theme, config)),
        graph.quadrant.quadrant_labels[2]
            .as_ref()
            .map(|t| measure_label(t, theme, config)),
        graph.quadrant.quadrant_labels[3]
            .as_ref()
            .map(|t| measure_label(t, theme, config)),
    ];

    let y_axis_width = y_bottom
        .as_ref()
        .map(|t| t.height + padding)
        .unwrap_or(padding);
    let x_axis_height = x_left
        .as_ref()
        .map(|t| t.height + padding)
        .unwrap_or(padding);

    let grid_x = y_axis_width + padding;
    let grid_y = title_height + padding;

    // Layout points
    let palette = quadrant_palette(theme);
    let points: Vec<QuadrantPointLayout> = graph
        .quadrant
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let px = grid_x + p.x.clamp(0.0, 1.0) * grid_size;
            let py = grid_y + (1.0 - p.y.clamp(0.0, 1.0)) * grid_size; // Invert Y
            QuadrantPointLayout {
                label: measure_label(&p.label, theme, config),
                x: px,
                y: py,
                color: palette[i % palette.len()].clone(),
            }
        })
        .collect();

    let width = grid_x + grid_size + padding * 2.0;
    let height = grid_y + grid_size + x_axis_height + padding;

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: Some(QuadrantLayout {
            title,
            title_y: title_height / 2.0,
            x_axis_left: x_left,
            x_axis_right: x_right,
            y_axis_bottom: y_bottom,
            y_axis_top: y_top,
            quadrant_labels: q_labels,
            points,
            grid_x,
            grid_y,
            grid_width: grid_size,
            grid_height: grid_size,
        }),
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn quadrant_palette(_theme: &Theme) -> Vec<String> {
    vec![
        "#6366f1".to_string(), // indigo
        "#f59e0b".to_string(), // amber
        "#10b981".to_string(), // emerald
        "#ef4444".to_string(), // red
        "#8b5cf6".to_string(), // violet
        "#06b6d4".to_string(), // cyan
    ]
}

fn compute_gantt_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let padding = theme.font_size * 1.25;
    let row_height = (theme.font_size * 2.8).max(theme.font_size + 18.0);
    let label_gap = theme.font_size * 1.05;
    let default_duration = 3.0_f32;

    let title = graph
        .gantt_title
        .as_ref()
        .map(|t| measure_label(t, theme, config));
    let title_height = title.as_ref().map(|t| t.height + padding).unwrap_or(0.0);

    let mut task_label_width = 0.0_f32;
    let mut section_label_width = 0.0_f32;
    for task in &graph.gantt_tasks {
        let label = measure_label(&task.label, theme, config);
        task_label_width = task_label_width.max(label.width);
        if let Some(section) = task.section.as_ref() {
            let section_label = measure_label(section, theme, config);
            section_label_width = section_label_width.max(section_label.width);
        }
    }
    task_label_width = task_label_width.max(theme.font_size * 6.5);

    let label_x = padding;
    let section_task_gap = if section_label_width > 0.0 {
        theme.font_size * 0.8
    } else {
        0.0
    };
    let label_width = section_label_width + section_task_gap + task_label_width;
    let section_label_x = label_x;
    let task_label_x = label_x + section_label_width + section_task_gap;
    let chart_x = padding + label_width + label_gap;
    let chart_y = title_height + padding;
    let chart_width = theme.font_size * 26.0;

    let mut parsed_starts: HashMap<String, f32> = HashMap::new();
    let mut origin: Option<f32> = None;
    for task in &graph.gantt_tasks {
        if let Some(start) = task.start.as_deref().and_then(parse_gantt_date) {
            let start = start as f32;
            parsed_starts.insert(task.id.clone(), start);
            origin = Some(origin.map_or(start, |v| v.min(start)));
        }
    }
    let has_dates = origin.is_some();

    let mut timing: HashMap<String, (f32, f32)> = HashMap::new();
    let mut cursor = 0.0_f32;
    let mut time_start = f32::MAX;
    let mut time_end = f32::MIN;

    let mut computed: Vec<(
        String,
        f32,
        f32,
        Option<crate::mermaid_renderer::ir::GanttStatus>,
        Option<String>,
    )> = Vec::with_capacity(graph.gantt_tasks.len());
    for task in &graph.gantt_tasks {
        let duration = task
            .duration
            .as_deref()
            .and_then(parse_gantt_duration)
            .unwrap_or(default_duration)
            .max(0.1);
        let mut start = parsed_starts.get(&task.id).copied();
        if start.is_none() {
            if let Some(after_id) = task.after.as_deref() {
                if let Some((_, end)) = timing.get(after_id) {
                    start = Some(*end);
                }
            }
        }
        let fallback_base = origin.unwrap_or(0.0);
        let start = start.unwrap_or(fallback_base + cursor);
        let end = start + duration;
        timing.insert(task.id.clone(), (start, end));
        cursor = cursor.max(end + 0.5);
        time_start = time_start.min(start);
        time_end = time_end.max(end);
        computed.push((
            task.label.clone(),
            start,
            duration,
            task.status,
            task.section.clone(),
        ));
    }
    if !time_start.is_finite() || !time_end.is_finite() {
        time_start = 0.0;
        time_end = 1.0;
    }
    if (time_end - time_start).abs() < 0.01 {
        time_end = time_start + 1.0;
    }
    let time_span = (time_end - time_start).max(1.0);
    let time_scale = chart_width / time_span;

    let mut ticks: Vec<GanttTick> = Vec::new();
    let tick_count = 4;
    for i in 0..=tick_count {
        let t = time_start + time_span * (i as f32) / (tick_count as f32);
        let x = chart_x + (t - time_start) * time_scale;
        let label = if has_dates {
            format_gantt_date(t.round() as i32)
        } else {
            format!("{:.0}", t - time_start)
        };
        ticks.push(GanttTick { x, label });
    }

    let palette = gantt_palette(theme);
    let section_palette = gantt_section_palette(theme, &graph.gantt_sections);
    let mut current_section: Option<String> = None;
    let mut current_section_idx: Option<usize> = None;
    let mut sections: Vec<GanttSectionLayout> = Vec::new();
    let mut tasks: Vec<GanttTaskLayout> = Vec::new();
    let mut y = chart_y;

    for (idx, (label, start, duration, status, section)) in computed.iter().enumerate() {
        if section != &current_section {
            if let Some(sec) = section.as_ref() {
                if let Some(prev_idx) = current_section_idx {
                    let height = (y - sections[prev_idx].y).max(row_height);
                    sections[prev_idx].height = height;
                }
                let base_color = section_palette
                    .get(sec)
                    .cloned()
                    .unwrap_or_else(|| palette[idx % palette.len()].clone());
                let band_color = shift_color(&base_color, 20.0, 92.0, 0.7);
                sections.push(GanttSectionLayout {
                    label: measure_label(sec, theme, config),
                    y,
                    height: 0.0,
                    color: base_color,
                    band_color,
                });
                current_section_idx = Some(sections.len() - 1);
            } else if let Some(prev_idx) = current_section_idx {
                let height = (y - sections[prev_idx].y).max(row_height);
                sections[prev_idx].height = height;
                current_section_idx = None;
            }
            current_section = section.clone();
        }

        let bar_x = chart_x + (start - time_start) * time_scale;
        let mut bar_width = duration * time_scale;
        let min_width = row_height * 0.5;
        if bar_width < min_width {
            bar_width = min_width;
        }
        let base_color = if let Some(sec) = section.as_ref() {
            section_palette
                .get(sec)
                .cloned()
                .unwrap_or_else(|| palette[idx % palette.len()].clone())
        } else {
            palette[idx % palette.len()].clone()
        };
        let color = gantt_task_color(*status, &base_color, &palette[0]);

        tasks.push(GanttTaskLayout {
            label: measure_label(label, theme, config),
            x: bar_x,
            y,
            width: bar_width,
            height: row_height,
            color,
            start: *start,
            duration: *duration,
            status: *status,
        });
        y += row_height;
    }
    if let Some(prev_idx) = current_section_idx {
        let height = (y - sections[prev_idx].y).max(row_height);
        sections[prev_idx].height = height;
    }

    let axis_pad = row_height * 0.9 + theme.font_size;
    let height = y + padding + axis_pad;
    let width = chart_x + chart_width + padding;

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: Some(GanttLayout {
            title,
            sections,
            tasks,
            time_start,
            time_end,
            chart_x,
            chart_y,
            chart_width,
            chart_height: y - chart_y,
            row_height,
            label_x,
            label_width,
            section_label_x,
            section_label_width,
            task_label_x,
            task_label_width,
            title_y: chart_y - row_height * 0.6,
            ticks,
        }),
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn gantt_palette(theme: &Theme) -> Vec<String> {
    vec![
        theme.primary_border_color.clone(),
        "#0ea5e9".to_string(), // sky-500
        "#10b981".to_string(), // emerald-500
        "#6366f1".to_string(), // indigo-500
        "#f97316".to_string(), // orange-500
    ]
}

fn hsl_color(h: f32, s: f32, l: f32) -> String {
    format!("hsl({:.10}, {:.10}%, {:.10}%)", h, s, l)
}

fn shift_color(color: &str, target_s: f32, target_l: f32, strength: f32) -> String {
    let Some((_h, s, l)) = parse_color_to_hsl(color) else {
        return color.to_string();
    };
    let delta_s = (target_s - s) * strength;
    let delta_l = (target_l - l) * strength;
    adjust_color(color, 0.0, delta_s, delta_l)
}

fn gantt_section_palette(theme: &Theme, sections: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if sections.is_empty() {
        return map;
    }
    let base = theme.primary_border_color.as_str();
    let step = 360.0 / sections.len().max(1) as f32;
    for (idx, name) in sections.iter().enumerate() {
        let hue_shift = step * idx as f32;
        let mut color = adjust_color(base, hue_shift, 0.0, 0.0);
        color = shift_color(&color, 60.0, 55.0, 0.4);
        map.insert(name.clone(), color);
    }
    map
}

fn gantt_task_color(
    status: Option<crate::mermaid_renderer::ir::GanttStatus>,
    base: &str,
    fallback: &str,
) -> String {
    let base = if parse_color_to_hsl(base).is_some() {
        base.to_string()
    } else {
        fallback.to_string()
    };
    match status {
        Some(crate::mermaid_renderer::ir::GanttStatus::Done) => shift_color(&base, 30.0, 80.0, 0.7),
        Some(crate::mermaid_renderer::ir::GanttStatus::Active) => {
            shift_color(&base, 70.0, 52.0, 0.6)
        }
        Some(crate::mermaid_renderer::ir::GanttStatus::Crit) => {
            if let Some((_, s, l)) = parse_color_to_hsl(&base) {
                hsl_color(0.0, s.max(65.0), l.clamp(45.0, 60.0))
            } else {
                "#ef4444".to_string()
            }
        }
        Some(crate::mermaid_renderer::ir::GanttStatus::Milestone) => {
            if let Some((_, s, l)) = parse_color_to_hsl(&base) {
                hsl_color(45.0, s.max(65.0), l.clamp(50.0, 65.0))
            } else {
                "#f59e0b".to_string()
            }
        }
        None => base,
    }
}

fn parse_gantt_duration(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut digits = String::new();
    let mut unit = None;
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            digits.push(ch);
        } else if !ch.is_whitespace() {
            unit = Some(ch.to_ascii_lowercase());
        }
    }
    let number: f32 = digits.parse().ok()?;
    let mult = match unit {
        Some('d') => 1.0,
        Some('w') => 7.0,
        Some('h') => 1.0 / 24.0,
        Some('m') => 30.0,
        Some('y') => 365.0,
        _ => 1.0,
    };
    Some(number * mult)
}

fn parse_gantt_date(value: &str) -> Option<i32> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let parts: Vec<&str> = value
        .split(|ch| ch == '-' || ch == '/' || ch == '.')
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    if month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i32 {
    let y = year - (month <= 2) as i32;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i32;
    let d = day as i32;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(days: i32) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + (m <= 2) as i32;
    (year, m as u32, d as u32)
}

fn format_gantt_date(days: i32) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn pie_palette(theme: &Theme) -> Vec<String> {
    theme.pie_colors.to_vec()
}

#[allow(dead_code)]
fn format_pie_value(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.001 {
        format!("{:.0}", rounded)
    } else {
        format!("{:.2}", rounded)
    }
}

fn compute_sankey_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    const SANKEY_WIDTH: f32 = 560.0;
    const SANKEY_HEIGHT: f32 = 360.0;
    const SANKEY_NODE_WIDTH: f32 = 10.0;
    const SANKEY_PALETTE: [&str; 10] = [
        "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
        "#9c755f", "#bab0ab",
    ];

    let mut node_ids: Vec<String> = graph.nodes.keys().cloned().collect();
    node_ids.sort_by(|a, b| {
        let order_a = graph.node_order.get(a).copied().unwrap_or(usize::MAX);
        let order_b = graph.node_order.get(b).copied().unwrap_or(usize::MAX);
        order_a.cmp(&order_b).then_with(|| a.cmp(b))
    });

    let node_count = node_ids.len();
    let mut id_to_idx: HashMap<String, usize> = HashMap::new();
    for (idx, id) in node_ids.iter().enumerate() {
        id_to_idx.insert(id.clone(), idx);
    }

    let node_order_idx: Vec<usize> = node_ids
        .iter()
        .map(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX))
        .collect();

    #[derive(Debug, Clone)]
    struct SankeyEdgeData {
        from_idx: usize,
        to_idx: usize,
        value: f32,
    }

    let mut edges_data: Vec<SankeyEdgeData> = Vec::new();
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut indegree: Vec<usize> = vec![0; node_count];
    let mut in_total: Vec<f32> = vec![0.0; node_count];
    let mut out_total: Vec<f32> = vec![0.0; node_count];

    for edge in &graph.edges {
        let Some(&from_idx) = id_to_idx.get(&edge.from) else {
            continue;
        };
        let Some(&to_idx) = id_to_idx.get(&edge.to) else {
            continue;
        };
        let raw_value = edge
            .label
            .as_deref()
            .and_then(|text| text.parse::<f32>().ok())
            .unwrap_or(1.0);
        let value = raw_value.max(0.0);
        let edge_idx = edges_data.len();
        edges_data.push(SankeyEdgeData {
            from_idx,
            to_idx,
            value,
        });
        outgoing[from_idx].push(edge_idx);
        incoming[to_idx].push(edge_idx);
        indegree[to_idx] += 1;
        out_total[from_idx] += value;
        in_total[to_idx] += value;
    }

    let mut ranks = vec![0usize; node_count];
    let mut indegree_work = indegree.clone();
    let mut queue: VecDeque<usize> = indegree_work
        .iter()
        .enumerate()
        .filter_map(|(idx, deg)| (*deg == 0).then_some(idx))
        .collect();
    let mut topo = Vec::with_capacity(node_count);
    while let Some(node_idx) = queue.pop_front() {
        topo.push(node_idx);
        for &edge_idx in &outgoing[node_idx] {
            let to_idx = edges_data[edge_idx].to_idx;
            if indegree_work[to_idx] > 0 {
                indegree_work[to_idx] -= 1;
                if indegree_work[to_idx] == 0 {
                    queue.push_back(to_idx);
                }
            }
        }
    }
    if topo.len() == node_count {
        for &node_idx in &topo {
            for &edge_idx in &outgoing[node_idx] {
                let to_idx = edges_data[edge_idx].to_idx;
                ranks[to_idx] = ranks[to_idx].max(ranks[node_idx] + 1);
            }
        }
    }

    let max_rank = ranks.iter().copied().max().unwrap_or(0);
    let num_ranks = max_rank + 1;
    let gap_x = if num_ranks > 1 {
        ((SANKEY_WIDTH - SANKEY_NODE_WIDTH * num_ranks as f32) / (num_ranks - 1) as f32).max(0.0)
    } else {
        0.0
    };

    let mut totals = vec![0.0f32; node_count];
    for idx in 0..node_count {
        let total = in_total[idx].max(out_total[idx]);
        totals[idx] = if total > 0.0 { total } else { 1.0 };
    }
    let max_total = totals.iter().copied().fold(0.0, f32::max).max(1.0);
    let scale = SANKEY_HEIGHT / max_total;

    let mut node_x = vec![0.0f32; node_count];
    let mut node_y = vec![0.0f32; node_count];
    let mut node_h = vec![0.0f32; node_count];
    for idx in 0..node_count {
        let rank = ranks[idx];
        node_x[idx] = rank as f32 * (SANKEY_NODE_WIDTH + gap_x);
        node_h[idx] = totals[idx] * scale;
    }

    let mut rank_nodes: Vec<Vec<usize>> = vec![Vec::new(); num_ranks];
    for idx in 0..node_count {
        rank_nodes[ranks[idx]].push(idx);
    }
    for nodes_in_rank in &mut rank_nodes {
        nodes_in_rank.sort_by(|a, b| {
            node_order_idx[*a]
                .cmp(&node_order_idx[*b])
                .then_with(|| node_ids[*a].cmp(&node_ids[*b]))
        });
    }

    let mut outbound_order = outgoing.clone();
    for edges in &mut outbound_order {
        edges.sort_by(|a, b| {
            let target_a = edges_data[*a].to_idx;
            let target_b = edges_data[*b].to_idx;
            ranks[target_b]
                .cmp(&ranks[target_a])
                .then_with(|| node_order_idx[target_a].cmp(&node_order_idx[target_b]))
                .then_with(|| node_ids[target_a].cmp(&node_ids[target_b]))
        });
    }

    let edge_thickness: Vec<f32> = edges_data.iter().map(|edge| edge.value * scale).collect();
    let mut link_top = vec![0.0f32; edges_data.len()];
    let mut outbound_offset = vec![0.0f32; edges_data.len()];
    let mut acc = vec![0.0f32; node_count];

    fn compute_link_tops(
        node_positions: &[f32],
        outbound_order: &[Vec<usize>],
        edge_thickness: &[f32],
        link_top: &mut [f32],
        outbound_offset: &mut [f32],
        acc: &mut [f32],
    ) {
        link_top.fill(0.0);
        outbound_offset.fill(0.0);
        acc.fill(0.0);
        for source_idx in 0..outbound_order.len() {
            for &edge_idx in &outbound_order[source_idx] {
                let offset = acc[source_idx];
                outbound_offset[edge_idx] = offset;
                link_top[edge_idx] = node_positions[source_idx] + offset;
                acc[source_idx] += edge_thickness[edge_idx];
            }
        }
    }

    for rank in 1..=max_rank {
        compute_link_tops(
            &node_y,
            &outbound_order,
            &edge_thickness,
            &mut link_top,
            &mut outbound_offset,
            &mut acc,
        );
        for &node_idx in &rank_nodes[rank] {
            let mut min_top = f32::INFINITY;
            for &edge_idx in &incoming[node_idx] {
                let from_idx = edges_data[edge_idx].from_idx;
                if ranks[from_idx] >= rank {
                    continue;
                }
                min_top = min_top.min(link_top[edge_idx]);
            }
            if !min_top.is_finite() {
                continue;
            }
            let max_y = (SANKEY_HEIGHT - node_h[node_idx]).max(0.0);
            node_y[node_idx] = min_top.clamp(0.0, max_y);
        }
    }
    compute_link_tops(
        &node_y,
        &outbound_order,
        &edge_thickness,
        &mut link_top,
        &mut outbound_offset,
        &mut acc,
    );

    let mut node_colors = Vec::with_capacity(node_count);
    for idx in 0..node_count {
        let default_color = SANKEY_PALETTE[idx % SANKEY_PALETTE.len()].to_string();
        let mut style = resolve_node_style(node_ids[idx].as_str(), graph);
        let color = style.fill.clone().unwrap_or(default_color);
        if style.fill.is_none() {
            style.fill = Some(color.clone());
        }
        if style.stroke.is_none() {
            style.stroke = Some("none".to_string());
        }
        if style.stroke_width.is_none() {
            style.stroke_width = Some(0.0);
        }
        node_colors.push((color, style));
    }

    let mut nodes = BTreeMap::new();
    let mut sankey_nodes = Vec::with_capacity(node_count);
    for idx in 0..node_count {
        let id = node_ids[idx].clone();
        let label = graph
            .nodes
            .get(&id)
            .map(|node| node.label.clone())
            .unwrap_or_else(|| id.clone());
        let (color, style) = &node_colors[idx];
        let label_block = measure_label(&label, theme, config);
        nodes.insert(
            id.clone(),
            NodeLayout {
                id: id.clone(),
                x: node_x[idx],
                y: node_y[idx],
                width: SANKEY_NODE_WIDTH,
                height: node_h[idx],
                label: label_block,
                shape: crate::mermaid_renderer::ir::NodeShape::Rectangle,
                style: style.clone(),
                link: graph.node_links.get(&id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
        sankey_nodes.push(SankeyNodeLayout {
            id: id.clone(),
            label,
            total: totals[idx],
            rank: ranks[idx],
            x: node_x[idx],
            y: node_y[idx],
            width: SANKEY_NODE_WIDTH,
            height: node_h[idx],
            color: color.clone(),
        });
    }

    let mut edges = Vec::with_capacity(edges_data.len());
    let mut sankey_links = Vec::with_capacity(edges_data.len());
    for (edge_idx, edge) in edges_data.iter().enumerate() {
        let from_id = node_ids[edge.from_idx].clone();
        let to_id = node_ids[edge.to_idx].clone();
        let thickness = edge_thickness[edge_idx];
        if thickness <= 0.0 {
            continue;
        }
        let start_x = node_x[edge.from_idx] + SANKEY_NODE_WIDTH;
        let end_x = node_x[edge.to_idx];
        let start_y = node_y[edge.from_idx] + outbound_offset[edge_idx] + thickness / 2.0;
        let inbound_offset = (link_top[edge_idx] - node_y[edge.to_idx]).max(0.0);
        let end_y = node_y[edge.to_idx] + inbound_offset + thickness / 2.0;
        let (color_start, _) = &node_colors[edge.from_idx];
        let (color_end, _) = &node_colors[edge.to_idx];
        let gradient_id = format!("sankey-grad-{edge_idx}");

        edges.push(EdgeLayout {
            from: from_id.clone(),
            to: to_id.clone(),
            label: None,
            start_label: None,
            end_label: None,
            points: vec![(start_x, start_y), (end_x, end_y)],
            directed: false,
            arrow_start: false,
            arrow_end: false,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
            override_style: crate::mermaid_renderer::ir::EdgeStyleOverride {
                stroke: Some(color_start.clone()),
                stroke_width: Some(thickness),
                dasharray: None,
                label_color: None,
            },
        });
        sankey_links.push(SankeyLinkLayout {
            source: from_id,
            target: to_id,
            value: edge.value,
            thickness,
            start: (start_x, start_y),
            end: (end_x, end_y),
            color_start: color_start.clone(),
            color_end: color_end.clone(),
            gradient_id,
        });
    }

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: Some(SankeyLayout {
            width: SANKEY_WIDTH,
            height: SANKEY_HEIGHT,
            node_width: SANKEY_NODE_WIDTH,
            nodes: sankey_nodes,
            links: sankey_links,
        }),
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width: SANKEY_WIDTH,
        height: SANKEY_HEIGHT,
    }
}

fn compute_architecture_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    const MARGIN: f32 = 24.0;
    const SERVICE_SIZE: f32 = 64.0;
    const SERVICE_GAP: f32 = 72.0;
    const GROUP_PAD_X: f32 = 28.0;
    const GROUP_PAD_TOP: f32 = 32.0;
    const GROUP_PAD_BOTTOM: f32 = 44.0;
    const GROUP_GAP_Y: f32 = 48.0;
    const GROUP_STROKE: &str = "hsl(240, 60%, 86.2745098039%)";
    const ICON_FILL: &str = "#087ebf";

    let mut nodes = BTreeMap::new();

    for node in graph.nodes.values() {
        let label = measure_label(&node.label, theme, config);
        let mut style = resolve_node_style(node.id.as_str(), graph);
        if style.fill.is_none() {
            style.fill = Some(ICON_FILL.to_string());
        }
        if style.stroke.is_none() {
            style.stroke = Some("none".to_string());
        }
        if style.stroke_width.is_none() {
            style.stroke_width = Some(0.0);
        }
        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width: SERVICE_SIZE,
                height: SERVICE_SIZE,
                label,
                shape: crate::mermaid_renderer::ir::NodeShape::Rectangle,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
    }

    let mut assigned: HashSet<String> = HashSet::new();
    let mut subgraphs = Vec::new();
    let mut current_y = MARGIN;

    for sub in &graph.subgraphs {
        let mut group_nodes: Vec<String> = sub
            .nodes
            .iter()
            .filter(|id| nodes.contains_key(*id))
            .cloned()
            .collect();
        if group_nodes.is_empty() {
            continue;
        }
        group_nodes.sort_by(|a, b| {
            let order_a = graph.node_order.get(a).copied().unwrap_or(usize::MAX);
            let order_b = graph.node_order.get(b).copied().unwrap_or(usize::MAX);
            order_a.cmp(&order_b).then_with(|| a.cmp(b))
        });
        assigned.extend(group_nodes.iter().cloned());

        let count = group_nodes.len() as f32;
        let gaps = (count - 1.0).max(0.0);
        let group_width = GROUP_PAD_X * 2.0 + SERVICE_SIZE * count + SERVICE_GAP * gaps;
        let group_height = GROUP_PAD_TOP + SERVICE_SIZE + GROUP_PAD_BOTTOM;
        let group_x = MARGIN;
        let group_y = current_y;

        let mut x_cursor = group_x + GROUP_PAD_X;
        for node_id in &group_nodes {
            if let Some(node) = nodes.get_mut(node_id) {
                node.x = x_cursor;
                node.y = group_y + GROUP_PAD_TOP;
            }
            x_cursor += SERVICE_SIZE + SERVICE_GAP;
        }

        let label_block = measure_label(&sub.label, theme, config);
        let mut style = resolve_subgraph_style(sub, graph);
        style.fill = Some("none".to_string());
        style.stroke = Some(GROUP_STROKE.to_string());
        style.stroke_width = Some(2.0);
        style.stroke_dasharray = Some("8".to_string());
        if style.text_color.is_none() {
            style.text_color = Some(theme.primary_text_color.clone());
        }

        subgraphs.push(SubgraphLayout {
            label: sub.label.clone(),
            label_block,
            nodes: group_nodes,
            x: group_x,
            y: group_y,
            width: group_width,
            height: group_height,
            style,
        });

        current_y += group_height + GROUP_GAP_Y;
    }

    let mut free_nodes: Vec<String> = nodes
        .keys()
        .filter(|id| !assigned.contains(*id))
        .cloned()
        .collect();
    free_nodes.sort_by(|a, b| {
        let order_a = graph.node_order.get(a).copied().unwrap_or(usize::MAX);
        let order_b = graph.node_order.get(b).copied().unwrap_or(usize::MAX);
        order_a.cmp(&order_b).then_with(|| a.cmp(b))
    });
    if !free_nodes.is_empty() {
        let row_y = current_y;
        let mut x_cursor = MARGIN + GROUP_PAD_X;
        for node_id in &free_nodes {
            if let Some(node) = nodes.get_mut(node_id) {
                node.x = x_cursor;
                node.y = row_y + GROUP_PAD_TOP;
            }
            x_cursor += SERVICE_SIZE + SERVICE_GAP;
        }
    }

    let mut edges = Vec::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let Some(from) = nodes.get(&edge.from) else {
            continue;
        };
        let Some(to) = nodes.get(&edge.to) else {
            continue;
        };
        let start_x = from.x + SERVICE_SIZE;
        let start_y = from.y + SERVICE_SIZE / 2.0;
        let end_x = to.x;
        let end_y = to.y + SERVICE_SIZE / 2.0;
        let mut override_style = resolve_edge_style(idx, graph);
        if override_style.stroke.is_none() {
            override_style.stroke = Some(theme.line_color.clone());
        }
        override_style.stroke_width = Some(override_style.stroke_width.unwrap_or(3.0));

        edges.push(EdgeLayout {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label: None,
            start_label: None,
            end_label: None,
            points: vec![(start_x, start_y), (end_x, end_y)],
            directed: true,
            arrow_start: false,
            arrow_end: true,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: edge.style,
            override_style,
        });
    }

    let (max_x, max_y) = bounds_with_edges(&nodes, &subgraphs, &edges);
    let width = (max_x + MARGIN).max(200.0);
    let height = (max_y + MARGIN).max(200.0);

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_radar_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    const WIDTH: f32 = 680.0;
    const HEIGHT: f32 = 680.0;
    const CENTER_X: f32 = WIDTH / 2.0;
    const CENTER_Y: f32 = HEIGHT / 2.0;
    const MAX_RADIUS: f32 = 290.0;
    const LEGEND_BOX_SIZE: f32 = 11.0;
    const LEGEND_GAP: f32 = 3.0;

    let legend_offset = MAX_RADIUS * 0.875;
    let legend_base_x = CENTER_X + legend_offset;
    let legend_base_y = CENTER_Y - legend_offset;
    let legend_row_height = theme.font_size + 6.0;

    let mut node_ids: Vec<String> = graph.nodes.keys().cloned().collect();
    node_ids.sort_by(|a, b| {
        let order_a = graph.node_order.get(a).copied().unwrap_or(usize::MAX);
        let order_b = graph.node_order.get(b).copied().unwrap_or(usize::MAX);
        order_a.cmp(&order_b).then_with(|| a.cmp(b))
    });

    let mut nodes = BTreeMap::new();
    for (idx, node_id) in node_ids.iter().enumerate() {
        let Some(node) = graph.nodes.get(node_id) else {
            continue;
        };
        let label = measure_label(&node.label, theme, config);
        let x = legend_base_x;
        let y = legend_base_y + idx as f32 * legend_row_height;
        let width = LEGEND_BOX_SIZE + LEGEND_GAP + label.width;
        let height = label.height.max(LEGEND_BOX_SIZE);
        let mut style = resolve_node_style(node.id.as_str(), graph);
        if style.stroke.is_none() {
            style.stroke = Some("none".to_string());
        }
        if style.stroke_width.is_none() {
            style.stroke_width = Some(0.0);
        }
        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x,
                y,
                width,
                height,
                label,
                shape: node.shape,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
    }

    Layout {
        kind: graph.kind,
        nodes,
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width: WIDTH,
        height: HEIGHT,
    }
}

fn compute_block_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let mut nodes = BTreeMap::new();
    for node in graph.nodes.values() {
        let label = measure_label(&node.label, theme, config);
        let (width, height) = shape_size(node.shape, &label, config, theme, graph.kind);
        let style = resolve_node_style(node.id.as_str(), graph);
        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width,
                height,
                label,
                shape: node.shape,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
    }

    let node_gap = (theme.font_size * 0.4).max(4.0);
    let column_gap = (theme.font_size * 0.45).max(6.0);
    let origin_x = 6.0;
    let origin_y = 6.0;

    let mut edges: Vec<EdgeLayout> = Vec::new();

    let Some(block) = graph.block.as_ref() else {
        let mut subgraphs = build_subgraph_layouts(graph, &nodes, theme, config);
        normalize_layout(&mut nodes, edges.as_mut_slice(), &mut subgraphs);
        let (max_x, max_y) = bounds_without_padding(&nodes, &subgraphs);
        return Layout {
            kind: graph.kind,
            nodes,
            edges,
            subgraphs,
            lifelines: Vec::new(),
            sequence_footboxes: Vec::new(),
            sequence_boxes: Vec::new(),
            sequence_frames: Vec::new(),
            sequence_notes: Vec::new(),
            sequence_activations: Vec::new(),
            sequence_numbers: Vec::new(),
            state_notes: Vec::new(),
            pie_slices: Vec::new(),
            pie_legend: Vec::new(),
            pie_center: (0.0, 0.0),
            pie_radius: 0.0,
            pie_title: None,
            quadrant: None,
            gantt: None,
            sankey: None,
            gitgraph: None,
            c4: None,
            xychart: None,
            timeline: None,
            journey: None,
            error: None,
            width: max_x + 6.0,
            height: max_y + 6.0,
        };
    };

    let columns = block.columns.unwrap_or_else(|| block.nodes.len().max(1));
    let mut column_widths = vec![0.0f32; columns];
    let mut column_x = vec![0.0f32; columns];
    let mut row_y = Vec::<f32>::new();

    let mut row = 0usize;
    let mut col = 0usize;
    let mut row_heights: Vec<f32> = vec![0.0];

    for node in &block.nodes {
        if col >= columns {
            col = 0;
            row += 1;
            row_heights.push(0.0);
        }
        let span = node.span.max(1).min(columns);
        if col + span > columns {
            col = 0;
            row += 1;
            row_heights.push(0.0);
        }
        if !node.is_space
            && let Some(layout) = nodes.get(&node.id)
        {
            let per_col = layout.width / span as f32;
            for i in 0..span {
                let idx = col + i;
                if idx < columns {
                    column_widths[idx] = column_widths[idx].max(per_col);
                }
            }
            row_heights[row] = row_heights[row].max(layout.height);
        }
        col += span;
    }

    column_x[0] = origin_x;
    for i in 1..columns {
        column_x[i] = column_x[i - 1] + column_widths[i - 1] + column_gap;
    }

    let mut y_cursor = origin_y;
    for h in &row_heights {
        row_y.push(y_cursor);
        y_cursor += *h + node_gap;
    }

    row = 0;
    col = 0;
    for node in &block.nodes {
        if col >= columns {
            col = 0;
            row += 1;
        }
        let span = node.span.max(1).min(columns);
        if col + span > columns {
            col = 0;
            row += 1;
        }
        if !node.is_space
            && let Some(layout) = nodes.get_mut(&node.id)
        {
            let start_x = column_x[col];
            let mut span_width = 0.0;
            for i in 0..span {
                let idx = col + i;
                if idx < columns {
                    span_width += column_widths[idx];
                    if i + 1 < span {
                        span_width += column_gap;
                    }
                }
            }
            let x = start_x + (span_width - layout.width) / 2.0;
            let y = row_y[row] + (row_heights[row] - layout.height) / 2.0;
            layout.x = x;
            layout.y = y;
        }
        col += span;
    }

    for edge in &graph.edges {
        let Some(from_layout) = nodes.get(&edge.from) else {
            continue;
        };
        let Some(to_layout) = nodes.get(&edge.to) else {
            continue;
        };
        let from_center = (
            from_layout.x + from_layout.width / 2.0,
            from_layout.y + from_layout.height / 2.0,
        );
        let to_center = (
            to_layout.x + to_layout.width / 2.0,
            to_layout.y + to_layout.height / 2.0,
        );
        let label = edge.label.as_ref().map(|l| measure_label(l, theme, config));
        let start_label = edge
            .start_label
            .as_ref()
            .map(|l| measure_label(l, theme, config));
        let end_label = edge
            .end_label
            .as_ref()
            .map(|l| measure_label(l, theme, config));
        let mut override_style = resolve_edge_style(edges.len(), graph);
        if edge.style == crate::mermaid_renderer::ir::EdgeStyle::Dotted
            && override_style.dasharray.is_none()
        {
            override_style.dasharray = Some("3 3".to_string());
        }
        edges.push(EdgeLayout {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label,
            start_label,
            end_label,
            points: vec![from_center, to_center],
            directed: edge.directed,
            arrow_start: edge.arrow_start,
            arrow_end: edge.arrow_end,
            arrow_start_kind: edge.arrow_start_kind,
            arrow_end_kind: edge.arrow_end_kind,
            start_decoration: edge.start_decoration,
            end_decoration: edge.end_decoration,
            style: edge.style,
            override_style,
        });
    }

    let mut subgraphs = build_subgraph_layouts(graph, &nodes, theme, config);
    normalize_layout(&mut nodes, edges.as_mut_slice(), &mut subgraphs);

    let (max_x, max_y) = bounds_with_edges(&nodes, &subgraphs, &edges);
    let width = max_x + 6.0;
    let height = max_y + 6.0;

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_kanban_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    if !graph.edges.is_empty() {
        return compute_flowchart_layout(graph, theme, config);
    }

    let mut nodes = BTreeMap::new();
    for node in graph.nodes.values() {
        let label = measure_label(&node.label, theme, config);
        let (width, height) = shape_size(node.shape, &label, config, theme, graph.kind);
        let mut style = resolve_node_style(node.id.as_str(), graph);
        if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement {
            if style.fill.is_none() {
                style.fill = Some(config.requirement.fill.clone());
            }
            if style.stroke.is_none() {
                style.stroke = Some(config.requirement.box_stroke.clone());
            }
            if style.stroke_width.is_none() {
                style.stroke_width = Some(config.requirement.box_stroke_width);
            }
            if style.text_color.is_none() {
                style.text_color = Some(config.requirement.label_color.clone());
            }
        }
        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width,
                height,
                label,
                shape: node.shape,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
    }

    let node_gap = (theme.font_size * 0.45).max(4.0);
    let column_gap = (theme.font_size * 0.3).max(3.0);
    let origin_x = 6.0;
    let origin_y = 6.0;
    let mut column_x = origin_x;
    let mut assigned: HashSet<String> = HashSet::new();

    for sub in &graph.subgraphs {
        let column_nodes: Vec<String> = sub
            .nodes
            .iter()
            .filter(|id| nodes.contains_key(*id))
            .cloned()
            .collect();
        if column_nodes.is_empty() {
            continue;
        }
        assigned.extend(column_nodes.iter().cloned());

        let label_empty = sub.label.trim().is_empty();
        let mut label_block = measure_label(&sub.label, theme, config);
        if label_empty {
            label_block.width = 0.0;
            label_block.height = 0.0;
        }
        let (pad_x, _pad_y, top_padding) =
            subgraph_padding_from_label(graph, sub, theme, &label_block);

        let max_node_width = column_nodes
            .iter()
            .filter_map(|id| nodes.get(id).map(|n| n.width))
            .fold(0.0_f32, f32::max);
        let inner_width = max_node_width.max(label_block.width);
        let column_width = inner_width + pad_x * 2.0;

        let mut y_cursor = origin_y + top_padding;
        let last_idx = column_nodes.len().saturating_sub(1);
        for (idx, node_id) in column_nodes.iter().enumerate() {
            if let Some(node) = nodes.get_mut(node_id) {
                let x = column_x + pad_x + (inner_width - node.width) / 2.0;
                node.x = x;
                node.y = y_cursor;
                y_cursor += node.height;
                if idx < last_idx {
                    y_cursor += node_gap;
                }
            }
        }

        column_x += column_width + column_gap;
    }

    let mut free_x = column_x;
    for node in nodes.values_mut() {
        if assigned.contains(&node.id) {
            continue;
        }
        node.x = free_x;
        node.y = origin_y;
        free_x += node.width + column_gap;
    }

    let mut edges: Vec<EdgeLayout> = Vec::new();
    let mut subgraphs = build_subgraph_layouts(graph, &nodes, theme, config);
    normalize_layout(&mut nodes, edges.as_mut_slice(), &mut subgraphs);

    let (max_x, max_y) = bounds_without_padding(&nodes, &subgraphs);
    let width = max_x + 6.0;
    let height = max_y + 6.0;

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_xychart_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let data = &graph.xychart;
    let font_size = theme.font_size;
    let padding = 40.0;
    let y_axis_width = 60.0;
    let x_axis_height = 40.0;
    let title_height = if data.title.is_some() { 30.0 } else { 0.0 };

    let plot_width = 400.0;
    let plot_height = 250.0;

    let width = padding * 2.0 + y_axis_width + plot_width;
    let height = padding * 2.0 + title_height + plot_height + x_axis_height;

    let plot_x = padding + y_axis_width;
    let plot_y = padding + title_height;

    // Find min/max values
    let all_values: Vec<f32> = data
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .collect();
    let min_val = data
        .y_axis_min
        .unwrap_or_else(|| all_values.iter().copied().fold(0.0_f32, f32::min).min(0.0));
    let max_val = data
        .y_axis_max
        .unwrap_or_else(|| all_values.iter().copied().fold(0.0_f32, f32::max));
    let range = (max_val - min_val).max(1.0);

    // Number of categories
    let num_categories = data
        .x_axis_categories
        .len()
        .max(
            data.series
                .iter()
                .map(|s| s.values.len())
                .max()
                .unwrap_or(0),
        )
        .max(1);

    let bar_group_width = plot_width / num_categories as f32;
    let bar_padding = bar_group_width * 0.1;

    // Count bar series for width calculation
    let bar_count = data
        .series
        .iter()
        .filter(|s| s.kind == crate::mermaid_renderer::ir::XYSeriesKind::Bar)
        .count()
        .max(1);
    let bar_width = (bar_group_width - bar_padding * 2.0) / bar_count as f32;

    let colors = [
        "#4e79a7".to_string(),
        "#f28e2c".to_string(),
        "#e15759".to_string(),
        "#76b7b2".to_string(),
        "#59a14f".to_string(),
        "#edc949".to_string(),
        "#af7aa1".to_string(),
        "#ff9da7".to_string(),
    ];

    let mut bars = Vec::new();
    let mut lines = Vec::new();
    let mut bar_series_idx = 0;

    for (series_idx, series) in data.series.iter().enumerate() {
        let color = colors
            .get(series_idx % colors.len())
            .cloned()
            .unwrap_or_else(|| "#333".to_string());

        match series.kind {
            crate::mermaid_renderer::ir::XYSeriesKind::Bar => {
                for (i, &value) in series.values.iter().enumerate() {
                    let bar_height = ((value - min_val) / range) * plot_height;
                    let x = plot_x
                        + i as f32 * bar_group_width
                        + bar_padding
                        + bar_series_idx as f32 * bar_width;
                    let y = plot_y + plot_height - bar_height;

                    bars.push(XYChartBarLayout {
                        x,
                        y,
                        width: bar_width,
                        height: bar_height,
                        value,
                        color: color.clone(),
                    });
                }
                bar_series_idx += 1;
            }
            crate::mermaid_renderer::ir::XYSeriesKind::Line => {
                let points: Vec<(f32, f32)> = series
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, &value)| {
                        let x = plot_x + i as f32 * bar_group_width + bar_group_width / 2.0;
                        let y = plot_y + plot_height - ((value - min_val) / range) * plot_height;
                        (x, y)
                    })
                    .collect();

                lines.push(XYChartLineLayout { points, color });
            }
        }
    }

    // X-axis categories
    let x_axis_categories: Vec<(String, f32)> = data
        .x_axis_categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let x = plot_x + i as f32 * bar_group_width + bar_group_width / 2.0;
            (cat.clone(), x)
        })
        .collect();

    // Y-axis ticks
    let num_ticks = 5;
    let y_axis_ticks: Vec<(String, f32)> = (0..=num_ticks)
        .map(|i| {
            let value = min_val + (i as f32 / num_ticks as f32) * range;
            let y = plot_y + plot_height - (i as f32 / num_ticks as f32) * plot_height;
            (format!("{:.0}", value), y)
        })
        .collect();

    let title = data.title.as_ref().map(|t| measure_label(t, theme, config));
    let x_axis_label = data
        .x_axis_label
        .as_ref()
        .map(|l| measure_label(l, theme, config));
    let y_axis_label = data
        .y_axis_label
        .as_ref()
        .map(|l| measure_label(l, theme, config));

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: Some(XYChartLayout {
            title,
            title_y: padding + font_size,
            x_axis_label,
            x_axis_label_y: plot_y + plot_height + x_axis_height - 10.0,
            y_axis_label,
            y_axis_label_x: padding,
            x_axis_categories,
            y_axis_ticks,
            bars,
            lines,
            plot_x,
            plot_y,
            plot_width,
            plot_height,
            width,
            height,
        }),
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_timeline_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let data = &graph.timeline;
    let font_size = theme.font_size;
    let padding = 30.0;
    let event_width = 120.0;
    let event_height = 80.0;
    let event_spacing = 40.0;
    let title_height = if data.title.is_some() { 40.0 } else { 0.0 };
    let line_y = padding + title_height + 60.0;

    let num_events = data.events.len().max(1);
    let total_events_width =
        num_events as f32 * event_width + (num_events - 1).max(0) as f32 * event_spacing;

    let width = padding * 2.0 + total_events_width;
    let height = padding * 2.0 + title_height + event_height + 100.0;

    let title = data.title.as_ref().map(|t| measure_label(t, theme, config));

    let events: Vec<TimelineEventLayout> = data
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let x = padding + i as f32 * (event_width + event_spacing);
            let y = line_y + 30.0;

            let time_block = measure_label(&event.time, theme, config);
            let event_blocks: Vec<TextBlock> = event
                .events
                .iter()
                .map(|e| measure_label(e, theme, config))
                .collect();

            TimelineEventLayout {
                time: time_block,
                events: event_blocks,
                x,
                y,
                width: event_width,
                height: event_height,
                circle_y: line_y,
            }
        })
        .collect();

    let line_start_x = padding;
    let line_end_x = width - padding;

    // Sections (simplified - just record them)
    let sections: Vec<TimelineSectionLayout> = data
        .sections
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let label = measure_label(section, theme, config);
            TimelineSectionLayout {
                label,
                x: padding + i as f32 * 200.0,
                y: padding,
                width: 180.0,
                height: 30.0,
            }
        })
        .collect();

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: Some(TimelineLayout {
            title,
            title_y: padding + font_size,
            events,
            sections,
            line_y,
            line_start_x,
            line_end_x,
            width,
            height,
        }),
        journey: None,
        error: None,
        width,
        height,
    }
}

fn parse_journey_task_label(label: &str) -> (String, Vec<String>) {
    let mut lines = split_lines(label);
    if lines.is_empty() {
        return (String::new(), Vec::new());
    }
    let title = lines.remove(0).trim().to_string();
    let mut actors = Vec::new();
    for line in lines {
        for part in line.split(',') {
            let actor = part.trim();
            if !actor.is_empty() {
                actors.push(actor.to_string());
            }
        }
    }
    (title, actors)
}

fn journey_score_color(score: f32) -> String {
    let clamped = score.clamp(1.0, 5.0);
    let t = (clamped - 1.0) / 4.0;
    let start = (248.0, 113.0, 113.0);
    let end = (74.0, 222.0, 128.0);
    let r = (start.0 + (end.0 - start.0) * t).round() as i32;
    let g = (start.1 + (end.1 - start.1) * t).round() as i32;
    let b = (start.2 + (end.2 - start.2) * t).round() as i32;
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

fn compute_journey_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let mut section_defs: Vec<(String, Vec<String>)> = Vec::new();
    let mut assigned: HashSet<String> = HashSet::new();
    if graph.subgraphs.is_empty() {
        let mut ordered: Vec<String> = graph.nodes.keys().cloned().collect();
        ordered.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));
        section_defs.push((String::new(), ordered));
    } else {
        for sub in &graph.subgraphs {
            let mut nodes = Vec::new();
            for id in &sub.nodes {
                if graph.nodes.contains_key(id) {
                    nodes.push(id.clone());
                    assigned.insert(id.clone());
                }
            }
            section_defs.push((sub.label.clone(), nodes));
        }
        let mut extras: Vec<String> = graph
            .nodes
            .keys()
            .filter(|id| !assigned.contains(*id))
            .cloned()
            .collect();
        if !extras.is_empty() {
            extras.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));
            section_defs.push(("Other".to_string(), extras));
        }
    }

    struct TaskData {
        id: String,
        label: TextBlock,
        score: Option<f32>,
        actors: Vec<String>,
        section_idx: usize,
        order_idx: usize,
    }

    let mut tasks_data: Vec<TaskData> = Vec::new();
    let mut section_ranges: Vec<(usize, usize)> = Vec::new();
    let mut order_idx = 0usize;
    for (section_idx, (_label, nodes)) in section_defs.iter().enumerate() {
        let start_idx = order_idx;
        for node_id in nodes {
            if let Some(node) = graph.nodes.get(node_id) {
                let (title, actors) = parse_journey_task_label(&node.label);
                let title_text = if title.is_empty() {
                    node.label.clone()
                } else {
                    title
                };
                let label = measure_label(&title_text, theme, config);
                tasks_data.push(TaskData {
                    id: node_id.clone(),
                    label,
                    score: node.value,
                    actors,
                    section_idx,
                    order_idx,
                });
                order_idx += 1;
            }
        }
        let end_idx = order_idx.saturating_sub(1);
        section_ranges.push((start_idx, end_idx));
    }

    let mut actor_order: Vec<String> = Vec::new();
    let mut actor_set: HashSet<String> = HashSet::new();
    for task in &tasks_data {
        for actor in &task.actors {
            if actor_set.insert(actor.clone()) {
                actor_order.push(actor.clone());
            }
        }
    }

    let mut max_label_w = theme.font_size * 4.0;
    let mut max_label_h = theme.font_size * config.label_line_height;
    for task in &tasks_data {
        max_label_w = max_label_w.max(task.label.width);
        max_label_h = max_label_h.max(task.label.height);
    }

    let margin_x = theme.font_size * 2.0;
    let margin_y = theme.font_size * 2.0;
    let task_gap_x = theme.font_size * 1.6;
    let section_gap_y = theme.font_size * 1.8;
    let header_height = theme.font_size * 1.6;
    let card_gap_y = theme.font_size * 0.6;
    let score_radius = (theme.font_size * 0.55).max(6.0);
    let actor_radius = (theme.font_size * 0.35).max(4.0);
    let actor_gap = theme.font_size * 0.5;
    let task_pad_x = theme.font_size * 0.9;
    let task_pad_y = theme.font_size * 0.6;

    let task_width = (max_label_w + task_pad_x * 2.0).max(theme.font_size * 6.0);
    let task_height = (max_label_h + task_pad_y * 2.0).max(theme.font_size * 2.4);

    let title_block = graph
        .journey_title
        .as_ref()
        .map(|title| measure_label(title, theme, config));
    let mut cursor_y = margin_y;
    let title_y = if let Some(ref title) = title_block {
        let y = cursor_y + title.height / 2.0;
        cursor_y += title.height + theme.font_size * 0.6;
        y
    } else {
        0.0
    };

    let mut actors = Vec::new();
    let mut actor_label_y = 0.0;
    if !actor_order.is_empty() {
        let mut x = margin_x;
        let legend_y = cursor_y + actor_radius;
        actor_label_y = legend_y + theme.font_size * 0.35;
        for (idx, actor) in actor_order.iter().enumerate() {
            let label = measure_label(actor, theme, config);
            let color = theme.git_colors[idx % theme.git_colors.len()].clone();
            actors.push(JourneyActorLayout {
                name: actor.clone(),
                color: color.clone(),
                x: x + actor_radius,
                y: legend_y,
                radius: actor_radius,
            });
            x += actor_radius * 2.0 + actor_gap + label.width + theme.font_size * 0.8;
        }
        cursor_y += actor_radius * 2.0 + theme.font_size * 0.8;
    }

    let content_y = cursor_y;
    let has_actor_rows = tasks_data.iter().any(|task| !task.actors.is_empty());
    let actor_row_height = if has_actor_rows {
        actor_radius * 2.0
    } else {
        0.0
    };
    let actor_row_gap = if has_actor_rows {
        theme.font_size * 0.4
    } else {
        0.0
    };
    let row_height = header_height
        + score_radius * 2.0
        + card_gap_y
        + task_height
        + actor_row_gap
        + actor_row_height
        + theme.font_size * 0.6;

    let content_x = margin_x;
    let total_tasks = tasks_data.len();
    let task_area_width = if total_tasks > 0 {
        total_tasks as f32 * task_width + (total_tasks.saturating_sub(1)) as f32 * task_gap_x
    } else {
        0.0
    };

    let mut tasks = Vec::new();
    for task in &tasks_data {
        let row_top = content_y + task.section_idx as f32 * (row_height + section_gap_y);
        let card_y = row_top + header_height + score_radius * 2.0 + card_gap_y;
        let score_y = row_top + header_height + score_radius;
        let actor_y = if has_actor_rows {
            Some(card_y + task_height + actor_row_gap + actor_radius)
        } else {
            None
        };
        let x = content_x + task.order_idx as f32 * (task_width + task_gap_x);
        let score_color = task
            .score
            .map(journey_score_color)
            .unwrap_or_else(|| theme.secondary_color.clone());
        tasks.push(JourneyTaskLayout {
            id: task.id.clone(),
            label: task.label.clone(),
            x,
            y: card_y,
            width: task_width,
            height: task_height,
            score: task.score,
            score_color,
            score_y,
            actors: task.actors.clone(),
            actor_y,
            section_idx: task.section_idx,
        });
    }

    let mut sections = Vec::new();
    let section_pad_x = theme.font_size * 0.6;
    for (section_idx, (label, _nodes)) in section_defs.iter().enumerate() {
        let (start_idx, end_idx) = section_ranges.get(section_idx).copied().unwrap_or((0, 0));
        if start_idx > end_idx || total_tasks == 0 {
            continue;
        }
        let row_top = content_y + section_idx as f32 * (row_height + section_gap_y);
        let x = content_x + start_idx as f32 * (task_width + task_gap_x) - section_pad_x;
        let span = end_idx.saturating_sub(start_idx) + 1;
        let width = span as f32 * task_width
            + (span.saturating_sub(1)) as f32 * task_gap_x
            + section_pad_x * 2.0;
        let label_block = measure_label(label, theme, config);
        let color = theme.git_colors[section_idx % theme.git_colors.len()].clone();
        sections.push(JourneySectionLayout {
            label: label_block,
            x,
            y: row_top,
            width,
            height: header_height,
            color,
        });
    }

    let baseline = if total_tasks > 0 {
        let rows = section_defs.len();
        let total_rows_height = if rows > 0 {
            rows as f32 * row_height + (rows.saturating_sub(1)) as f32 * section_gap_y
        } else {
            0.0
        };
        let y = content_y + total_rows_height + theme.font_size * 0.5;
        Some((content_x, y, content_x + task_area_width))
    } else {
        None
    };

    let width = (content_x + task_area_width + margin_x).max(1.0);
    let height = baseline
        .map(|(_, y, _)| y + theme.font_size * 1.6)
        .unwrap_or(content_y + theme.font_size * 4.0)
        .max(1.0);

    Layout {
        kind: graph.kind,
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: Some(JourneyLayout {
            title: title_block,
            title_y,
            actors,
            actor_label_y,
            tasks,
            sections,
            baseline,
            score_radius,
            actor_radius,
            actor_gap,
            card_gap_y,
            width,
            height,
        }),
        error: None,
        width,
        height,
    }
}

fn adaptive_spacing_for_nodes(
    nodes: &BTreeMap<String, NodeLayout>,
    min_spacing: f32,
    max_spacing: f32,
) -> f32 {
    let mut total = 0.0f32;
    let mut count = 0usize;
    for node in nodes.values() {
        if node.hidden || node.anchor_subgraph.is_some() {
            continue;
        }
        total += (node.width + node.height) * 0.5;
        count += 1;
    }
    if count == 0 {
        return max_spacing;
    }
    let avg = total / count as f32;
    let target = (avg * 0.5).max(min_spacing);
    target.min(max_spacing)
}

fn compute_flowchart_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let mut effective_config = config.clone();
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement {
        effective_config.max_label_width_chars = effective_config.max_label_width_chars.max(32);
    }
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart {
        let node_count = graph.nodes.len();
        let edge_count = graph.edges.len() as f32;
        let density = if node_count > 0 {
            edge_count / node_count as f32
        } else {
            0.0
        };
        let auto = &config.flowchart.auto_spacing;
        if auto.enabled && !auto.buckets.is_empty() {
            let mut scale = auto.buckets[0].scale;
            for bucket in &auto.buckets {
                if node_count >= bucket.min_nodes {
                    scale = bucket.scale;
                }
            }
            if density > auto.density_threshold {
                scale = scale.max(auto.dense_scale_floor);
            }
            effective_config.node_spacing =
                (effective_config.node_spacing * scale).max(auto.min_spacing);
            effective_config.rank_spacing =
                (effective_config.rank_spacing * scale).max(auto.min_spacing);
        }
    }
    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    let tiny_graph = graph.subgraphs.is_empty() && node_count <= 4 && edge_count <= 4;
    if tiny_graph {
        effective_config.flowchart.order_passes = 1;
        effective_config.flowchart.routing.enable_grid_router = false;
        effective_config.flowchart.routing.snap_ports_to_grid = false;
    }
    let mut nodes = BTreeMap::new();
    let measure_font_size = theme.font_size;
    let mut label_config = effective_config.clone();
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Class {
        label_config.label_line_height = label_config.class_label_line_height();
    }
    let mut state_marker_ids: Vec<String> = Vec::new();
    let mut state_height_total = 0.0f32;
    let mut state_height_count = 0usize;

    for node in graph.nodes.values() {
        let label = measure_label_with_font_size(
            &node.label,
            measure_font_size,
            &label_config,
            true,
            theme.font_family.as_str(),
        );
        let label_empty = label.lines.len() == 1 && label.lines[0].trim().is_empty();
        let (mut width, mut height) =
            shape_size(node.shape, &label, &effective_config, theme, graph.kind);
        if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State
            && label_empty
            && matches!(
                node.shape,
                crate::mermaid_renderer::ir::NodeShape::Circle
                    | crate::mermaid_renderer::ir::NodeShape::DoubleCircle
            )
        {
            let size = (theme.font_size * 0.75).max(10.0);
            width = size;
            height = size;
            state_marker_ids.push(node.id.clone());
        } else if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State {
            state_height_total += height;
            state_height_count += 1;
        }
        let style = resolve_node_style(node.id.as_str(), graph);
        nodes.insert(
            node.id.clone(),
            NodeLayout {
                id: node.id.clone(),
                x: 0.0,
                y: 0.0,
                width,
                height,
                label,
                shape: node.shape,
                style,
                link: graph.node_links.get(&node.id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
    }

    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State && !state_marker_ids.is_empty()
    {
        let avg_height = if state_height_count > 0 {
            state_height_total / state_height_count as f32
        } else {
            theme.font_size * 2.4
        };
        let marker_size = (avg_height / 3.0).clamp(theme.font_size * 0.5, theme.font_size * 0.95);
        for id in state_marker_ids {
            if let Some(node) = nodes.get_mut(&id) {
                node.width = marker_size;
                node.height = marker_size;
            }
        }
    }

    let adaptive_node_spacing = adaptive_spacing_for_nodes(
        &nodes,
        effective_config.flowchart.auto_spacing.min_spacing,
        effective_config.node_spacing,
    );
    let adaptive_rank_spacing = adaptive_spacing_for_nodes(
        &nodes,
        effective_config.flowchart.auto_spacing.min_spacing,
        effective_config.rank_spacing,
    );
    if adaptive_node_spacing < effective_config.node_spacing {
        effective_config.node_spacing = adaptive_node_spacing;
    }
    if adaptive_rank_spacing < effective_config.rank_spacing {
        effective_config.rank_spacing = adaptive_rank_spacing;
    }

    let config = &effective_config;

    let anchor_ids = mark_subgraph_anchor_nodes_hidden(graph, &mut nodes);
    let mut anchor_info = apply_subgraph_anchor_sizes(graph, &mut nodes, theme, config);
    let mut anchored_subgraph_nodes: HashSet<String> = HashSet::new();
    for info in anchor_info.values() {
        if let Some(sub) = graph.subgraphs.get(info.sub_idx) {
            anchored_subgraph_nodes.extend(sub.nodes.iter().cloned());
        }
    }

    let anchored_indices: HashSet<usize> = anchor_info.values().map(|info| info.sub_idx).collect();
    let mut edge_redirects: HashMap<String, String> = HashMap::new();
    if !graph.subgraphs.is_empty() {
        for (idx, sub) in graph.subgraphs.iter().enumerate() {
            let Some(anchor_id) = subgraph_anchor_id(sub, &nodes) else {
                continue;
            };
            if anchored_indices.contains(&idx) {
                continue;
            }
            if let Some(anchor_child) = pick_subgraph_anchor_child(sub, graph, &anchor_ids)
                && anchor_child != anchor_id
            {
                edge_redirects.insert(anchor_id.to_string(), anchor_child);
            }
        }
    }

    let mut layout_edges: Vec<crate::mermaid_renderer::ir::Edge> =
        Vec::with_capacity(graph.edges.len());
    for edge in &graph.edges {
        let mut layout_edge = edge.clone();
        if let Some(new_from) = edge_redirects.get(&layout_edge.from) {
            layout_edge.from = new_from.clone();
        }
        if let Some(new_to) = edge_redirects.get(&layout_edge.to) {
            layout_edge.to = new_to.clone();
        }
        layout_edges.push(layout_edge);
    }

    let mut layout_node_ids: Vec<String> = graph.nodes.keys().cloned().collect();
    layout_node_ids.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));
    if !anchored_subgraph_nodes.is_empty() {
        layout_node_ids.retain(|id| !anchored_subgraph_nodes.contains(id));
    }
    let mut layout_set: HashSet<String> = layout_node_ids.iter().cloned().collect();

    if anchor_info.is_empty() {
        anchor_info = apply_subgraph_anchor_sizes(graph, &mut nodes, theme, config);
        anchored_subgraph_nodes.clear();
        for info in anchor_info.values() {
            if let Some(sub) = graph.subgraphs.get(info.sub_idx) {
                anchored_subgraph_nodes.extend(sub.nodes.iter().cloned());
            }
        }
        if !anchored_subgraph_nodes.is_empty() {
            layout_node_ids.retain(|id| !anchored_subgraph_nodes.contains(id));
        }
        layout_set = layout_node_ids.iter().cloned().collect();
    }
    assign_positions_manual(
        graph,
        &layout_node_ids,
        &layout_set,
        &mut nodes,
        config,
        &layout_edges,
    );

    if !graph.subgraphs.is_empty() {
        if graph.kind != crate::mermaid_renderer::ir::DiagramKind::State {
            apply_subgraph_direction_overrides(graph, &mut nodes, config, &anchored_indices);
        }
        if !anchor_info.is_empty() {
            let _anchored_nodes =
                align_subgraphs_to_anchor_nodes(graph, &anchor_info, &mut nodes, config);
        }
        if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State && !anchor_info.is_empty()
        {
            apply_state_subgraph_layouts(graph, &mut nodes, config, &anchored_indices);
        }
        apply_orthogonal_region_bands(graph, &mut nodes, config);
        if graph.kind != crate::mermaid_renderer::ir::DiagramKind::State {
            apply_subgraph_bands(graph, &mut nodes, config);
        }
    }

    compress_linear_subgraphs(graph, &mut nodes, config);
    enforce_top_level_subgraph_gap(graph, &mut nodes, theme, config);

    // Separate overlapping sibling subgraphs
    separate_sibling_subgraphs(graph, &mut nodes, theme, config);
    align_disconnected_top_level_subgraphs(graph, &mut nodes);
    align_disconnected_components(graph, &mut nodes, config);
    apply_visual_objectives(graph, &layout_edges, &mut nodes, theme, &effective_config);

    let mut subgraphs = build_subgraph_layouts(graph, &nodes, theme, config);
    apply_subgraph_anchors(graph, &subgraphs, &mut nodes);
    let obstacles = build_obstacles(&nodes, &subgraphs, config);
    let label_obstacles = build_label_obstacles_for_routing(&nodes, &subgraphs);
    let routing_grid = if config.flowchart.routing.enable_grid_router && !tiny_graph {
        build_routing_grid(&obstacles, config)
    } else {
        None
    };
    let mut node_degrees: HashMap<String, usize> = HashMap::new();
    for edge in &graph.edges {
        *node_degrees.entry(edge.from.clone()).or_insert(0) += 1;
        *node_degrees.entry(edge.to.clone()).or_insert(0) += 1;
    }
    let mut side_loads: HashMap<String, [usize; 4]> = HashMap::new();
    let mut edge_ports: Vec<EdgePortInfo> = Vec::with_capacity(graph.edges.len());
    let mut port_candidates: HashMap<(String, EdgeSide), Vec<PortCandidate>> = HashMap::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let from_layout = nodes.get(&edge.from).expect("from node missing");
        let to_layout = nodes.get(&edge.to).expect("to node missing");
        let temp_from = from_layout.anchor_subgraph.and_then(|anchor_idx| {
            subgraphs
                .get(anchor_idx)
                .map(|sub| anchor_layout_for_edge(from_layout, sub, graph.direction, true))
        });
        let temp_to = to_layout.anchor_subgraph.and_then(|anchor_idx| {
            subgraphs
                .get(anchor_idx)
                .map(|sub| anchor_layout_for_edge(to_layout, sub, graph.direction, false))
        });
        let from = temp_from.as_ref().unwrap_or(from_layout);
        let to = temp_to.as_ref().unwrap_or(to_layout);
        let (start_side, end_side, _is_backward) = edge_sides_balanced(
            &edge.from,
            &edge.to,
            from,
            to,
            graph.direction,
            &node_degrees,
            &side_loads,
        );
        bump_side_load(&mut side_loads, &edge.from, start_side);
        bump_side_load(&mut side_loads, &edge.to, end_side);
        edge_ports.push(EdgePortInfo {
            start_side,
            end_side,
            start_offset: 0.0,
            end_offset: 0.0,
        });

        let from_anchor = anchor_point_for_node(from, start_side, 0.0);
        let to_anchor = anchor_point_for_node(to, end_side, 0.0);
        let start_other = if side_is_vertical(start_side) {
            to_anchor.1
        } else {
            to_anchor.0
        };
        let end_other = if side_is_vertical(end_side) {
            from_anchor.1
        } else {
            from_anchor.0
        };
        port_candidates
            .entry((edge.from.clone(), start_side))
            .or_default()
            .push(PortCandidate {
                edge_idx: idx,
                is_start: true,
                other_pos: start_other,
            });
        port_candidates
            .entry((edge.to.clone(), end_side))
            .or_default()
            .push(PortCandidate {
                edge_idx: idx,
                is_start: false,
                other_pos: end_other,
            });
    }
    let routing_cell = routing_cell_size(config);
    for ((node_id, side), candidates) in port_candidates {
        let Some(node) = nodes.get(&node_id) else {
            continue;
        };
        let mut min_other = f32::MAX;
        let mut max_other = f32::MIN;
        for candidate in &candidates {
            min_other = min_other.min(candidate.other_pos);
            max_other = max_other.max(candidate.other_pos);
        }
        let span = (max_other - min_other).max(0.0);
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by(|&a, &b| {
            candidates[a]
                .other_pos
                .partial_cmp(&candidates[b].other_pos)
                .unwrap_or(Ordering::Equal)
        });
        let node_len = if side_is_vertical(side) {
            node.height
        } else {
            node.width
        };
        let pad = (node_len * config.flowchart.port_pad_ratio)
            .min(config.flowchart.port_pad_max)
            .max(config.flowchart.port_pad_min);
        let usable = (node_len - 2.0 * pad).max(1.0);
        let min_sep = usable / (candidates.len() as f32 + 1.0);
        let span_ratio = if node_len > 1.0 { span / node_len } else { 1.0 };
        let position_weight = (1.0 / (1.0 + span_ratio)).clamp(0.25, 0.85);
        let rank_weight = 1.0 - position_weight;
        let desired: Vec<(usize, f32)> = order
            .iter()
            .enumerate()
            .map(|(rank, &idx)| {
                let candidate = &candidates[idx];
                let t_pos = if span <= 1.0 {
                    0.5
                } else {
                    (candidate.other_pos - min_other) / span
                };
                let t_rank = (rank as f32 + 0.5) / candidates.len() as f32;
                let t = t_pos * position_weight + t_rank * rank_weight;
                let pos = pad + t * usable;
                (idx, pos)
            })
            .collect();
        let mut assigned = vec![0.0; candidates.len()];
        let mut prev = pad;
        for (order_idx, (cand_idx, pos)) in desired.iter().enumerate() {
            let mut p = *pos;
            if order_idx == 0 {
                p = p.max(pad);
            } else {
                p = p.max(prev + min_sep);
            }
            assigned[*cand_idx] = p;
            prev = p;
        }
        let mut next = pad + usable;
        for (order_idx, (cand_idx, _pos)) in desired.iter().enumerate().rev() {
            let mut p = assigned[*cand_idx];
            if order_idx + 1 == desired.len() {
                p = p.min(next);
            } else {
                p = p.min(next - min_sep);
            }
            assigned[*cand_idx] = p;
            next = p;
        }
        for (rank, &cand_idx) in order.iter().enumerate() {
            let candidate = &candidates[cand_idx];
            let mut offset = assigned[cand_idx] - node_len / 2.0;
            if config.flowchart.routing.snap_ports_to_grid && routing_cell > 0.0 {
                offset = (offset / routing_cell).round() * routing_cell;
            }
            if config.flowchart.port_side_bias != 0.0 {
                offset += config.flowchart.port_side_bias
                    * (rank as f32 - (candidates.len() as f32 - 1.0) / 2.0);
            }
            if let Some(info) = edge_ports.get_mut(candidate.edge_idx) {
                if candidate.is_start {
                    info.start_offset = offset;
                } else {
                    info.end_offset = offset;
                }
            }
        }
    }
    let pair_counts = build_edge_pair_counts(&graph.edges);
    let mut pair_seen: HashMap<(String, String), usize> = HashMap::new();
    let mut pair_index: Vec<usize> = vec![0; graph.edges.len()];
    let edge_route_labels: Vec<Option<TextBlock>> = graph
        .edges
        .iter()
        .map(|edge| {
            edge.label.as_ref().map(|label| {
                let label_text =
                    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement {
                        requirement_edge_label_text(label, config)
                    } else {
                        label.clone()
                    };
                measure_label(&label_text, theme, config)
            })
        })
        .collect();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let key = edge_pair_key(edge);
        let seen = pair_seen.entry(key).or_insert(0usize);
        pair_index[idx] = *seen;
        *seen += 1;
    }

    let mut route_order: Vec<(u8, f32, f32, usize)> = Vec::with_capacity(graph.edges.len());
    for (idx, edge) in graph.edges.iter().enumerate() {
        let from_layout = nodes.get(&edge.from).expect("from node missing");
        let to_layout = nodes.get(&edge.to).expect("to node missing");
        let temp_from = from_layout.anchor_subgraph.and_then(|idx| {
            subgraphs
                .get(idx)
                .map(|sub| anchor_layout_for_edge(from_layout, sub, graph.direction, true))
        });
        let temp_to = to_layout.anchor_subgraph.and_then(|idx| {
            subgraphs
                .get(idx)
                .map(|sub| anchor_layout_for_edge(to_layout, sub, graph.direction, false))
        });
        let from = temp_from.as_ref().unwrap_or(from_layout);
        let to = temp_to.as_ref().unwrap_or(to_layout);
        let from_center = (from.x + from.width / 2.0, from.y + from.height / 2.0);
        let to_center = (to.x + to.width / 2.0, to.y + to.height / 2.0);
        let dx = to_center.0 - from_center.0;
        let dy = to_center.1 - from_center.1;
        let cross_axis = if is_horizontal(graph.direction) {
            dy.abs()
        } else {
            dx.abs()
        };
        let main_axis = if is_horizontal(graph.direction) {
            dx.abs()
        } else {
            dy.abs()
        };
        let (_, _, is_backward) = edge_sides(from, to, graph.direction);
        let is_secondary =
            edge.style == crate::mermaid_renderer::ir::EdgeStyle::Dotted || edge.label.is_some();
        let priority = if is_secondary {
            2u8
        } else if is_backward {
            1u8
        } else {
            0u8
        };
        route_order.push((priority, cross_axis, main_axis, idx));
    }
    let steep_count = route_order
        .iter()
        .filter(|(_, cross_axis, main_axis, _)| *cross_axis > *main_axis * 0.8)
        .count();
    let use_cross_axis_order = graph.edges.len() >= 10 && steep_count * 4 >= graph.edges.len();
    if use_cross_axis_order {
        route_order.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
                .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal))
                .then_with(|| a.3.cmp(&b.3))
        });
    } else {
        let use_priority_preorder = graph.edges.len() >= 10;
        route_order.sort_by(|a, b| {
            let len_a = a.1 * a.1 + a.2 * a.2;
            let len_b = b.1 * b.1 + b.2 * b.2;
            let by_length = len_b.partial_cmp(&len_a).unwrap_or(Ordering::Equal);
            if use_priority_preorder {
                a.0.cmp(&b.0)
                    .then_with(|| by_length)
                    .then_with(|| a.3.cmp(&b.3))
            } else {
                by_length.then_with(|| a.3.cmp(&b.3))
            }
        });
    }

    let mut routed_points: Vec<Vec<(f32, f32)>> = vec![Vec::new(); graph.edges.len()];
    let use_occupancy = !tiny_graph && graph.edges.len() > 2;
    let mut edge_occupancy = if use_occupancy {
        Some(EdgeOccupancy::new(config.node_spacing.max(16.0) * 0.6))
    } else {
        None
    };
    let mut route_label_obstacles = label_obstacles;
    let mut existing_segments: Vec<Segment> = Vec::new();
    for (_, _, _, idx) in &route_order {
        let edge = &graph.edges[*idx];
        let key = edge_pair_key(edge);
        let total = *pair_counts.get(&key).unwrap_or(&1) as f32;
        let idx_in_pair = pair_index[*idx] as f32;
        let base_offset = if total > 1.0 {
            (idx_in_pair - (total - 1.0) / 2.0) * (config.node_spacing * 0.35)
        } else {
            0.0
        };
        let from_layout = nodes.get(&edge.from).expect("from node missing");
        let to_layout = nodes.get(&edge.to).expect("to node missing");
        let temp_from = from_layout.anchor_subgraph.and_then(|idx| {
            subgraphs
                .get(idx)
                .map(|sub| anchor_layout_for_edge(from_layout, sub, graph.direction, true))
        });
        let temp_to = to_layout.anchor_subgraph.and_then(|idx| {
            subgraphs
                .get(idx)
                .map(|sub| anchor_layout_for_edge(to_layout, sub, graph.direction, false))
        });
        let from = temp_from.as_ref().unwrap_or(from_layout);
        let to = temp_to.as_ref().unwrap_or(to_layout);
        let port_info = edge_ports
            .get(*idx)
            .copied()
            .expect("edge port info missing");
        let route_ctx = RouteContext {
            from_id: &edge.from,
            to_id: &edge.to,
            from,
            to,
            direction: graph.direction,
            config,
            obstacles: &obstacles,
            label_obstacles: &route_label_obstacles,
            fast_route: tiny_graph,
            base_offset,
            start_side: port_info.start_side,
            end_side: port_info.end_side,
            start_offset: port_info.start_offset,
            end_offset: port_info.end_offset,
        };
        let points = route_edge_with_avoidance(
            &route_ctx,
            edge_occupancy.as_ref(),
            routing_grid.as_ref(),
            Some(&existing_segments),
        );
        if let Some(label) = edge_route_labels.get(*idx).and_then(|label| label.as_ref())
            && label.width > 0.0
            && label.height > 0.0
            && let Some((label_x, label_y)) = edge_label_anchor_from_points(&points)
        {
            route_label_obstacles.push(Obstacle {
                id: format!("edge-label:{}", idx),
                x: label_x - label.width / 2.0 - 6.0,
                y: label_y - label.height / 2.0 - 4.0,
                width: label.width + 12.0,
                height: label.height + 8.0,
                members: None,
            });
        }
        if let Some(occ) = edge_occupancy.as_mut() {
            occ.add_path(&points);
        }
        if points.len() >= 2 {
            for segment in points.windows(2) {
                existing_segments.push((segment[0], segment[1]));
            }
        }
        routed_points[*idx] = points;
    }

    let mut edges = Vec::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let label = edge.label.as_ref().map(|l| {
            let label_text = if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement
            {
                requirement_edge_label_text(l, config)
            } else {
                l.clone()
            };
            measure_label(&label_text, theme, config)
        });
        let start_label = edge.start_label.as_ref().map(|l| {
            let label_text = if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement
            {
                requirement_edge_label_text(l, config)
            } else {
                l.clone()
            };
            measure_label(&label_text, theme, config)
        });
        let end_label = edge.end_label.as_ref().map(|l| {
            let label_text = if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement
            {
                requirement_edge_label_text(l, config)
            } else {
                l.clone()
            };
            measure_label(&label_text, theme, config)
        });
        let mut override_style = resolve_edge_style(idx, graph);
        if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Requirement {
            if override_style.stroke.is_none() {
                override_style.stroke = Some(config.requirement.edge_stroke.clone());
            }
            override_style.stroke_width = Some(
                override_style
                    .stroke_width
                    .unwrap_or(config.requirement.edge_stroke_width),
            );
            if override_style.dasharray.is_none() {
                override_style.dasharray = Some(config.requirement.edge_dasharray.clone());
            }
            if override_style.label_color.is_none() {
                override_style.label_color = Some(config.requirement.edge_label_color.clone());
            }
        }
        edges.push(EdgeLayout {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label,
            start_label,
            end_label,
            points: routed_points[idx].clone(),
            directed: edge.directed,
            arrow_start: edge.arrow_start,
            arrow_end: edge.arrow_end,
            arrow_start_kind: edge.arrow_start_kind,
            arrow_end_kind: edge.arrow_end_kind,
            start_decoration: edge.start_decoration,
            end_decoration: edge.end_decoration,
            style: edge.style,
            override_style,
        });
    }

    if matches!(graph.direction, Direction::RightLeft | Direction::BottomTop) {
        apply_direction_mirror(graph.direction, &mut nodes, &mut edges, &mut subgraphs);
    }

    normalize_layout(&mut nodes, &mut edges, &mut subgraphs);
    let mut state_notes = Vec::new();
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State
        && !graph.state_notes.is_empty()
    {
        let note_pad_x = theme.font_size * 0.75;
        let note_pad_y = theme.font_size * 0.5;
        let note_gap = (theme.font_size * 0.9).max(10.0);
        for note in &graph.state_notes {
            let Some(target) = nodes.get(&note.target) else {
                continue;
            };
            let label = measure_label(&note.label, theme, config);
            let width = label.width + note_pad_x * 2.0;
            let height = label.height + note_pad_y * 2.0;
            let y = target.y + target.height / 2.0 - height / 2.0;
            let x = match note.position {
                crate::mermaid_renderer::ir::StateNotePosition::LeftOf => {
                    target.x - note_gap - width
                }
                crate::mermaid_renderer::ir::StateNotePosition::RightOf => {
                    target.x + target.width + note_gap
                }
            };
            state_notes.push(StateNoteLayout {
                x,
                y,
                width,
                height,
                label,
                position: note.position,
                target: note.target.clone(),
            });
        }
    }
    let (mut max_x, mut max_y) = bounds_with_edges(&nodes, &subgraphs, &edges);
    for note in &state_notes {
        max_x = max_x.max(note.x + note.width);
        max_y = max_y.max(note.y + note.height);
    }
    let width = max_x + 8.0;
    let height = max_y + 8.0;

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes,
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn compute_treemap_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let mut nodes: BTreeMap<String, NodeLayout> = BTreeMap::new();
    let edges = Vec::new();
    let subgraphs = Vec::new();

    let width = config.treemap.width.max(1.0);
    let height = config.treemap.height.max(1.0);
    let root_rect = TreemapRect::new(0.0, 0.0, width, height);

    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut parents: HashMap<String, String> = HashMap::new();
    for edge in &graph.edges {
        children
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        parents.insert(edge.to.clone(), edge.from.clone());
    }
    for list in children.values_mut() {
        list.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));
    }

    let mut roots: Vec<String> = graph
        .nodes
        .keys()
        .filter(|id| !parents.contains_key(*id))
        .cloned()
        .collect();
    roots.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));

    let mut weight_cache: HashMap<String, f32> = HashMap::new();
    if !roots.is_empty() {
        layout_treemap_nodes(
            &roots,
            root_rect,
            0,
            graph,
            &children,
            &mut weight_cache,
            &mut nodes,
            theme,
            config,
        );
    }

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines: Vec::new(),
        sequence_footboxes: Vec::new(),
        sequence_boxes: Vec::new(),
        sequence_frames: Vec::new(),
        sequence_notes: Vec::new(),
        sequence_activations: Vec::new(),
        sequence_numbers: Vec::new(),
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

#[derive(Debug, Clone, Copy)]
struct TreemapRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl TreemapRect {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    fn inset(self, padding: f32) -> Self {
        let pad = padding.max(0.0);
        let w = (self.w - pad * 2.0).max(0.0);
        let h = (self.h - pad * 2.0).max(0.0);
        Self {
            x: self.x + pad,
            y: self.y + pad,
            w,
            h,
        }
    }
}

fn layout_treemap_nodes(
    ids: &[String],
    rect: TreemapRect,
    depth: usize,
    graph: &Graph,
    children: &HashMap<String, Vec<String>>,
    weight_cache: &mut HashMap<String, f32>,
    nodes_out: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) {
    if ids.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let total_weight: f32 = ids
        .iter()
        .map(|id| treemap_weight(id, graph, children, weight_cache))
        .sum();
    if total_weight <= 0.0 {
        return;
    }
    let gap = config.treemap.gap.max(0.0);
    let horizontal = depth.is_multiple_of(2);
    let count = ids.len();
    let available = if horizontal {
        (rect.w - gap * (count.saturating_sub(1) as f32)).max(0.0)
    } else {
        (rect.h - gap * (count.saturating_sub(1) as f32)).max(0.0)
    };

    let mut offset = 0.0;
    for id in ids {
        let weight = treemap_weight(id, graph, children, weight_cache);
        let ratio = (weight / total_weight).max(0.0);
        let span = available * ratio;
        let node_rect = if horizontal {
            let x = rect.x + offset;
            offset += span + gap;
            TreemapRect::new(x, rect.y, span, rect.h)
        } else {
            let y = rect.y + offset;
            offset += span + gap;
            TreemapRect::new(rect.x, y, rect.w, span)
        };

        if let Some(node) = graph.nodes.get(id) {
            let mut style = resolve_node_style(id, graph);
            if style.fill.is_none() {
                style.fill = Some(treemap_depth_color(depth, theme));
            }
            if style.stroke.is_none() {
                style.stroke = Some(theme.primary_border_color.clone());
            }
            if style.stroke_width.is_none() {
                style.stroke_width = Some(1.0);
            }
            if style.text_color.is_none() {
                style.text_color = Some(theme.primary_text_color.clone());
            }

            let label = measure_label(&node.label, theme, config);
            let pad_x = config.treemap.label_padding_x;
            let pad_y = config.treemap.label_padding_y;
            let fits = label.width <= (node_rect.w - pad_x * 2.0).max(0.0)
                && label.height <= (node_rect.h - pad_y * 2.0).max(0.0);
            let area = node_rect.w * node_rect.h;
            let label = if fits && area >= config.treemap.min_label_area {
                label
            } else {
                TextBlock {
                    lines: vec![String::new()],
                    width: 0.0,
                    height: 0.0,
                }
            };

            nodes_out.insert(
                id.clone(),
                NodeLayout {
                    id: node.id.clone(),
                    x: node_rect.x,
                    y: node_rect.y,
                    width: node_rect.w,
                    height: node_rect.h,
                    label,
                    shape: crate::mermaid_renderer::ir::NodeShape::Rectangle,
                    style,
                    link: graph.node_links.get(id).cloned(),
                    anchor_subgraph: None,
                    hidden: false,
                },
            );
        }

        if let Some(children_ids) = children.get(id) {
            let child_rect = node_rect.inset(config.treemap.padding);
            if child_rect.w > 1.0 && child_rect.h > 1.0 {
                layout_treemap_nodes(
                    children_ids,
                    child_rect,
                    depth + 1,
                    graph,
                    children,
                    weight_cache,
                    nodes_out,
                    theme,
                    config,
                );
            }
        }
    }
}

fn treemap_weight(
    id: &str,
    graph: &Graph,
    children: &HashMap<String, Vec<String>>,
    cache: &mut HashMap<String, f32>,
) -> f32 {
    if let Some(value) = cache.get(id) {
        return *value;
    }
    let mut weight = graph
        .nodes
        .get(id)
        .and_then(|node| node.value)
        .unwrap_or(0.0);
    if weight <= 0.0
        && let Some(child_ids) = children.get(id)
    {
        weight = child_ids
            .iter()
            .map(|child| treemap_weight(child, graph, children, cache))
            .sum();
    }
    if weight <= 0.0 {
        weight = 1.0;
    }
    cache.insert(id.to_string(), weight);
    weight
}

fn treemap_depth_color(depth: usize, theme: &Theme) -> String {
    match depth % 3 {
        0 => theme.primary_color.clone(),
        1 => theme.secondary_color.clone(),
        _ => theme.tertiary_color.clone(),
    }
}

fn assign_positions_manual(
    graph: &Graph,
    layout_node_ids: &[String],
    layout_set: &HashSet<String>,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
    layout_edges: &[crate::mermaid_renderer::ir::Edge],
) {
    let layout_edges: Vec<crate::mermaid_renderer::ir::Edge> = layout_edges
        .iter()
        .filter(|edge| layout_set.contains(&edge.from) && layout_set.contains(&edge.to))
        .cloned()
        .collect();
    let rank_edges = rank_edges_for_manual_layout(graph, layout_node_ids, &layout_edges);
    let ranks = compute_ranks_subset(layout_node_ids, &rank_edges, &graph.node_order);
    let mut max_rank = 0usize;
    for rank in ranks.values() {
        max_rank = max_rank.max(*rank);
    }
    let mut rank_nodes: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for node_id in layout_node_ids {
        let rank = *ranks.get(node_id).unwrap_or(&0);
        if let Some(bucket) = rank_nodes.get_mut(rank) {
            bucket.push(node_id.clone());
        }
    }

    let mut expanded_edges: Vec<crate::mermaid_renderer::ir::Edge> = Vec::new();
    let mut order_map = graph.node_order.clone();
    let mut dummy_counter = 0usize;

    for edge in &layout_edges {
        let Some(&from_rank) = ranks.get(&edge.from) else {
            continue;
        };
        let Some(&to_rank) = ranks.get(&edge.to) else {
            continue;
        };
        if to_rank <= from_rank {
            continue;
        }
        let span = to_rank - from_rank;
        if span <= 1 {
            expanded_edges.push(edge.clone());
            continue;
        }
        let mut prev = edge.from.clone();
        for step in 1..span {
            let dummy_id = format!("__dummy_{}__", dummy_counter);
            dummy_counter += 1;
            let order_idx = order_map.len();
            order_map.insert(dummy_id.clone(), order_idx);
            if let Some(bucket) = rank_nodes.get_mut(from_rank + step) {
                bucket.push(dummy_id.clone());
            }
            expanded_edges.push(crate::mermaid_renderer::ir::Edge {
                from: prev.clone(),
                to: dummy_id.clone(),
                label: None,
                start_label: None,
                end_label: None,
                directed: true,
                arrow_start: false,
                arrow_end: false,
                arrow_start_kind: None,
                arrow_end_kind: None,
                start_decoration: None,
                end_decoration: None,
                style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
            });
            prev = dummy_id;
        }
        expanded_edges.push(crate::mermaid_renderer::ir::Edge {
            from: prev,
            to: edge.to.clone(),
            label: None,
            start_label: None,
            end_label: None,
            directed: true,
            arrow_start: false,
            arrow_end: false,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
        });
    }

    for bucket in &mut rank_nodes {
        bucket.sort_by_key(|id| order_map.get(id).copied().unwrap_or(usize::MAX));
    }
    order_rank_nodes(
        &mut rank_nodes,
        &expanded_edges,
        &order_map,
        config.flowchart.order_passes,
    );

    let mut main_cursor = 0.0;
    for bucket in &rank_nodes {
        let mut max_main: f32 = 0.0;
        for node_id in bucket {
            if let Some(node_layout) = nodes.get_mut(node_id) {
                if is_horizontal(graph.direction) {
                    node_layout.x = main_cursor;
                    max_main = max_main.max(node_layout.width);
                } else {
                    node_layout.y = main_cursor;
                    max_main = max_main.max(node_layout.height);
                }
            }
        }
        if max_main > 0.0 {
            main_cursor += max_main + config.rank_spacing;
        }
    }

    let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &layout_edges {
        incoming
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut cross_pos: HashMap<String, f32> = HashMap::new();
    for bucket in &rank_nodes {
        for (idx, node_id) in bucket.iter().enumerate() {
            if let Some(node) = nodes.get(node_id) {
                let center = if is_horizontal(graph.direction) {
                    node.y + node.height / 2.0
                } else {
                    node.x + node.width / 2.0
                };
                cross_pos.insert(node_id.clone(), center + idx as f32 * 0.01);
            }
        }
    }

    let mut place_rank = |rank_idx: usize,
                          use_incoming: bool,
                          nodes: &mut BTreeMap<String, NodeLayout>| {
        let bucket = &rank_nodes[rank_idx];
        if bucket.is_empty() {
            return;
        }
        let neighbors = if use_incoming { &incoming } else { &outgoing };
        let mut entries: Vec<(String, f32, f32, usize)> = Vec::new();
        for (idx, node_id) in bucket.iter().enumerate() {
            let Some(node) = nodes.get(node_id) else {
                continue;
            };
            let mut neighbor_centers: Vec<f32> = Vec::new();
            if let Some(list) = neighbors.get(node_id) {
                for neighbor_id in list {
                    if let Some(center) = cross_pos.get(neighbor_id) {
                        neighbor_centers.push(*center);
                    }
                }
            }
            let mut desired = if neighbor_centers.is_empty() {
                cross_pos.get(node_id).copied().unwrap_or(0.0)
            } else {
                neighbor_centers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let mid = neighbor_centers.len() / 2;
                if neighbor_centers.len() % 2 == 1 {
                    neighbor_centers[mid]
                } else {
                    (neighbor_centers[mid - 1] + neighbor_centers[mid]) * 0.5
                }
            };
            if let Some(current) = cross_pos.get(node_id) {
                if !neighbor_centers.is_empty() {
                    desired = desired * 0.85 + current * 0.15;
                } else {
                    desired = *current;
                }
            }
            let half = if is_horizontal(graph.direction) {
                node.height / 2.0
            } else {
                node.width / 2.0
            };
            entries.push((node_id.clone(), desired, half, idx));
        }
        entries.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.3.cmp(&b.3))
        });
        let desired_mean =
            entries.iter().map(|(_, d, _, _)| *d).sum::<f32>() / entries.len() as f32;
        let mut assigned: Vec<(String, f32, f32)> = Vec::new();
        let mut prev_center: Option<f32> = None;
        let mut prev_half = 0.0;
        for (node_id, desired, half, _idx) in entries {
            let center = if let Some(prev) = prev_center {
                let min_center = prev + prev_half + half + config.node_spacing;
                if desired < min_center {
                    min_center
                } else {
                    desired
                }
            } else {
                desired
            };
            assigned.push((node_id, center, half));
            prev_center = Some(center);
            prev_half = half;
        }
        let actual_mean = assigned.iter().map(|(_, c, _)| *c).sum::<f32>() / assigned.len() as f32;
        let delta = desired_mean - actual_mean;
        for (node_id, center, _half) in assigned {
            let center = center + delta;
            if let Some(node) = nodes.get_mut(&node_id) {
                if is_horizontal(graph.direction) {
                    node.y = center - node.height / 2.0;
                } else {
                    node.x = center - node.width / 2.0;
                }
            }
            cross_pos.insert(node_id, center);
        }
    };

    for _ in 0..config.flowchart.order_passes.max(1) {
        for rank_idx in 0..rank_nodes.len() {
            place_rank(rank_idx, true, nodes);
        }
        for rank_idx in (0..rank_nodes.len()).rev() {
            place_rank(rank_idx, false, nodes);
        }
    }
}

fn rank_edges_for_manual_layout(
    graph: &Graph,
    layout_node_ids: &[String],
    layout_edges: &[crate::mermaid_renderer::ir::Edge],
) -> Vec<crate::mermaid_renderer::ir::Edge> {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart || layout_edges.len() < 3 {
        return layout_edges.to_vec();
    }

    let primary: Vec<crate::mermaid_renderer::ir::Edge> = layout_edges
        .iter()
        .filter(|edge| edge.style != crate::mermaid_renderer::ir::EdgeStyle::Dotted)
        .cloned()
        .collect();
    if primary.is_empty() {
        return layout_edges.to_vec();
    }

    let mut covered: HashSet<&str> = HashSet::new();
    for edge in &primary {
        covered.insert(edge.from.as_str());
        covered.insert(edge.to.as_str());
    }
    let min_covered = layout_node_ids.len().div_ceil(2);
    if covered.len() >= min_covered {
        return primary;
    }

    layout_edges.to_vec()
}

fn compute_sequence_layout(graph: &Graph, theme: &Theme, config: &LayoutConfig) -> Layout {
    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();
    let subgraphs = Vec::new();

    let mut participants = graph.sequence_participants.clone();
    for id in graph.nodes.keys() {
        if !participants.contains(id) {
            participants.push(id.clone());
        }
    }

    let mut label_blocks: HashMap<String, TextBlock> = HashMap::new();
    let mut max_label_width: f32 = 0.0;
    let mut max_label_height: f32 = 0.0;
    for id in &participants {
        let node = graph.nodes.get(id).expect("participant missing");
        let label = measure_label(&node.label, theme, config);
        max_label_width = max_label_width.max(label.width);
        max_label_height = max_label_height.max(label.height);
        label_blocks.insert(id.clone(), label);
    }

    let actor_width = (max_label_width + theme.font_size * 2.1).max(130.0);
    let actor_height = (max_label_height + theme.font_size * 2.0).max(56.0);
    let actor_gap = (theme.font_size * 3.0).max(30.0);

    // Add consistent margins to center the diagram
    let margin = 8.0;
    let mut cursor_x = margin;
    for id in &participants {
        let node = graph.nodes.get(id).expect("participant missing");
        let label = label_blocks.get(id).cloned().unwrap_or_else(|| TextBlock {
            lines: vec![id.clone()],
            width: 0.0,
            height: 0.0,
        });
        nodes.insert(
            id.clone(),
            NodeLayout {
                id: id.clone(),
                x: cursor_x,
                y: margin,
                width: actor_width,
                height: actor_height,
                label,
                shape: node.shape,
                style: resolve_node_style(id.as_str(), graph),
                link: graph.node_links.get(id).cloned(),
                anchor_subgraph: None,
                hidden: false,
            },
        );
        cursor_x += actor_width + actor_gap;
    }

    let base_spacing = (theme.font_size * 2.1).max(18.0);
    let note_gap_y = (theme.font_size * 0.55).max(5.0);
    let note_gap_x = (theme.font_size * 0.65).max(7.0);
    let note_padding_x = (theme.font_size * 0.75).max(7.0);
    let note_padding_y = (theme.font_size * 0.45).max(4.0);
    let mut extra_before = vec![0.0; graph.edges.len()];
    let frame_end_pad = base_spacing * 0.25;
    for frame in &graph.sequence_frames {
        if frame.start_idx < extra_before.len() {
            extra_before[frame.start_idx] += base_spacing;
        }
        for section in frame.sections.iter().skip(1) {
            if section.start_idx < extra_before.len() {
                extra_before[section.start_idx] += base_spacing;
            }
        }
        if frame.end_idx < extra_before.len() {
            extra_before[frame.end_idx] += frame_end_pad;
        }
    }

    let mut notes_by_index = vec![Vec::new(); graph.edges.len().saturating_add(1)];
    for note in &graph.sequence_notes {
        let idx = note.index.min(graph.edges.len());
        notes_by_index[idx].push(note);
    }

    let mut message_cursor = margin + actor_height + theme.font_size * 2.2;
    let mut message_ys = Vec::new();
    let mut sequence_notes = Vec::new();
    for idx in 0..=graph.edges.len() {
        if let Some(bucket) = notes_by_index.get(idx) {
            for note in bucket {
                message_cursor += note_gap_y;
                let label = measure_label(&note.label, theme, config);
                let mut width = label.width + note_padding_x * 2.0;
                let height = label.height + note_padding_y * 2.0;
                let mut lifeline_xs = note
                    .participants
                    .iter()
                    .filter_map(|id| nodes.get(id))
                    .map(|node| node.x + node.width / 2.0)
                    .collect::<Vec<_>>();
                if lifeline_xs.is_empty() {
                    lifeline_xs.push(0.0);
                }
                let base_x = lifeline_xs[0];
                let min_x = lifeline_xs.iter().copied().fold(f32::INFINITY, f32::min);
                let max_x = lifeline_xs
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max);
                if note.position == crate::mermaid_renderer::ir::SequenceNotePosition::Over
                    && note.participants.len() > 1
                {
                    let span = (max_x - min_x).abs();
                    width = width.max(span + note_gap_x * 2.0);
                }
                let x = match note.position {
                    crate::mermaid_renderer::ir::SequenceNotePosition::LeftOf => {
                        base_x - note_gap_x - width
                    }
                    crate::mermaid_renderer::ir::SequenceNotePosition::RightOf => {
                        base_x + note_gap_x
                    }
                    crate::mermaid_renderer::ir::SequenceNotePosition::Over => {
                        (min_x + max_x) / 2.0 - width / 2.0
                    }
                };
                let y = message_cursor;
                sequence_notes.push(SequenceNoteLayout {
                    x,
                    y,
                    width,
                    height,
                    label,
                    position: note.position,
                    participants: note.participants.clone(),
                    index: note.index,
                });
                message_cursor += height + note_gap_y;
            }
        }
        if idx < graph.edges.len() {
            message_cursor += extra_before[idx];
            message_ys.push(message_cursor);
            message_cursor += base_spacing;
        }
    }

    for (idx, edge) in graph.edges.iter().enumerate() {
        let from = nodes.get(&edge.from).expect("from node missing");
        let to = nodes.get(&edge.to).expect("to node missing");
        let y = message_ys.get(idx).copied().unwrap_or(message_cursor);
        let label = edge.label.as_ref().map(|l| measure_label(l, theme, config));
        let start_label = edge
            .start_label
            .as_ref()
            .map(|l| measure_label(l, theme, config));
        let end_label = edge
            .end_label
            .as_ref()
            .map(|l| measure_label(l, theme, config));

        let points = if edge.from == edge.to {
            let pad = config.node_spacing.max(20.0) * 0.6;
            let x = from.x + from.width / 2.0;
            vec![(x, y), (x + pad, y), (x + pad, y + pad), (x, y + pad)]
        } else {
            let from_x = from.x + from.width / 2.0;
            let to_x = to.x + to.width / 2.0;
            vec![(from_x, y), (to_x, y)]
        };

        let mut override_style = resolve_edge_style(idx, graph);
        if edge.style == crate::mermaid_renderer::ir::EdgeStyle::Dotted
            && override_style.dasharray.is_none()
        {
            override_style.dasharray = Some("3 3".to_string());
        }
        edges.push(EdgeLayout {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label,
            start_label,
            end_label,
            points,
            directed: edge.directed,
            arrow_start: edge.arrow_start,
            arrow_end: edge.arrow_end,
            arrow_start_kind: edge.arrow_start_kind,
            arrow_end_kind: edge.arrow_end_kind,
            start_decoration: edge.start_decoration,
            end_decoration: edge.end_decoration,
            style: edge.style,
            override_style,
        });
    }

    let mut sequence_frames = Vec::new();
    if !graph.sequence_frames.is_empty() && !message_ys.is_empty() {
        let mut frames = graph.sequence_frames.clone();
        frames.sort_by(|a, b| {
            a.start_idx
                .cmp(&b.start_idx)
                .then_with(|| b.end_idx.cmp(&a.end_idx))
        });
        for frame in frames {
            if frame.start_idx >= frame.end_idx || frame.start_idx >= message_ys.len() {
                continue;
            }
            let mut xs = Vec::new();
            for edge in graph
                .edges
                .iter()
                .skip(frame.start_idx)
                .take(frame.end_idx.saturating_sub(frame.start_idx))
            {
                if let Some(node) = nodes.get(&edge.from) {
                    xs.push(node.x + node.width / 2.0);
                }
                if let Some(node) = nodes.get(&edge.to) {
                    xs.push(node.x + node.width / 2.0);
                }
            }
            if xs.is_empty() {
                for node in nodes.values() {
                    xs.push(node.x + node.width / 2.0);
                }
            }
            let (min_x, max_x) = xs
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |acc, x| {
                    (acc.0.min(*x), acc.1.max(*x))
                });
            if !min_x.is_finite() || !max_x.is_finite() {
                continue;
            }
            let frame_pad_x = theme.font_size * 0.7;
            let frame_x = min_x - frame_pad_x;
            let frame_width = (max_x - min_x) + frame_pad_x * 2.0;

            let first_y = message_ys
                .get(frame.start_idx)
                .copied()
                .unwrap_or(message_cursor);
            let last_y = message_ys
                .get(frame.end_idx.saturating_sub(1))
                .copied()
                .unwrap_or(first_y);
            let mut min_y = first_y;
            let mut max_y = last_y;
            for note in &sequence_notes {
                if note.index >= frame.start_idx && note.index <= frame.end_idx {
                    min_y = min_y.min(note.y);
                    max_y = max_y.max(note.y + note.height);
                }
            }
            let header_offset = theme.font_size * 0.6;
            let top_offset = (2.0 * base_spacing - header_offset).max(base_spacing);
            let bottom_offset = header_offset;
            let frame_y = min_y - top_offset;
            let frame_height = (max_y - min_y).max(0.0) + top_offset + bottom_offset;

            let frame_label_text = match frame.kind {
                crate::mermaid_renderer::ir::SequenceFrameKind::Alt => "alt",
                crate::mermaid_renderer::ir::SequenceFrameKind::Opt => "opt",
                crate::mermaid_renderer::ir::SequenceFrameKind::Loop => "loop",
                crate::mermaid_renderer::ir::SequenceFrameKind::Par => "par",
                crate::mermaid_renderer::ir::SequenceFrameKind::Rect => "rect",
                crate::mermaid_renderer::ir::SequenceFrameKind::Critical => "critical",
                crate::mermaid_renderer::ir::SequenceFrameKind::Break => "break",
            };
            let label_block = measure_label(frame_label_text, theme, config);
            let label_box_w =
                (label_block.width + theme.font_size * 2.0).max(theme.font_size * 3.0);
            let label_box_h = theme.font_size * 1.25;
            let label_box_x = frame_x;
            let label_box_y = frame_y;
            let label = SequenceLabel {
                x: label_box_x + label_box_w / 2.0,
                y: label_box_y + label_box_h / 2.0,
                text: label_block,
            };

            let mut dividers = Vec::new();
            let divider_offset = theme.font_size * 0.9;
            for window in frame.sections.windows(2) {
                let prev_end = window[0].end_idx;
                let base_y = message_ys
                    .get(prev_end.saturating_sub(1))
                    .copied()
                    .unwrap_or(first_y);
                dividers.push(base_y + divider_offset);
            }

            let mut section_labels = Vec::new();
            let label_offset = theme.font_size * 0.7;
            for (section_idx, section) in frame.sections.iter().enumerate() {
                if let Some(label) = &section.label {
                    let display = format!("[{}]", label);
                    let block = measure_label(&display, theme, config);
                    let label_y = if section_idx == 0 {
                        frame_y + label_offset
                    } else {
                        dividers
                            .get(section_idx - 1)
                            .copied()
                            .unwrap_or(frame_y + label_offset)
                            + label_offset
                    };
                    section_labels.push(SequenceLabel {
                        x: frame_x + frame_width / 2.0,
                        y: label_y,
                        text: block,
                    });
                }
            }

            sequence_frames.push(SequenceFrameLayout {
                kind: frame.kind,
                x: frame_x,
                y: frame_y,
                width: frame_width,
                height: frame_height,
                label_box: (label_box_x, label_box_y, label_box_w, label_box_h),
                label,
                section_labels,
                dividers,
            });
        }
    }

    let lifeline_start = margin + actor_height;
    let mut last_message_y = message_ys
        .last()
        .copied()
        .unwrap_or(lifeline_start + base_spacing);
    for note in &sequence_notes {
        last_message_y = last_message_y.max(note.y + note.height);
    }
    let footbox_gap = (theme.font_size * 1.25).max(16.0);
    let lifeline_end = last_message_y + footbox_gap;
    let mut lifelines = participants
        .iter()
        .filter_map(|id| nodes.get(id))
        .map(|node| Lifeline {
            id: node.id.clone(),
            x: node.x + node.width / 2.0,
            y1: lifeline_start,
            y2: lifeline_end,
        })
        .collect::<Vec<_>>();

    let mut sequence_footboxes = participants
        .iter()
        .filter_map(|id| nodes.get(id))
        .map(|node| {
            let mut foot = node.clone();
            foot.y = lifeline_end;
            foot
        })
        .collect::<Vec<_>>();

    let mut sequence_boxes = Vec::new();
    if !graph.sequence_boxes.is_empty() {
        let pad_x = theme.font_size * 0.8;
        let pad_y = theme.font_size * 0.6;
        let bottom = sequence_footboxes
            .iter()
            .map(|foot| foot.y + foot.height)
            .fold(lifeline_end, f32::max);
        for seq_box in &graph.sequence_boxes {
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            for participant in &seq_box.participants {
                if let Some(node) = nodes.get(participant) {
                    min_x = min_x.min(node.x);
                    max_x = max_x.max(node.x + node.width);
                }
            }
            if !min_x.is_finite() || !max_x.is_finite() {
                continue;
            }
            let x = min_x - pad_x;
            let y = 0.0;
            let width = (max_x - min_x) + pad_x * 2.0;
            let height = bottom + pad_y;
            let label = seq_box
                .label
                .as_ref()
                .map(|text| measure_label(text, theme, config));
            sequence_boxes.push(SequenceBoxLayout {
                x,
                y,
                width,
                height,
                label,
                color: seq_box.color.clone(),
            });
        }
    }
    let activation_width = (theme.font_size * 0.75).max(10.0);
    let activation_offset = (activation_width * 0.6).max(4.0);
    let activation_end_default = message_ys
        .last()
        .copied()
        .unwrap_or(lifeline_start + base_spacing * 0.5)
        + base_spacing * 0.6;
    let mut sequence_activations = Vec::new();
    let mut activation_stacks: HashMap<String, Vec<(f32, usize)>> = HashMap::new();
    let mut events = graph
        .sequence_activations
        .iter()
        .cloned()
        .enumerate()
        .map(|(order, event)| (event.index, order, event))
        .collect::<Vec<_>>();
    events.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let activation_y_for = |idx: usize| {
        if idx < message_ys.len() {
            message_ys[idx]
        } else {
            activation_end_default
        }
    };
    for (_, _, event) in events {
        let y = activation_y_for(event.index);
        let stack = activation_stacks
            .entry(event.participant.clone())
            .or_default();
        match event.kind {
            crate::mermaid_renderer::ir::SequenceActivationKind::Activate => {
                let depth = stack.len();
                stack.push((y, depth));
            }
            crate::mermaid_renderer::ir::SequenceActivationKind::Deactivate => {
                if let Some((start_y, depth)) = stack.pop()
                    && let Some(node) = nodes.get(&event.participant)
                {
                    let base_x = node.x + node.width / 2.0 - activation_width / 2.0;
                    let x = base_x + depth as f32 * activation_offset;
                    let mut y0 = start_y.min(y);
                    let mut height = (y - start_y).abs();
                    if height < base_spacing * 0.6 {
                        height = base_spacing * 0.6;
                    }
                    if y0 < lifeline_start {
                        y0 = lifeline_start;
                    }
                    sequence_activations.push(SequenceActivationLayout {
                        x,
                        y: y0,
                        width: activation_width,
                        height,
                        participant: event.participant.clone(),
                        depth,
                    });
                }
            }
        }
    }
    for (participant, stack) in activation_stacks {
        for (start_y, depth) in stack {
            if let Some(node) = nodes.get(&participant) {
                let base_x = node.x + node.width / 2.0 - activation_width / 2.0;
                let x = base_x + depth as f32 * activation_offset;
                let mut y0 = start_y.min(activation_end_default);
                let mut height = (activation_end_default - start_y).abs();
                if height < base_spacing * 0.6 {
                    height = base_spacing * 0.6;
                }
                if y0 < lifeline_start {
                    y0 = lifeline_start;
                }
                sequence_activations.push(SequenceActivationLayout {
                    x,
                    y: y0,
                    width: activation_width,
                    height,
                    participant: participant.clone(),
                    depth,
                });
            }
        }
    }

    let mut sequence_numbers = Vec::new();
    if let Some(start) = graph.sequence_autonumber {
        let mut value = start;
        for (idx, edge) in graph.edges.iter().enumerate() {
            if let (Some(from), Some(y)) = (nodes.get(&edge.from), message_ys.get(idx).copied()) {
                let from_x = from.x + from.width / 2.0;
                let to_x = nodes
                    .get(&edge.to)
                    .map(|node| node.x + node.width / 2.0)
                    .unwrap_or(from_x);
                let offset = if to_x >= from_x { 16.0 } else { -16.0 };
                sequence_numbers.push(SequenceNumberLayout {
                    x: from_x + offset,
                    y,
                    value,
                });
                value += 1;
            }
        }
    }

    let (mut width, mut height) = bounds_from_layout(&nodes, &subgraphs);
    let mut max_x = width.max(cursor_x + 40.0) - 60.0;
    let mut max_y = height - 60.0;
    let mut min_x: f32 = 0.0;
    for note in &sequence_notes {
        min_x = min_x.min(note.x);
        max_x = max_x.max(note.x + note.width);
        max_y = max_y.max(note.y + note.height);
    }
    for frame in &sequence_frames {
        min_x = min_x.min(frame.x);
        max_x = max_x.max(frame.x + frame.width);
        max_y = max_y.max(frame.y + frame.height);
    }
    for activation in &sequence_activations {
        min_x = min_x.min(activation.x);
        max_x = max_x.max(activation.x + activation.width);
        max_y = max_y.max(activation.y + activation.height);
    }
    for number in &sequence_numbers {
        min_x = min_x.min(number.x);
        max_x = max_x.max(number.x);
        max_y = max_y.max(number.y);
    }

    let shift_x = if min_x < 0.0 { -min_x + 20.0 } else { 0.0 };
    if shift_x > 0.0 {
        for node in nodes.values_mut() {
            node.x += shift_x;
        }
        for edge in &mut edges {
            for point in &mut edge.points {
                point.0 += shift_x;
            }
        }
        for lifeline in &mut lifelines {
            lifeline.x += shift_x;
        }
        for footbox in &mut sequence_footboxes {
            footbox.x += shift_x;
        }
        for frame in &mut sequence_frames {
            frame.x += shift_x;
            frame.label_box.0 += shift_x;
            frame.label.x += shift_x;
            for label in &mut frame.section_labels {
                label.x += shift_x;
            }
        }
        for note in &mut sequence_notes {
            note.x += shift_x;
        }
        for activation in &mut sequence_activations {
            activation.x += shift_x;
        }
        for number in &mut sequence_numbers {
            number.x += shift_x;
        }
        max_x += shift_x;
    }

    let footbox_height = sequence_footboxes
        .iter()
        .map(|node| node.height)
        .fold(0.0, f32::max);
    max_y = max_y.max(lifeline_end + footbox_height);
    width = max_x + 60.0;
    height = max_y + 60.0;

    Layout {
        kind: graph.kind,
        nodes,
        edges,
        subgraphs,
        lifelines,
        sequence_footboxes,
        sequence_boxes,
        sequence_frames,
        sequence_notes,
        sequence_activations,
        sequence_numbers,
        state_notes: Vec::new(),
        pie_slices: Vec::new(),
        pie_legend: Vec::new(),
        pie_center: (0.0, 0.0),
        pie_radius: 0.0,
        pie_title: None,
        quadrant: None,
        gantt: None,
        sankey: None,
        gitgraph: None,
        c4: None,
        xychart: None,
        timeline: None,
        journey: None,
        error: None,
        width,
        height,
    }
}

fn resolve_edge_style(idx: usize, graph: &Graph) -> crate::mermaid_renderer::ir::EdgeStyleOverride {
    let mut style = graph.edge_style_default.clone().unwrap_or_default();
    if let Some(edge_style) = graph.edge_styles.get(&idx) {
        merge_edge_style(&mut style, edge_style);
    }
    style
}

fn merge_edge_style(
    target: &mut crate::mermaid_renderer::ir::EdgeStyleOverride,
    source: &crate::mermaid_renderer::ir::EdgeStyleOverride,
) {
    if source.stroke.is_some() {
        target.stroke = source.stroke.clone();
    }
    if source.stroke_width.is_some() {
        target.stroke_width = source.stroke_width;
    }
    if source.dasharray.is_some() {
        target.dasharray = source.dasharray.clone();
    }
    if source.label_color.is_some() {
        target.label_color = source.label_color.clone();
    }
}

fn order_rank_nodes(
    rank_nodes: &mut [Vec<String>],
    edges: &[crate::mermaid_renderer::ir::Edge],
    node_order: &HashMap<String, usize>,
    passes: usize,
) {
    if rank_nodes.len() <= 1 {
        return;
    }
    let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();

    for edge in edges {
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        incoming
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }

    let mut positions: HashMap<String, usize> = HashMap::new();
    let update_positions = |rank_nodes: &mut [Vec<String>],
                            positions: &mut HashMap<String, usize>| {
        positions.clear();
        for bucket in rank_nodes.iter() {
            for (idx, node_id) in bucket.iter().enumerate() {
                positions.insert(node_id.clone(), idx);
            }
        }
    };

    update_positions(rank_nodes, &mut positions);

    let sort_bucket = |bucket: &mut Vec<String>,
                       neighbors: &HashMap<String, Vec<String>>,
                       positions: &HashMap<String, usize>| {
        let current_positions: HashMap<String, usize> = bucket
            .iter()
            .enumerate()
            .map(|(idx, id)| (id.clone(), idx))
            .collect();
        bucket.sort_by(|a, b| {
            let a_score = median_position(a, neighbors, positions, &current_positions);
            let b_score = median_position(b, neighbors, positions, &current_positions);
            match a_score.partial_cmp(&b_score) {
                Some(std::cmp::Ordering::Equal) | None => {
                    let a_pos = current_positions.get(a).copied().unwrap_or(0);
                    let b_pos = current_positions.get(b).copied().unwrap_or(0);
                    match a_pos.cmp(&b_pos) {
                        std::cmp::Ordering::Equal => node_order
                            .get(a)
                            .copied()
                            .unwrap_or(usize::MAX)
                            .cmp(&node_order.get(b).copied().unwrap_or(usize::MAX)),
                        other => other,
                    }
                }
                Some(ordering) => ordering,
            }
        });
    };

    let passes = passes.max(1);
    for _ in 0..passes {
        for rank in 1..rank_nodes.len() {
            if rank_nodes[rank].len() <= 1 {
                continue;
            }
            sort_bucket(&mut rank_nodes[rank], &incoming, &positions);
            update_positions(rank_nodes, &mut positions);
        }
        for rank in (0..rank_nodes.len().saturating_sub(1)).rev() {
            if rank_nodes[rank].len() <= 1 {
                continue;
            }
            sort_bucket(&mut rank_nodes[rank], &outgoing, &positions);
            update_positions(rank_nodes, &mut positions);
        }
    }
}

fn median_position(
    node_id: &str,
    neighbors: &HashMap<String, Vec<String>>,
    positions: &HashMap<String, usize>,
    current_positions: &HashMap<String, usize>,
) -> f32 {
    let Some(list) = neighbors.get(node_id) else {
        return *current_positions.get(node_id).unwrap_or(&0) as f32;
    };
    let mut values = Vec::new();
    for neighbor in list {
        if let Some(pos) = positions.get(neighbor) {
            values.push(*pos as f32);
        }
    }
    if values.is_empty() {
        return *current_positions.get(node_id).unwrap_or(&0) as f32;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) * 0.5
    }
}

fn apply_subgraph_bands(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    let mut group_nodes: Vec<Vec<String>> = Vec::new();
    let mut node_group: HashMap<String, usize> = HashMap::new();

    // Group 0: nodes not in any subgraph.
    group_nodes.push(Vec::new());

    let top_level = top_level_subgraph_indices(graph);
    for (pos, idx) in top_level.iter().enumerate() {
        let group_idx = pos + 1;
        let sub = &graph.subgraphs[*idx];
        group_nodes.push(Vec::new());
        for node_id in &sub.nodes {
            if nodes.contains_key(node_id) {
                node_group.insert(node_id.clone(), group_idx);
            }
        }
        if let Some(anchor_id) = subgraph_anchor_id(sub, nodes) {
            if nodes.contains_key(anchor_id) {
                node_group.insert(anchor_id.to_string(), group_idx);
            }
        }
    }

    for node_id in graph.nodes.keys() {
        if node_group.contains_key(node_id) {
            continue;
        }
        node_group.insert(node_id.clone(), 0);
    }

    for (node_id, group_idx) in &node_group {
        if let Some(bucket) = group_nodes.get_mut(*group_idx) {
            bucket.push(node_id.clone());
        }
    }

    let mut groups: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
    for (idx, bucket) in group_nodes.iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node_id in bucket {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
                max_x = max_x.max(node.x + node.width);
                max_y = max_y.max(node.y + node.height);
            }
        }
        if min_x != f32::MAX {
            groups.push((idx, min_x, min_y, max_x, max_y));
        }
    }

    let mut inter_group_edges = 0usize;
    let mut group_links: HashSet<(usize, usize)> = HashSet::new();
    let mut group_degree: HashMap<usize, usize> = HashMap::new();
    for edge in &graph.edges {
        let from_group = node_group.get(&edge.from);
        let to_group = node_group.get(&edge.to);
        if let (Some(a), Some(b)) = (from_group, to_group) {
            if a != b {
                inter_group_edges += 1;
                let (min_g, max_g) = if a < b { (*a, *b) } else { (*b, *a) };
                group_links.insert((min_g, max_g));
                *group_degree.entry(*a).or_insert(0) += 1;
                *group_degree.entry(*b).or_insert(0) += 1;
            }
        }
    }
    let max_degree = group_degree.values().copied().max().unwrap_or(0);
    let path_like = inter_group_edges > 0
        && group_links.len() <= groups.len().saturating_sub(1)
        && max_degree <= 2;
    let grid_pack = inter_group_edges == 0;
    let align_cross = path_like;

    // Order groups by their current position to minimize crossing shifts.
    // Keep the non-subgraph group first to bias subgraphs after the main flow.
    if is_horizontal(graph.direction) {
        groups.sort_by(|a, b| {
            let a_primary = if a.0 == 0 { 0 } else { 1 };
            let b_primary = if b.0 == 0 { 0 } else { 1 };
            a_primary
                .cmp(&b_primary)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
    } else {
        groups.sort_by(|a, b| {
            let a_primary = if a.0 == 0 { 0 } else { 1 };
            let b_primary = if b.0 == 0 { 0 } else { 1 };
            a_primary
                .cmp(&b_primary)
                .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    let spacing = config.rank_spacing * 0.8;
    if is_horizontal(graph.direction) {
        if align_cross && !groups.is_empty() {
            let target_y = groups.iter().map(|group| group.2).fold(f32::MAX, f32::min);
            for (group_idx, _min_x, min_y, _max_x, _max_y) in &groups {
                let offset_y = target_y - *min_y;
                for node_id in group_nodes[*group_idx].iter() {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.y += offset_y;
                    }
                }
            }
        } else if grid_pack && groups.len() > 1 {
            let mut bounds: HashMap<usize, (f32, f32, f32, f32)> = HashMap::new();
            for (group_idx, min_x, min_y, max_x, max_y) in &groups {
                bounds.insert(*group_idx, (*min_x, *min_y, max_x - min_x, max_y - min_y));
            }
            let origin_x = groups.iter().map(|group| group.1).fold(f32::MAX, f32::min);
            let origin_y = groups.iter().map(|group| group.2).fold(f32::MAX, f32::min);

            let mut best_area = f32::MAX;
            let mut best_rows: Vec<Vec<usize>> = Vec::new();
            for cols in 1..=groups.len() {
                let mut rows: Vec<Vec<usize>> = Vec::new();
                let mut idx = 0usize;
                while idx < groups.len() {
                    let mut row = Vec::new();
                    for _ in 0..cols {
                        if idx >= groups.len() {
                            break;
                        }
                        row.push(groups[idx].0);
                        idx += 1;
                    }
                    rows.push(row);
                }
                let mut max_row_width = 0.0f32;
                let mut total_height = 0.0f32;
                for row in &rows {
                    let mut row_width = 0.0f32;
                    let mut row_height = 0.0f32;
                    for (pos, group_idx) in row.iter().enumerate() {
                        if let Some((_, _, width, height)) = bounds.get(group_idx) {
                            row_width += *width;
                            if pos + 1 < row.len() {
                                row_width += spacing;
                            }
                            row_height = row_height.max(*height);
                        }
                    }
                    max_row_width = max_row_width.max(row_width);
                    total_height += row_height;
                }
                if !rows.is_empty() {
                    total_height += spacing * (rows.len().saturating_sub(1) as f32);
                }
                let area = max_row_width * total_height;
                if area < best_area {
                    best_area = area;
                    best_rows = rows;
                }
            }

            let mut cursor_y = origin_y;
            for row in best_rows {
                let mut row_height = 0.0f32;
                let mut cursor_x = origin_x;
                for group_idx in row {
                    let Some((min_x, min_y, width, height)) = bounds.get(&group_idx) else {
                        continue;
                    };
                    let offset_x = cursor_x - min_x;
                    let offset_y = cursor_y - min_y;
                    for node_id in group_nodes[group_idx].iter() {
                        if let Some(node) = nodes.get_mut(node_id) {
                            node.x += offset_x;
                            node.y += offset_y;
                        }
                    }
                    cursor_x += width + spacing;
                    row_height = row_height.max(*height);
                }
                cursor_y += row_height + spacing;
            }
        } else {
            let mut cursor = groups
                .iter()
                .find(|group| group.0 == 0)
                .map(|group| group.3)
                .unwrap_or(0.0)
                + spacing;
            for (group_idx, min_x, _min_y, max_x, _max_y) in groups {
                if group_idx == 0 {
                    continue;
                }
                let width = max_x - min_x;
                let offset = cursor - min_x;
                for node_id in group_nodes[group_idx].iter() {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.x += offset;
                    }
                }
                cursor += width + spacing;
            }
        }
    } else {
        if align_cross && !groups.is_empty() {
            let target_x = groups.iter().map(|group| group.1).fold(f32::MAX, f32::min);
            for (group_idx, min_x, _min_y, _max_x, _max_y) in &groups {
                let offset_x = target_x - *min_x;
                for node_id in group_nodes[*group_idx].iter() {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.x += offset_x;
                    }
                }
            }
        } else if grid_pack && groups.len() > 1 {
            let mut bounds: HashMap<usize, (f32, f32, f32, f32)> = HashMap::new();
            for (group_idx, min_x, min_y, max_x, max_y) in &groups {
                bounds.insert(*group_idx, (*min_x, *min_y, max_x - min_x, max_y - min_y));
            }
            let origin_x = groups.iter().map(|group| group.1).fold(f32::MAX, f32::min);
            let origin_y = groups.iter().map(|group| group.2).fold(f32::MAX, f32::min);

            let mut best_rows = Vec::new();
            let mut best_area = f32::MAX;
            for rows in 1..=groups.len() {
                let cols = (groups.len() + rows - 1) / rows;
                let mut grid: Vec<Vec<usize>> = Vec::new();
                let mut idx = 0usize;
                for _ in 0..rows {
                    let mut col = Vec::new();
                    for _ in 0..cols {
                        if idx >= groups.len() {
                            break;
                        }
                        col.push(groups[idx].0);
                        idx += 1;
                    }
                    grid.push(col);
                }
                let mut max_col_height = 0.0f32;
                let mut total_width = 0.0f32;
                for col in &grid {
                    let mut col_height = 0.0f32;
                    let mut col_width = 0.0f32;
                    for (pos, group_idx) in col.iter().enumerate() {
                        if let Some((_, _, width, height)) = bounds.get(group_idx) {
                            col_height += *height;
                            if pos + 1 < col.len() {
                                col_height += spacing;
                            }
                            col_width = col_width.max(*width);
                        }
                    }
                    max_col_height = max_col_height.max(col_height);
                    total_width += col_width;
                }
                if !grid.is_empty() {
                    total_width += spacing * (grid.len().saturating_sub(1) as f32);
                }
                let area = total_width * max_col_height;
                if area < best_area {
                    best_area = area;
                    best_rows = grid;
                }
            }

            let mut cursor_x = origin_x;
            for col in best_rows {
                let mut col_width = 0.0f32;
                let mut cursor_y = origin_y;
                for group_idx in col {
                    let Some((min_x, min_y, width, height)) = bounds.get(&group_idx) else {
                        continue;
                    };
                    let offset_x = cursor_x - min_x;
                    let offset_y = cursor_y - min_y;
                    for node_id in group_nodes[group_idx].iter() {
                        if let Some(node) = nodes.get_mut(node_id) {
                            node.x += offset_x;
                            node.y += offset_y;
                        }
                    }
                    cursor_y += height + spacing;
                    col_width = col_width.max(*width);
                }
                cursor_x += col_width + spacing;
            }
        } else {
            let mut cursor = groups
                .iter()
                .find(|group| group.0 == 0)
                .map(|group| group.4)
                .unwrap_or(0.0)
                + spacing;
            for (group_idx, _min_x, min_y, _max_x, max_y) in groups {
                if group_idx == 0 {
                    continue;
                }
                let height = max_y - min_y;
                let offset = cursor - min_y;
                for node_id in group_nodes[group_idx].iter() {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.y += offset;
                    }
                }
                cursor += height + spacing;
            }
        }
    }
}

fn compress_linear_subgraphs(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart
        || graph.subgraphs.is_empty()
    {
        return;
    }
    let gap = config.flowchart.auto_spacing.min_spacing;
    let horizontal = is_horizontal(graph.direction);

    for sub in &graph.subgraphs {
        if sub.nodes.len() < 3 {
            continue;
        }
        let sub_set: HashSet<&str> = sub.nodes.iter().map(|id| id.as_str()).collect();
        let mut in_deg: HashMap<String, usize> = HashMap::new();
        let mut out_deg: HashMap<String, usize> = HashMap::new();
        let mut next_map: HashMap<String, String> = HashMap::new();
        let mut edges_in_sub = 0usize;

        for node_id in &sub.nodes {
            in_deg.insert(node_id.clone(), 0);
            out_deg.insert(node_id.clone(), 0);
        }

        for edge in &graph.edges {
            if !sub_set.contains(edge.from.as_str()) || !sub_set.contains(edge.to.as_str()) {
                continue;
            }
            edges_in_sub += 1;
            let out = out_deg.entry(edge.from.clone()).or_insert(0);
            *out += 1;
            if *out == 1 {
                next_map.insert(edge.from.clone(), edge.to.clone());
            } else {
                next_map.remove(&edge.from);
            }
            let entry = in_deg.entry(edge.to.clone()).or_insert(0);
            *entry += 1;
        }

        if edges_in_sub + 1 != sub.nodes.len() {
            continue;
        }
        if in_deg.values().any(|&d| d > 1) || out_deg.values().any(|&d| d > 1) {
            continue;
        }

        let starts: Vec<&String> = sub
            .nodes
            .iter()
            .filter(|id| *in_deg.get(*id).unwrap_or(&0) == 0)
            .collect();
        if starts.len() != 1 {
            continue;
        }

        let mut order: Vec<String> = Vec::with_capacity(sub.nodes.len());
        let mut visited: HashSet<String> = HashSet::new();
        let mut current = starts[0].clone();
        while visited.insert(current.clone()) {
            order.push(current.clone());
            if let Some(next) = next_map.get(&current) {
                current = next.clone();
            } else {
                break;
            }
        }
        if order.len() != sub.nodes.len() {
            continue;
        }

        let min_main = order
            .iter()
            .filter_map(|id| nodes.get(id))
            .map(|node| if horizontal { node.x } else { node.y })
            .fold(f32::MAX, f32::min);
        let mut cursor = min_main;
        for node_id in order {
            if let Some(node) = nodes.get_mut(&node_id) {
                if horizontal {
                    node.x = cursor;
                    cursor += node.width + gap;
                } else {
                    node.y = cursor;
                    cursor += node.height + gap;
                }
            }
        }
    }
}

fn apply_orthogonal_region_bands(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    let mut region_indices = Vec::new();
    for (idx, sub) in graph.subgraphs.iter().enumerate() {
        if is_region_subgraph(sub) {
            region_indices.push(idx);
        }
    }
    if region_indices.is_empty() {
        return;
    }

    let sets: Vec<HashSet<String>> = graph
        .subgraphs
        .iter()
        .map(|sub| sub.nodes.iter().cloned().collect())
        .collect();

    let mut parent_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for region_idx in region_indices {
        let region_set = &sets[region_idx];
        let mut parent: Option<usize> = None;
        for (idx, set) in sets.iter().enumerate() {
            if idx == region_idx {
                continue;
            }
            if set.len() <= region_set.len() {
                continue;
            }
            if !region_set.is_subset(set) {
                continue;
            }
            if is_region_subgraph(&graph.subgraphs[idx]) {
                continue;
            }
            match parent {
                None => parent = Some(idx),
                Some(current) => {
                    if set.len() < sets[current].len() {
                        parent = Some(idx);
                    }
                }
            }
        }
        if let Some(parent_idx) = parent {
            parent_map.entry(parent_idx).or_default().push(region_idx);
        }
    }

    let spacing = config.rank_spacing * 0.6;
    let stack_along_x = is_horizontal(graph.direction);

    for region_list in parent_map.values() {
        let mut region_boxes: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
        for &region_idx in region_list {
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            for node_id in &graph.subgraphs[region_idx].nodes {
                if let Some(node) = nodes.get(node_id) {
                    min_x = min_x.min(node.x);
                    min_y = min_y.min(node.y);
                    max_x = max_x.max(node.x + node.width);
                    max_y = max_y.max(node.y + node.height);
                }
            }
            if min_x != f32::MAX {
                region_boxes.push((region_idx, min_x, min_y, max_x, max_y));
            }
        }
        if region_boxes.len() <= 1 {
            continue;
        }

        if stack_along_x {
            region_boxes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let mut cursor = region_boxes.first().map(|entry| entry.1).unwrap_or(0.0);
            for (region_idx, min_x, _min_y, max_x, _max_y) in region_boxes {
                let offset = cursor - min_x;
                for node_id in &graph.subgraphs[region_idx].nodes {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.x += offset;
                    }
                }
                cursor += (max_x - min_x) + spacing;
            }
        } else {
            region_boxes.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
            let mut cursor = region_boxes.first().map(|entry| entry.2).unwrap_or(0.0);
            for (region_idx, _min_x, min_y, _max_x, max_y) in region_boxes {
                let offset = cursor - min_y;
                for node_id in &graph.subgraphs[region_idx].nodes {
                    if let Some(node) = nodes.get_mut(node_id) {
                        node.y += offset;
                    }
                }
                cursor += (max_y - min_y) + spacing;
            }
        }
    }
}

fn top_level_subgraph_indices(graph: &Graph) -> Vec<usize> {
    let mut sets: Vec<HashSet<String>> = Vec::new();
    for sub in &graph.subgraphs {
        sets.push(sub.nodes.iter().cloned().collect());
    }

    let mut top_level = Vec::new();
    for i in 0..graph.subgraphs.len() {
        let mut nested = false;
        for j in 0..graph.subgraphs.len() {
            if i == j {
                continue;
            }
            if sets[j].len() > sets[i].len() && sets[i].is_subset(&sets[j]) {
                nested = true;
                break;
            }
        }
        if !nested {
            top_level.push(i);
        }
    }
    top_level
}

fn apply_subgraph_direction_overrides(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
    skip_indices: &HashSet<usize>,
) {
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart {
        return;
    }
    for (idx, sub) in graph.subgraphs.iter().enumerate() {
        if skip_indices.contains(&idx) {
            continue;
        }
        if is_region_subgraph(sub) {
            continue;
        }
        let direction = match sub.direction {
            Some(direction) => direction,
            None => {
                if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart {
                    continue;
                }
                subgraph_layout_direction(graph, sub)
            }
        };
        if sub.nodes.is_empty() {
            continue;
        }
        if direction == graph.direction {
            continue;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
            }
        }
        if min_x == f32::MAX {
            continue;
        }

        let mut temp_nodes: BTreeMap<String, NodeLayout> = BTreeMap::new();
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                let mut clone = node.clone();
                clone.x = 0.0;
                clone.y = 0.0;
                temp_nodes.insert(node_id.clone(), clone);
            }
        }
        let local_config = subgraph_layout_config(graph, false, config);
        let ranks = compute_ranks_subset(&sub.nodes, &graph.edges, &graph.node_order);
        assign_positions(
            &sub.nodes,
            &ranks,
            direction,
            &local_config,
            &mut temp_nodes,
            0.0,
            0.0,
        );
        let mut temp_min_x = f32::MAX;
        let mut temp_min_y = f32::MAX;
        for node_id in &sub.nodes {
            if let Some(node) = temp_nodes.get(node_id) {
                temp_min_x = temp_min_x.min(node.x);
                temp_min_y = temp_min_y.min(node.y);
            }
        }
        if temp_min_x == f32::MAX {
            continue;
        }
        for node_id in &sub.nodes {
            if let (Some(target), Some(source)) = (nodes.get_mut(node_id), temp_nodes.get(node_id))
            {
                target.x = source.x - temp_min_x + min_x;
                target.y = source.y - temp_min_y + min_y;
            }
        }

        if matches!(direction, Direction::RightLeft | Direction::BottomTop) {
            mirror_subgraph_nodes(&sub.nodes, nodes, direction);
        }
    }
}

fn subgraph_is_anchorable(
    sub: &crate::mermaid_renderer::ir::Subgraph,
    graph: &Graph,
    nodes: &BTreeMap<String, NodeLayout>,
) -> bool {
    if sub.nodes.is_empty() {
        return false;
    }
    let anchor_id = subgraph_anchor_id(sub, nodes);
    let set: HashSet<&str> = sub.nodes.iter().map(|id| id.as_str()).collect();
    for edge in &graph.edges {
        if let Some(anchor) = anchor_id
            && (edge.from == anchor || edge.to == anchor)
        {
            return false;
        }
        let from_in = set.contains(edge.from.as_str());
        let to_in = set.contains(edge.to.as_str());
        if from_in ^ to_in {
            return false;
        }
    }
    true
}

fn subgraph_should_anchor(
    sub: &crate::mermaid_renderer::ir::Subgraph,
    graph: &Graph,
    nodes: &BTreeMap<String, NodeLayout>,
) -> bool {
    if sub.nodes.is_empty() {
        return false;
    }
    // For flowcharts and state diagrams, anchor if there's an anchor node
    // State diagram composite states can have external edges, so we can't use
    // subgraph_is_anchorable which rejects subgraphs with external edges
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart
        || graph.kind == crate::mermaid_renderer::ir::DiagramKind::State
    {
        return subgraph_anchor_id(sub, nodes).is_some();
    }
    subgraph_is_anchorable(sub, graph, nodes)
}

fn subgraph_anchor_id<'a>(
    sub: &'a crate::mermaid_renderer::ir::Subgraph,
    nodes: &BTreeMap<String, NodeLayout>,
) -> Option<&'a str> {
    if let Some(id) = sub.id.as_deref()
        && nodes.contains_key(id)
        && !sub.nodes.iter().any(|node_id| node_id == id)
    {
        return Some(id);
    }
    let label = sub.label.as_str();
    if nodes.contains_key(label) && !sub.nodes.iter().any(|node_id| node_id == label) {
        return Some(label);
    }
    None
}

fn mark_subgraph_anchor_nodes_hidden(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
) -> HashSet<String> {
    let mut anchor_ids = HashSet::new();
    for sub in &graph.subgraphs {
        let Some(anchor_id) = subgraph_anchor_id(sub, nodes) else {
            continue;
        };
        anchor_ids.insert(anchor_id.to_string());
        if let Some(node) = nodes.get_mut(anchor_id) {
            node.hidden = true;
        }
    }
    anchor_ids
}

fn pick_subgraph_anchor_child(
    sub: &crate::mermaid_renderer::ir::Subgraph,
    graph: &Graph,
    anchor_ids: &HashSet<String>,
) -> Option<String> {
    let mut candidates: Vec<&String> = sub
        .nodes
        .iter()
        .filter(|id| !anchor_ids.contains(*id))
        .collect();
    if candidates.is_empty() {
        candidates = sub.nodes.iter().collect();
    }
    candidates.sort_by_key(|id| graph.node_order.get(*id).copied().unwrap_or(usize::MAX));
    candidates.first().map(|id| (*id).clone())
}

#[derive(Debug, Clone)]
struct SubgraphAnchorInfo {
    sub_idx: usize,
    padding_x: f32,
    top_padding: f32,
}

fn subgraph_layout_direction(
    graph: &Graph,
    sub: &crate::mermaid_renderer::ir::Subgraph,
) -> Direction {
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State {
        return graph.direction;
    }
    let _ = sub; // Subgraph direction is currently ignored for CLI parity.
    graph.direction
}

fn subgraph_layout_config(graph: &Graph, anchorable: bool, config: &LayoutConfig) -> LayoutConfig {
    let mut local = config.clone();
    if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart && anchorable {
        local.rank_spacing = config.rank_spacing + 25.0;
    }
    local
}

fn flowchart_subgraph_padding(direction: Direction) -> (f32, f32) {
    // Mermaid CLI uses larger padding along the main axis and slightly
    // smaller padding along the cross axis.
    if is_horizontal(direction) {
        (40.0, 30.0)
    } else {
        (30.0, 40.0)
    }
}

fn subgraph_padding_from_label(
    graph: &Graph,
    sub: &crate::mermaid_renderer::ir::Subgraph,
    theme: &Theme,
    label_block: &TextBlock,
) -> (f32, f32, f32) {
    if is_region_subgraph(sub) {
        return (0.0, 0.0, 0.0);
    }

    let label_empty = sub.label.trim().is_empty();
    let label_height = if label_empty { 0.0 } else { label_block.height };

    let (pad_x, pad_y) = if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart {
        flowchart_subgraph_padding(graph.direction)
    } else if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Kanban {
        (8.0, 8.0)
    } else {
        let base_padding = if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State {
            16.0
        } else {
            24.0
        };
        (base_padding, base_padding)
    };

    let top_padding = if label_empty {
        pad_y
    } else if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Flowchart {
        // Keep the label comfortably inside the top band without over-expanding
        // the cluster height.
        pad_y.max(label_height + 6.0)
    } else if graph.kind == crate::mermaid_renderer::ir::DiagramKind::Kanban {
        pad_y.max(label_height + 4.0)
    } else if graph.kind == crate::mermaid_renderer::ir::DiagramKind::State {
        (label_height + theme.font_size * 0.75).max(theme.font_size * 1.4)
    } else {
        pad_y + label_height + 8.0
    };

    (pad_x, pad_y, top_padding)
}
fn estimate_subgraph_box_size(
    graph: &Graph,
    sub: &crate::mermaid_renderer::ir::Subgraph,
    nodes: &BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
    anchorable: bool,
) -> Option<(f32, f32, f32, f32)> {
    if sub.nodes.is_empty() {
        return None;
    }
    let direction = subgraph_layout_direction(graph, sub);
    let mut temp_nodes: BTreeMap<String, NodeLayout> = BTreeMap::new();
    for node_id in &sub.nodes {
        if let Some(node) = nodes.get(node_id) {
            let mut clone = node.clone();
            clone.x = 0.0;
            clone.y = 0.0;
            temp_nodes.insert(node_id.clone(), clone);
        }
    }
    let local_config = subgraph_layout_config(graph, anchorable, config);
    let ranks = compute_ranks_subset(&sub.nodes, &graph.edges, &graph.node_order);
    assign_positions(
        &sub.nodes,
        &ranks,
        direction,
        &local_config,
        &mut temp_nodes,
        0.0,
        0.0,
    );
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node_id in &sub.nodes {
        if let Some(node) = temp_nodes.get(node_id) {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.width);
            max_y = max_y.max(node.y + node.height);
        }
    }
    if min_x == f32::MAX {
        return None;
    }
    let label_empty = sub.label.trim().is_empty();
    let mut label_block = measure_label(&sub.label, theme, config);
    if label_empty {
        label_block.width = 0.0;
        label_block.height = 0.0;
    }
    let (padding_x, padding_y, top_padding) =
        subgraph_padding_from_label(graph, sub, theme, &label_block);

    let width = (max_x - min_x) + padding_x * 2.0;
    let height = (max_y - min_y) + padding_y + top_padding;
    Some((width, height, padding_x, top_padding))
}

fn apply_subgraph_anchor_sizes(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) -> HashMap<String, SubgraphAnchorInfo> {
    let mut anchors: HashMap<String, SubgraphAnchorInfo> = HashMap::new();
    if graph.subgraphs.is_empty() {
        return anchors;
    }
    for (idx, sub) in graph.subgraphs.iter().enumerate() {
        if is_region_subgraph(sub) {
            continue;
        }
        if !subgraph_should_anchor(sub, graph, nodes) {
            continue;
        }
        let Some(anchor_id) = subgraph_anchor_id(sub, nodes) else {
            continue;
        };
        let Some((width, height, padding_x, top_padding)) =
            estimate_subgraph_box_size(graph, sub, nodes, theme, config, true)
        else {
            continue;
        };
        if let Some(node) = nodes.get_mut(anchor_id) {
            node.width = width;
            node.height = height;
        }
        anchors.insert(
            anchor_id.to_string(),
            SubgraphAnchorInfo {
                sub_idx: idx,
                padding_x,
                top_padding,
            },
        );
    }
    anchors
}

fn align_subgraphs_to_anchor_nodes(
    graph: &Graph,
    anchor_info: &HashMap<String, SubgraphAnchorInfo>,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) -> HashSet<String> {
    let mut anchored_nodes = HashSet::new();
    if anchor_info.is_empty() {
        return anchored_nodes;
    }
    for (anchor_id, info) in anchor_info {
        let (anchor_x, anchor_y) = {
            let Some(anchor) = nodes.get(anchor_id) else {
                continue;
            };
            (anchor.x, anchor.y)
        };
        let Some(sub) = graph.subgraphs.get(info.sub_idx) else {
            continue;
        };
        let direction = subgraph_layout_direction(graph, sub);
        let local_config = subgraph_layout_config(graph, true, config);
        let ranks = compute_ranks_subset(&sub.nodes, &graph.edges, &graph.node_order);
        assign_positions(
            &sub.nodes,
            &ranks,
            direction,
            &local_config,
            nodes,
            anchor_x + info.padding_x,
            anchor_y + info.top_padding,
        );
        if matches!(direction, Direction::RightLeft | Direction::BottomTop) {
            mirror_subgraph_nodes(&sub.nodes, nodes, direction);
        }
        anchored_nodes.extend(sub.nodes.iter().cloned());
    }
    anchored_nodes
}

fn apply_state_subgraph_layouts(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
    skip_indices: &HashSet<usize>,
) {
    for (idx, sub) in graph.subgraphs.iter().enumerate() {
        if skip_indices.contains(&idx) {
            continue;
        }
        if sub.nodes.len() <= 1 {
            continue;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
            }
        }
        if min_x == f32::MAX {
            continue;
        }
        let ranks = compute_ranks_subset(&sub.nodes, &graph.edges, &graph.node_order);
        assign_positions(
            &sub.nodes,
            &ranks,
            graph.direction,
            config,
            nodes,
            min_x,
            min_y,
        );
    }
}

fn apply_subgraph_anchors(
    graph: &Graph,
    subgraphs: &[SubgraphLayout],
    nodes: &mut BTreeMap<String, NodeLayout>,
) {
    if subgraphs.is_empty() {
        return;
    }

    let mut label_to_index: HashMap<&str, usize> = HashMap::new();
    for (idx, sub) in subgraphs.iter().enumerate() {
        label_to_index.insert(sub.label.as_str(), idx);
    }

    for sub in &graph.subgraphs {
        let Some(&layout_idx) = label_to_index.get(sub.label.as_str()) else {
            continue;
        };
        let layout = &subgraphs[layout_idx];
        let mut anchor_ids: HashSet<&str> = HashSet::new();
        if let Some(id) = &sub.id {
            anchor_ids.insert(id.as_str());
        }
        anchor_ids.insert(sub.label.as_str());

        for anchor_id in anchor_ids {
            if sub.nodes.iter().any(|node_id| node_id == anchor_id) {
                continue;
            }
            let Some(node) = nodes.get_mut(anchor_id) else {
                continue;
            };
            node.anchor_subgraph = Some(layout_idx);
            let size = 2.0;
            node.width = size;
            node.height = size;
            node.x = layout.x + layout.width / 2.0 - size / 2.0;
            node.y = layout.y + layout.height / 2.0 - size / 2.0;
        }
    }
}

fn anchor_layout_for_edge(
    anchor: &NodeLayout,
    subgraph: &SubgraphLayout,
    direction: Direction,
    is_from: bool,
) -> NodeLayout {
    let size = 2.0;
    let mut node = anchor.clone();
    node.width = size;
    node.height = size;

    if is_horizontal(direction) {
        let x = if is_from {
            subgraph.x + subgraph.width - size
        } else {
            subgraph.x
        };
        let y = subgraph.y + subgraph.height / 2.0 - size / 2.0;
        node.x = x;
        node.y = y;
    } else {
        let x = subgraph.x + subgraph.width / 2.0 - size / 2.0;
        let y = if is_from {
            subgraph.y + subgraph.height - size
        } else {
            subgraph.y
        };
        node.x = x;
        node.y = y;
    }

    node
}

fn mirror_subgraph_nodes(
    node_ids: &[String],
    nodes: &mut BTreeMap<String, NodeLayout>,
    direction: Direction,
) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for node_id in node_ids {
        if let Some(node) = nodes.get(node_id) {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.width);
            max_y = max_y.max(node.y + node.height);
        }
    }

    if min_x == f32::MAX {
        return;
    }

    if matches!(direction, Direction::RightLeft) {
        for node_id in node_ids {
            if let Some(node) = nodes.get_mut(node_id) {
                node.x = min_x + (max_x - (node.x + node.width));
            }
        }
    }
    if matches!(direction, Direction::BottomTop) {
        for node_id in node_ids {
            if let Some(node) = nodes.get_mut(node_id) {
                node.y = min_y + (max_y - (node.y + node.height));
            }
        }
    }
}

fn compute_ranks_subset(
    node_ids: &[String],
    edges: &[crate::mermaid_renderer::ir::Edge],
    node_order: &HashMap<String, usize>,
) -> HashMap<String, usize> {
    let set: HashSet<String> = node_ids.iter().cloned().collect();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();

    for edge in edges {
        if set.contains(&edge.from) && set.contains(&edge.to) {
            adj.entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
            rev.entry(edge.to.clone())
                .or_default()
                .push(edge.from.clone());
        }
    }

    let mut fallback_order: HashMap<&str, usize> = HashMap::new();
    for (idx, id) in node_ids.iter().enumerate() {
        fallback_order.insert(id.as_str(), idx);
    }
    let order_key = |id: &str| -> usize {
        node_order
            .get(id)
            .copied()
            .unwrap_or_else(|| fallback_order.get(id).copied().unwrap_or(usize::MAX))
    };

    let mut indeg: HashMap<String, usize> = HashMap::new();
    for id in &set {
        let count = rev.get(id).map(|v| v.len()).unwrap_or(0);
        indeg.insert(id.clone(), count);
    }

    let mut ready: BinaryHeap<Reverse<(usize, String)>> = BinaryHeap::new();
    for id in &set {
        if *indeg.get(id).unwrap_or(&0) == 0 {
            ready.push(Reverse((order_key(id.as_str()), id.clone())));
        }
    }

    let mut order = Vec::with_capacity(set.len());
    while let Some(Reverse((_key, id))) = ready.pop() {
        order.push(id.clone());
        if let Some(nexts) = adj.get(&id) {
            for next in nexts {
                if let Some(deg) = indeg.get_mut(next) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        ready.push(Reverse((order_key(next.as_str()), next.clone())));
                    }
                }
            }
        }
    }

    if order.len() < set.len() {
        // Break cycles deterministically using a feedback arc heuristic.
        let order_set: HashSet<String> = order.iter().cloned().collect();
        let mut remaining: HashSet<String> = set.difference(&order_set).cloned().collect();
        let mut cycle_order = Vec::new();
        while !remaining.is_empty() {
            let mut heap: BinaryHeap<(i32, Reverse<(usize, String)>)> = BinaryHeap::new();
            for id in &remaining {
                let out = adj
                    .get(id)
                    .map(|v| v.iter().filter(|to| remaining.contains(*to)).count())
                    .unwrap_or(0) as i32;
                let inp = rev
                    .get(id)
                    .map(|v| v.iter().filter(|from| remaining.contains(*from)).count())
                    .unwrap_or(0) as i32;
                heap.push((out - inp, Reverse((order_key(id.as_str()), id.clone()))));
            }
            if let Some((_score, Reverse((_key, id)))) = heap.pop() {
                remaining.remove(&id);
                cycle_order.push(id);
            } else {
                break;
            }
        }
        order.extend(cycle_order);
    }

    let order_index: HashMap<String, usize> = order
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.clone(), idx))
        .collect();

    let mut ranks: HashMap<String, usize> = HashMap::new();
    for node in &order {
        let rank = *ranks.get(node).unwrap_or(&0);
        ranks.entry(node.clone()).or_insert(rank);
        if let Some(nexts) = adj.get(node) {
            let from_idx = *order_index.get(node).unwrap_or(&0);
            for next in nexts {
                let to_idx = *order_index.get(next).unwrap_or(&from_idx);
                if to_idx <= from_idx {
                    continue;
                }
                let entry = ranks.entry(next.clone()).or_insert(0);
                *entry = (*entry).max(rank + 1);
            }
        }
    }

    ranks
}

fn assign_positions(
    node_ids: &[String],
    ranks: &HashMap<String, usize>,
    direction: Direction,
    config: &LayoutConfig,
    nodes: &mut BTreeMap<String, NodeLayout>,
    origin_x: f32,
    origin_y: f32,
) {
    let mut max_rank = 0usize;
    for rank in ranks.values() {
        max_rank = max_rank.max(*rank);
    }

    let mut rank_nodes: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for node_id in node_ids {
        let rank = *ranks.get(node_id).unwrap_or(&0);
        if let Some(bucket) = rank_nodes.get_mut(rank) {
            bucket.push(node_id.clone());
        }
    }
    for bucket in &mut rank_nodes {
        bucket.sort();
    }

    let mut main_cursor = 0.0;
    for bucket in rank_nodes {
        let mut cross_cursor = 0.0;
        let mut max_main: f32 = 0.0;
        for node_id in bucket {
            if let Some(node) = nodes.get_mut(&node_id) {
                if is_horizontal(direction) {
                    node.x = origin_x + main_cursor;
                    node.y = origin_y + cross_cursor;
                    cross_cursor += node.height + config.node_spacing;
                    max_main = max_main.max(node.width);
                } else {
                    node.x = origin_x + cross_cursor;
                    node.y = origin_y + main_cursor;
                    cross_cursor += node.width + config.node_spacing;
                    max_main = max_main.max(node.height);
                }
            }
        }
        main_cursor += max_main + config.rank_spacing;
    }
}

fn bounds_from_layout(
    nodes: &BTreeMap<String, NodeLayout>,
    subgraphs: &[SubgraphLayout],
) -> (f32, f32) {
    let (max_x, max_y) = bounds_without_padding(nodes, subgraphs);
    (max_x + 60.0, max_y + 60.0)
}

fn bounds_without_padding(
    nodes: &BTreeMap<String, NodeLayout>,
    subgraphs: &[SubgraphLayout],
) -> (f32, f32) {
    bounds_with_edges(nodes, subgraphs, &[])
}

fn bounds_with_edges(
    nodes: &BTreeMap<String, NodeLayout>,
    subgraphs: &[SubgraphLayout],
    edges: &[EdgeLayout],
) -> (f32, f32) {
    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    for node in nodes.values() {
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    for sub in subgraphs {
        let invisible_region = sub.label.trim().is_empty()
            && sub.style.stroke.as_deref() == Some("none")
            && sub.style.fill.as_deref() == Some("none");
        if invisible_region {
            continue;
        }
        max_x = max_x.max(sub.x + sub.width);
        max_y = max_y.max(sub.y + sub.height);
    }
    // Also include edge points - routing can place waypoints outside node bounds
    for edge in edges {
        for point in &edge.points {
            max_x = max_x.max(point.0);
            max_y = max_y.max(point.1);
        }
    }
    (max_x, max_y)
}

fn apply_direction_mirror(
    direction: Direction,
    nodes: &mut BTreeMap<String, NodeLayout>,
    edges: &mut [EdgeLayout],
    subgraphs: &mut [SubgraphLayout],
) {
    let (max_x, max_y) = bounds_without_padding(nodes, subgraphs);
    if matches!(direction, Direction::RightLeft) {
        for node in nodes.values_mut() {
            node.x = max_x - node.x - node.width;
        }
        for edge in edges.iter_mut() {
            for point in edge.points.iter_mut() {
                point.0 = max_x - point.0;
            }
        }
        for sub in subgraphs.iter_mut() {
            sub.x = max_x - sub.x - sub.width;
        }
    }
    if matches!(direction, Direction::BottomTop) {
        for node in nodes.values_mut() {
            node.y = max_y - node.y - node.height;
        }
        for edge in edges.iter_mut() {
            for point in edge.points.iter_mut() {
                point.1 = max_y - point.1;
            }
        }
        for sub in subgraphs.iter_mut() {
            sub.y = max_y - sub.y - sub.height;
        }
    }
}

fn normalize_layout(
    nodes: &mut BTreeMap<String, NodeLayout>,
    edges: &mut [EdgeLayout],
    subgraphs: &mut [SubgraphLayout],
) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    for node in nodes.values() {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
    }
    for sub in subgraphs.iter() {
        min_x = min_x.min(sub.x);
        min_y = min_y.min(sub.y);
    }
    // Also check edge points - routing can place waypoints outside node bounds
    for edge in edges.iter() {
        for point in &edge.points {
            min_x = min_x.min(point.0);
            min_y = min_y.min(point.1);
        }
    }

    let padding = 8.0;
    let shift_x = if min_x < padding {
        padding - min_x
    } else {
        0.0
    };
    let shift_y = if min_y < padding {
        padding - min_y
    } else {
        0.0
    };

    if shift_x == 0.0 && shift_y == 0.0 {
        return;
    }

    for node in nodes.values_mut() {
        node.x += shift_x;
        node.y += shift_y;
    }
    for edge in edges.iter_mut() {
        for point in edge.points.iter_mut() {
            point.0 += shift_x;
            point.1 += shift_y;
        }
    }
    for sub in subgraphs.iter_mut() {
        sub.x += shift_x;
        sub.y += shift_y;
    }
}

struct RouteContext<'a> {
    from_id: &'a str,
    to_id: &'a str,
    from: &'a NodeLayout,
    to: &'a NodeLayout,
    direction: Direction,
    config: &'a LayoutConfig,
    obstacles: &'a [Obstacle],
    label_obstacles: &'a [Obstacle],
    fast_route: bool,
    base_offset: f32,
    start_side: EdgeSide,
    end_side: EdgeSide,
    start_offset: f32,
    end_offset: f32,
}

#[derive(Debug, Clone)]
struct EdgeOccupancy {
    cell: f32,
    weights: HashMap<(i32, i32), u16>,
}

impl EdgeOccupancy {
    fn new(cell: f32) -> Self {
        let cell = cell.max(8.0);
        Self {
            cell,
            weights: HashMap::new(),
        }
    }

    fn cell_index(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / self.cell).floor() as i32,
            (y / self.cell).floor() as i32,
        )
    }

    fn score_path(&self, points: &[(f32, f32)]) -> u32 {
        let mut score = 0u32;
        for segment in points.windows(2) {
            let (x1, y1) = segment[0];
            let (x2, y2) = segment[1];
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            let steps = ((len / self.cell).ceil() as usize).max(1);
            let stride = if steps > 32 { (steps / 32).max(1) } else { 1 };
            for i in (0..=steps).step_by(stride) {
                let t = i as f32 / steps as f32;
                let x = x1 + dx * t;
                let y = y1 + dy * t;
                if let Some(weight) = self.weights.get(&self.cell_index(x, y)) {
                    score += *weight as u32;
                }
            }
        }
        score
    }

    fn overlap_count(&self, points: &[(f32, f32)]) -> u32 {
        let mut count = 0u32;
        for segment in points.windows(2) {
            let (x1, y1) = segment[0];
            let (x2, y2) = segment[1];
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            let steps = ((len / self.cell).ceil() as usize).max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let x = x1 + dx * t;
                let y = y1 + dy * t;
                if let Some(weight) = self.weights.get(&self.cell_index(x, y))
                    && *weight > 0
                {
                    count = count.saturating_add(1);
                }
            }
        }
        count
    }

    fn add_path(&mut self, points: &[(f32, f32)]) {
        for segment in points.windows(2) {
            let (x1, y1) = segment[0];
            let (x2, y2) = segment[1];
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            let steps = ((len / self.cell).ceil() as usize).max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let x = x1 + dx * t;
                let y = y1 + dy * t;
                let (ix, iy) = self.cell_index(x, y);
                for dx_cell in -1i32..=1 {
                    for dy_cell in -1i32..=1 {
                        let weight = match (dx_cell.abs(), dy_cell.abs()) {
                            (0, 0) => 3u16,
                            (1, 0) | (0, 1) => 2u16,
                            _ => 1u16,
                        };
                        let idx = (ix + dx_cell, iy + dy_cell);
                        let entry = self.weights.entry(idx).or_insert(0);
                        *entry = entry.saturating_add(weight);
                    }
                }
            }
        }
    }

    fn weight_at(&self, x: f32, y: f32) -> u16 {
        self.weights
            .get(&self.cell_index(x, y))
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
struct RoutingGrid {
    cell: f32,
    min_x: f32,
    min_y: f32,
    cols: i32,
    rows: i32,
    cell_obstacles: Vec<Vec<usize>>,
}

impl RoutingGrid {
    fn new(obstacles: &[Obstacle], cell: f32, margin: f32, max_cells: usize) -> Option<Self> {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for obs in obstacles {
            min_x = min_x.min(obs.x);
            min_y = min_y.min(obs.y);
            max_x = max_x.max(obs.x + obs.width);
            max_y = max_y.max(obs.y + obs.height);
        }
        if min_x == f32::MAX {
            return None;
        }
        min_x -= margin;
        min_y -= margin;
        max_x += margin;
        max_y += margin;
        let cell = cell.max(6.0);
        let cols = ((max_x - min_x) / cell).ceil() as i32 + 1;
        let rows = ((max_y - min_y) / cell).ceil() as i32 + 1;
        if cols <= 1 || rows <= 1 {
            return None;
        }
        let total_cells = (cols as usize).saturating_mul(rows as usize);
        if total_cells > max_cells {
            return None;
        }
        let mut cell_obstacles = vec![Vec::new(); (cols * rows) as usize];
        for (idx, obs) in obstacles.iter().enumerate() {
            let start_x = ((obs.x - min_x) / cell).floor().max(0.0) as i32;
            let end_x = ((obs.x + obs.width - min_x) / cell)
                .floor()
                .min((cols - 1) as f32) as i32;
            let start_y = ((obs.y - min_y) / cell).floor().max(0.0) as i32;
            let end_y = ((obs.y + obs.height - min_y) / cell)
                .floor()
                .min((rows - 1) as f32) as i32;
            for iy in start_y..=end_y {
                for ix in start_x..=end_x {
                    let cell_idx = (iy * cols + ix) as usize;
                    cell_obstacles[cell_idx].push(idx);
                }
            }
        }
        Some(Self {
            cell,
            min_x,
            min_y,
            cols,
            rows,
            cell_obstacles,
        })
    }

    fn index(&self, ix: i32, iy: i32) -> usize {
        (iy * self.cols + ix) as usize
    }

    fn cell_for_point(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        let ix = ((x - self.min_x) / self.cell).floor() as i32;
        let iy = ((y - self.min_y) / self.cell).floor() as i32;
        if ix < 0 || iy < 0 || ix >= self.cols || iy >= self.rows {
            return None;
        }
        Some((ix, iy))
    }

    fn cell_center(&self, ix: i32, iy: i32) -> (f32, f32) {
        (
            self.min_x + (ix as f32 + 0.5) * self.cell,
            self.min_y + (iy as f32 + 0.5) * self.cell,
        )
    }

    fn cell_obstacle_indices(&self, ix: i32, iy: i32) -> &[usize] {
        &self.cell_obstacles[self.index(ix, iy)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GridState {
    x: i32,
    y: i32,
    dir: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GridEntry {
    est: u32,
    cost: u32,
    state: GridState,
}

impl Ord for GridEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est
            .cmp(&self.est)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| self.state.y.cmp(&other.state.y))
            .then_with(|| self.state.x.cmp(&other.state.x))
            .then_with(|| self.state.dir.cmp(&other.state.dir))
    }
}

impl PartialOrd for GridEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn apply_port_offset(point: (f32, f32), side: EdgeSide, offset: f32) -> (f32, f32) {
    match side {
        EdgeSide::Left | EdgeSide::Right => (point.0, point.1 + offset),
        EdgeSide::Top | EdgeSide::Bottom => (point.0 + offset, point.1),
    }
}

fn port_stub_length(config: &LayoutConfig, from: &NodeLayout, to: &NodeLayout) -> f32 {
    let base = config.node_spacing * 0.35;
    let size_cap = from.width.min(from.height).min(to.width.min(to.height)) * 0.35;
    let max_len = if size_cap.is_finite() && size_cap > 0.0 {
        size_cap
    } else {
        18.0
    };
    base.min(max_len).clamp(6.0, 22.0)
}

fn port_stub_point(point: (f32, f32), side: EdgeSide, length: f32) -> (f32, f32) {
    match side {
        EdgeSide::Left => (point.0 - length, point.1),
        EdgeSide::Right => (point.0 + length, point.1),
        EdgeSide::Top => (point.0, point.1 - length),
        EdgeSide::Bottom => (point.0, point.1 + length),
    }
}

fn shape_polygon_points(node: &NodeLayout) -> Option<Vec<(f32, f32)>> {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;
    match node.shape {
        crate::mermaid_renderer::ir::NodeShape::Rectangle
        | crate::mermaid_renderer::ir::NodeShape::RoundRect
        | crate::mermaid_renderer::ir::NodeShape::ActorBox
        | crate::mermaid_renderer::ir::NodeShape::Stadium
        | crate::mermaid_renderer::ir::NodeShape::Subroutine
        | crate::mermaid_renderer::ir::NodeShape::Text => {
            Some(vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
        }
        crate::mermaid_renderer::ir::NodeShape::Diamond => {
            let cx = x + w / 2.0;
            let cy = y + h / 2.0;
            Some(vec![(cx, y), (x + w, cy), (cx, y + h), (x, cy)])
        }
        crate::mermaid_renderer::ir::NodeShape::Hexagon => {
            let x1 = x + w * 0.25;
            let x2 = x + w * 0.75;
            let y_mid = y + h / 2.0;
            Some(vec![
                (x1, y),
                (x2, y),
                (x + w, y_mid),
                (x2, y + h),
                (x1, y + h),
                (x, y_mid),
            ])
        }
        crate::mermaid_renderer::ir::NodeShape::Parallelogram
        | crate::mermaid_renderer::ir::NodeShape::ParallelogramAlt => {
            let offset = w * 0.18;
            let points = if node.shape == crate::mermaid_renderer::ir::NodeShape::Parallelogram {
                vec![
                    (x + offset, y),
                    (x + w, y),
                    (x + w - offset, y + h),
                    (x, y + h),
                ]
            } else {
                vec![
                    (x, y),
                    (x + w - offset, y),
                    (x + w, y + h),
                    (x + offset, y + h),
                ]
            };
            Some(points)
        }
        crate::mermaid_renderer::ir::NodeShape::Trapezoid
        | crate::mermaid_renderer::ir::NodeShape::TrapezoidAlt => {
            let offset = w * 0.18;
            let points = if node.shape == crate::mermaid_renderer::ir::NodeShape::Trapezoid {
                vec![
                    (x + offset, y),
                    (x + w - offset, y),
                    (x + w, y + h),
                    (x, y + h),
                ]
            } else {
                vec![
                    (x, y),
                    (x + w, y),
                    (x + w - offset, y + h),
                    (x + offset, y + h),
                ]
            };
            Some(points)
        }
        crate::mermaid_renderer::ir::NodeShape::Asymmetric => {
            let slant = w * 0.22;
            Some(vec![
                (x, y),
                (x + w - slant, y),
                (x + w, y + h / 2.0),
                (x + w - slant, y + h),
                (x, y + h),
            ])
        }
        _ => None,
    }
}

fn ray_polygon_intersection(
    origin: (f32, f32),
    dir: (f32, f32),
    poly: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let mut best_t = None;
    let ox = origin.0;
    let oy = origin.1;
    let rx = dir.0;
    let ry = dir.1;
    if poly.len() < 2 {
        return None;
    }
    for i in 0..poly.len() {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % poly.len()];
        let sx = x2 - x1;
        let sy = y2 - y1;
        let qx = x1 - ox;
        let qy = y1 - oy;
        let denom = rx * sy - ry * sx;
        if denom.abs() < 1e-6 {
            continue;
        }
        let t = (qx * sy - qy * sx) / denom;
        let u = (qx * ry - qy * rx) / denom;
        if t >= 0.0 && (0.0..=1.0).contains(&u) {
            match best_t {
                Some(best) if t >= best => {}
                _ => best_t = Some(t),
            }
        }
    }
    best_t.map(|t| (ox + rx * t, oy + ry * t))
}

fn ray_ellipse_intersection(
    origin: (f32, f32),
    dir: (f32, f32),
    center: (f32, f32),
    rx: f32,
    ry: f32,
) -> Option<(f32, f32)> {
    let (ox, oy) = origin;
    let (dx, dy) = dir;
    let (cx, cy) = center;
    let ox = ox - cx;
    let oy = oy - cy;
    let a = (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry);
    let b = 2.0 * ((ox * dx) / (rx * rx) + (oy * dy) / (ry * ry));
    let c = (ox * ox) / (rx * rx) + (oy * oy) / (ry * ry) - 1.0;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 || a.abs() < 1e-6 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    let t = if t1 >= 0.0 {
        t1
    } else if t2 >= 0.0 {
        t2
    } else {
        return None;
    };
    Some((origin.0 + dx * t, origin.1 + dy * t))
}

fn anchor_point_for_node(node: &NodeLayout, side: EdgeSide, offset: f32) -> (f32, f32) {
    let cx = node.x + node.width / 2.0;
    let cy = node.y + node.height / 2.0;
    let (dir, perp, max_offset) = match side {
        EdgeSide::Left => ((-1.0, 0.0), (0.0, 1.0), node.height / 2.0 - 1.0),
        EdgeSide::Right => ((1.0, 0.0), (0.0, 1.0), node.height / 2.0 - 1.0),
        EdgeSide::Top => ((0.0, -1.0), (1.0, 0.0), node.width / 2.0 - 1.0),
        EdgeSide::Bottom => ((0.0, 1.0), (1.0, 0.0), node.width / 2.0 - 1.0),
    };
    let clamp = if max_offset > 0.0 {
        offset.clamp(-max_offset, max_offset)
    } else {
        0.0
    };
    let origin = (cx + perp.0 * clamp, cy + perp.1 * clamp);

    match node.shape {
        crate::mermaid_renderer::ir::NodeShape::Circle
        | crate::mermaid_renderer::ir::NodeShape::DoubleCircle => {
            let rx = node.width / 2.0;
            let ry = node.height / 2.0;
            if let Some(point) = ray_ellipse_intersection(origin, dir, (cx, cy), rx, ry) {
                return point;
            }
        }
        _ => {}
    }

    if let Some(poly) = shape_polygon_points(node)
        && let Some(point) = ray_polygon_intersection(origin, dir, &poly)
    {
        return point;
    }

    // Fallback to bounding box anchor.
    let base = match side {
        EdgeSide::Left => (node.x, cy),
        EdgeSide::Right => (node.x + node.width, cy),
        EdgeSide::Top => (cx, node.y),
        EdgeSide::Bottom => (cx, node.y + node.height),
    };
    apply_port_offset(base, side, clamp)
}

fn routing_cell_size(config: &LayoutConfig) -> f32 {
    let mut cell = config.flowchart.routing.grid_cell;
    if cell <= 0.0 {
        cell = config.node_spacing * 0.35;
    }
    cell.max(8.0)
}

fn build_routing_grid(obstacles: &[Obstacle], config: &LayoutConfig) -> Option<RoutingGrid> {
    let cell = routing_cell_size(config);
    let margin = config.node_spacing.max(24.0) * 2.0;
    let max_cells = (config.flowchart.routing.max_steps / 16).max(3000);
    RoutingGrid::new(obstacles, cell, margin, max_cells)
}

fn cell_blocked(
    grid: &RoutingGrid,
    obstacles: &[Obstacle],
    ix: i32,
    iy: i32,
    ctx: &RouteContext<'_>,
) -> bool {
    let (cx, cy) = grid.cell_center(ix, iy);
    for &obs_idx in grid.cell_obstacle_indices(ix, iy) {
        let obstacle = &obstacles[obs_idx];
        if obstacle.id == ctx.from_id || obstacle.id == ctx.to_id {
            continue;
        }
        if let Some(members) = &obstacle.members
            && (members.contains(ctx.from_id) || members.contains(ctx.to_id))
        {
            continue;
        }
        if cx >= obstacle.x
            && cx <= obstacle.x + obstacle.width
            && cy >= obstacle.y
            && cy <= obstacle.y + obstacle.height
        {
            return true;
        }
    }
    false
}

fn compress_path(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(points.len());
    out.push(points[0]);
    for idx in 1..points.len() - 1 {
        let prev = out[out.len() - 1];
        let curr = points[idx];
        if (curr.0 - prev.0).abs() <= 1e-4 && (curr.1 - prev.1).abs() <= 1e-4 {
            continue;
        }
        if idx == 1 || idx == points.len() - 2 {
            out.push(curr);
            continue;
        }
        let next = points[idx + 1];
        let dx1 = curr.0 - prev.0;
        let dy1 = curr.1 - prev.1;
        let dx2 = next.0 - curr.0;
        let dy2 = next.1 - curr.1;
        if (dx1.abs() <= 1e-4 && dx2.abs() <= 1e-4) || (dy1.abs() <= 1e-4 && dy2.abs() <= 1e-4) {
            continue;
        }
        out.push(curr);
    }
    let last = points[points.len() - 1];
    if (last.0 - out[out.len() - 1].0).abs() > 1e-4 || (last.1 - out[out.len() - 1].1).abs() > 1e-4
    {
        out.push(last);
    }
    out
}

fn route_edge_with_grid(
    ctx: &RouteContext<'_>,
    grid: &RoutingGrid,
    occupancy: Option<&EdgeOccupancy>,
    start: (f32, f32),
    end: (f32, f32),
) -> Option<Vec<(f32, f32)>> {
    if !ctx.config.flowchart.routing.enable_grid_router {
        return None;
    }

    let (start_ix, start_iy) = grid.cell_for_point(start.0, start.1)?;
    let (end_ix, end_iy) = grid.cell_for_point(end.0, end.1)?;
    if start_ix == end_ix && start_iy == end_iy {
        return Some(vec![start, end]);
    }

    let dirs: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    let step_cost = (grid.cell * 1000.0).round() as u32;
    let turn_penalty =
        (ctx.config.flowchart.routing.turn_penalty * grid.cell * 1000.0).round() as u32;
    let occupancy_weight =
        (ctx.config.flowchart.routing.occupancy_weight * grid.cell * 1000.0).round() as u32;
    let max_steps = ctx.config.flowchart.routing.max_steps.max(10_000);

    let cols = grid.cols;
    let rows = grid.rows;
    let states = (cols * rows * 4) as usize;
    let mut best_cost = vec![u32::MAX; states];
    let mut prev: Vec<Option<GridState>> = vec![None; states];
    let mut heap = BinaryHeap::new();

    for dir in 0..4u8 {
        let idx = ((start_iy * cols + start_ix) as usize) * 4 + dir as usize;
        best_cost[idx] = 0;
        heap.push(GridEntry {
            est: 0,
            cost: 0,
            state: GridState {
                x: start_ix,
                y: start_iy,
                dir,
            },
        });
    }

    let mut end_state: Option<GridState> = None;
    let mut steps = 0usize;

    while let Some(entry) = heap.pop() {
        steps += 1;
        if steps > max_steps {
            break;
        }
        let GridEntry { cost, state, .. } = entry;
        let state_idx = ((state.y * cols + state.x) as usize) * 4 + state.dir as usize;
        if cost != best_cost[state_idx] {
            continue;
        }
        if state.x == end_ix && state.y == end_iy {
            end_state = Some(state);
            break;
        }
        for (dir_idx, (dx, dy)) in dirs.iter().enumerate() {
            let nx = state.x + dx;
            let ny = state.y + dy;
            if nx < 0 || ny < 0 || nx >= cols || ny >= rows {
                continue;
            }
            if (nx != end_ix || ny != end_iy)
                && (nx != start_ix || ny != start_iy)
                && cell_blocked(grid, ctx.obstacles, nx, ny, ctx)
            {
                continue;
            }
            let mut next_cost = cost.saturating_add(step_cost);
            if state.dir != dir_idx as u8 {
                next_cost = next_cost.saturating_add(turn_penalty);
            }
            if let Some(occ) = occupancy {
                let (cx, cy) = grid.cell_center(nx, ny);
                let weight = occ.weight_at(cx, cy) as u32;
                if weight > 0 {
                    next_cost = next_cost.saturating_add(weight.saturating_mul(occupancy_weight));
                }
            }
            let next_idx = ((ny * cols + nx) as usize) * 4 + dir_idx;
            if next_cost >= best_cost[next_idx] {
                continue;
            }
            best_cost[next_idx] = next_cost;
            prev[next_idx] = Some(state);
            let manhattan = (nx - end_ix).unsigned_abs() + (ny - end_iy).unsigned_abs();
            let est = next_cost.saturating_add(manhattan.saturating_mul(step_cost));
            heap.push(GridEntry {
                est,
                cost: next_cost,
                state: GridState {
                    x: nx,
                    y: ny,
                    dir: dir_idx as u8,
                },
            });
        }
    }

    let end_state = end_state?;
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let mut cur = end_state;
    loop {
        cells.push((cur.x, cur.y));
        let cur_idx = ((cur.y * cols + cur.x) as usize) * 4 + cur.dir as usize;
        if let Some(prev_state) = prev[cur_idx] {
            cur = prev_state;
        } else {
            break;
        }
    }
    cells.reverse();
    if cells.is_empty() {
        return None;
    }

    let mut points: Vec<(f32, f32)> = Vec::with_capacity(cells.len() + 4);
    points.push(start);
    if let Some((ix, iy)) = cells.first() {
        let (cx, cy) = grid.cell_center(*ix, *iy);
        match ctx.start_side {
            EdgeSide::Left | EdgeSide::Right => points.push((cx, start.1)),
            EdgeSide::Top | EdgeSide::Bottom => points.push((start.0, cy)),
        }
        points.push((cx, cy));
    }
    for &(ix, iy) in cells.iter().skip(1) {
        points.push(grid.cell_center(ix, iy));
    }
    if let Some((ix, iy)) = cells.last() {
        let (cx, cy) = grid.cell_center(*ix, *iy);
        match ctx.end_side {
            EdgeSide::Left | EdgeSide::Right => points.push((cx, end.1)),
            EdgeSide::Top | EdgeSide::Bottom => points.push((end.0, cy)),
        }
    }
    points.push(end);
    Some(compress_path(&points))
}

fn push_route_candidate_metrics(
    points: Vec<(f32, f32)>,
    ctx: &RouteContext<'_>,
    existing_segments: &[((f32, f32), (f32, f32))],
    use_existing: bool,
    candidates: &mut Vec<Vec<(f32, f32)>>,
    intersections: &mut Vec<usize>,
    crossings: &mut Vec<usize>,
    label_hits: &mut Vec<usize>,
    overlaps: &mut Vec<f32>,
) {
    if points.len() < 2 || !path_coords_reasonable(&points) {
        return;
    }
    let hits = path_obstacle_intersections(&points, ctx.obstacles, ctx.from_id, ctx.to_id);
    let labels = path_label_intersections(&points, ctx.label_obstacles);
    let (cross, overlap) = if use_existing {
        edge_crossings_with_existing(&points, existing_segments)
    } else {
        (0, 0.0)
    };
    candidates.push(points);
    intersections.push(hits);
    crossings.push(cross);
    label_hits.push(labels);
    overlaps.push(overlap);
}

fn path_coords_reasonable(points: &[(f32, f32)]) -> bool {
    const LIMIT: f32 = 100_000.0;
    points
        .iter()
        .all(|(x, y)| x.is_finite() && y.is_finite() && x.abs() <= LIMIT && y.abs() <= LIMIT)
}

fn route_edge_with_avoidance(
    ctx: &RouteContext<'_>,
    occupancy: Option<&EdgeOccupancy>,
    grid: Option<&RoutingGrid>,
    existing: Option<&[Segment]>,
) -> Vec<(f32, f32)> {
    if ctx.from_id == ctx.to_id {
        let existing_segments = existing.unwrap_or(&[]);
        let use_existing = !existing_segments.is_empty();
        let mut candidates: Vec<Vec<(f32, f32)>> = Vec::new();
        let mut intersections: Vec<usize> = Vec::new();
        let mut crossings: Vec<usize> = Vec::new();
        let mut label_hits: Vec<usize> = Vec::new();
        let mut overlaps: Vec<f32> = Vec::new();

        let pad = ctx.config.node_spacing.max(20.0) * 0.6;
        for points in route_self_loop_candidates(ctx.from, pad) {
            push_route_candidate_metrics(
                points,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );
        }
        push_route_candidate_metrics(
            route_self_loop(ctx.from, ctx.direction, ctx.config),
            ctx,
            existing_segments,
            use_existing,
            &mut candidates,
            &mut intersections,
            &mut crossings,
            &mut label_hits,
            &mut overlaps,
        );

        if candidates.is_empty() {
            return route_self_loop(ctx.from, ctx.direction, ctx.config);
        }

        let mut best_idx = 0usize;
        let mut best_hits = usize::MAX;
        let mut best_cross = usize::MAX;
        let mut best_label_hits = usize::MAX;
        let mut best_overlap = f32::MAX;
        let mut best_bends = usize::MAX;
        let mut best_len = f32::MAX;
        let mut best_score = u32::MAX;

        for (idx, points) in candidates.iter().enumerate() {
            let hits = intersections.get(idx).copied().unwrap_or(0);
            let cross = crossings.get(idx).copied().unwrap_or(0);
            let label = label_hits.get(idx).copied().unwrap_or(0);
            let overlap = overlaps.get(idx).copied().unwrap_or(0.0);
            let bends = path_bend_count(points);
            let len = path_length(points);
            let score = occupancy.map(|grid| grid.score_path(points)).unwrap_or(0);
            if hits < best_hits
                || (hits == best_hits && cross < best_cross)
                || (hits == best_hits && cross == best_cross && label < best_label_hits)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && bends < best_bends)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && bends == best_bends
                    && overlap < best_overlap)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && (overlap - best_overlap).abs() <= 1e-4
                    && bends == best_bends
                    && occupancy.is_some()
                    && score < best_score)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && (overlap - best_overlap).abs() <= 1e-4
                    && bends == best_bends
                    && (!occupancy.is_some() || score == best_score)
                    && len < best_len)
            {
                best_hits = hits;
                best_cross = cross;
                best_label_hits = label;
                best_overlap = overlap;
                best_bends = bends;
                best_len = len;
                best_score = score;
                best_idx = idx;
            }
        }

        return compress_path(&candidates.swap_remove(best_idx));
    }

    let (_, _, is_backward) = edge_sides(ctx.from, ctx.to, ctx.direction);

    // Anchor edges using resolved port offsets to reduce overlap
    let start = anchor_point_for_node(ctx.from, ctx.start_side, ctx.start_offset);
    let end = anchor_point_for_node(ctx.to, ctx.end_side, ctx.end_offset);
    let stub_len = port_stub_length(ctx.config, ctx.from, ctx.to);
    let mut route_start = port_stub_point(start, ctx.start_side, stub_len);
    let mut route_end = port_stub_point(end, ctx.end_side, stub_len);
    let stub_hits_node = |a: (f32, f32), b: (f32, f32)| {
        ctx.obstacles.iter().any(|obstacle| {
            if obstacle.members.is_some() {
                return false;
            }
            if obstacle.id == ctx.from_id || obstacle.id == ctx.to_id {
                return false;
            }
            segment_intersects_rect(a, b, obstacle)
        })
    };
    if ctx.obstacles.len() <= 10 {
        if stub_hits_node(start, route_start) {
            route_start = start;
        }
        if stub_hits_node(route_end, end) {
            route_end = end;
        }
    }
    if ctx.fast_route {
        return compress_path(&[start, route_start, route_end, end]);
    }
    let mut candidates: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut intersections: Vec<usize> = Vec::new();
    let mut crossings: Vec<usize> = Vec::new();
    let mut label_hits: Vec<usize> = Vec::new();
    let mut overlaps: Vec<f32> = Vec::new();
    let existing_segments = existing.unwrap_or(&[]);
    let use_existing = !existing_segments.is_empty();

    // For backward edges, try routing around obstacles (both left and right)
    if is_backward {
        let pad = ctx.config.node_spacing.max(30.0);
        // Find the extents of any obstacle that blocks the direct path
        let mut min_left = f32::MAX;
        let mut max_right = 0.0f32;
        for obstacle in ctx.obstacles {
            if obstacle.id == ctx.from_id || obstacle.id == ctx.to_id {
                continue;
            }
            if let Some(members) = &obstacle.members
                && (members.contains(ctx.from_id) || members.contains(ctx.to_id))
            {
                continue;
            }
            // Check if obstacle vertically overlaps the edge path
            let obs_top = obstacle.y;
            let obs_bottom = obstacle.y + obstacle.height;
            let path_top = end.1.min(start.1);
            let path_bottom = start.1.max(end.1);
            if obs_top < path_bottom && obs_bottom > path_top {
                min_left = min_left.min(obstacle.x);
                max_right = max_right.max(obstacle.x + obstacle.width);
            }
        }

        // Try routing around the right side first
        if max_right > 0.0 {
            let route_x = max_right + pad;
            let points = vec![
                route_start,
                (route_x, route_start.1),
                (route_x, route_end.1),
                route_end,
            ];
            push_route_candidate_metrics(
                points,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );
        }

        // Try routing around the left side
        if min_left < f32::MAX {
            let route_x = min_left - pad;
            let points = vec![
                route_start,
                (route_x, route_start.1),
                (route_x, route_end.1),
                route_end,
            ];
            push_route_candidate_metrics(
                points,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );
        }
    }

    // Check if a direct line is possible (no obstacles in the way)
    let direct_path = vec![route_start, route_end];
    push_route_candidate_metrics(
        direct_path,
        ctx,
        existing_segments,
        use_existing,
        &mut candidates,
        &mut intersections,
        &mut crossings,
        &mut label_hits,
        &mut overlaps,
    );

    // Fall back to orthogonal routing with control points
    let step = ctx.config.node_spacing.max(16.0) * 0.6;
    let mut offsets = vec![ctx.base_offset];
    for i in 1..=6 {
        let delta = step * i as f32;
        offsets.push(ctx.base_offset + delta);
        offsets.push(ctx.base_offset - delta);
    }

    let cross_axis_delta = if is_horizontal(ctx.direction) {
        (route_end.1 - route_start.1).abs()
    } else {
        (route_end.0 - route_start.0).abs()
    };
    let use_channel_candidates = cross_axis_delta > step * 0.75 && ctx.obstacles.len() > 10;

    for (offset_rank, offset) in offsets.iter().copied().enumerate() {
        if is_horizontal(ctx.direction) {
            let mid_x = (route_start.0 + route_end.0) / 2.0 + offset;
            let points = vec![
                route_start,
                (mid_x, route_start.1),
                (mid_x, route_end.1),
                route_end,
            ];
            push_route_candidate_metrics(
                points,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );

            let mid_y = (route_start.1 + route_end.1) / 2.0 + offset;
            let alt = vec![
                route_start,
                (route_start.0, mid_y),
                (route_end.0, mid_y),
                route_end,
            ];
            push_route_candidate_metrics(
                alt,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );

            if use_channel_candidates && offset_rank <= 3 {
                let near_start_x = route_start.0 + offset;
                let near_start = vec![
                    route_start,
                    (near_start_x, route_start.1),
                    (near_start_x, route_end.1),
                    route_end,
                ];
                push_route_candidate_metrics(
                    near_start,
                    ctx,
                    existing_segments,
                    use_existing,
                    &mut candidates,
                    &mut intersections,
                    &mut crossings,
                    &mut label_hits,
                    &mut overlaps,
                );

                let near_end_x = route_end.0 + offset;
                let near_end = vec![
                    route_start,
                    (near_end_x, route_start.1),
                    (near_end_x, route_end.1),
                    route_end,
                ];
                push_route_candidate_metrics(
                    near_end,
                    ctx,
                    existing_segments,
                    use_existing,
                    &mut candidates,
                    &mut intersections,
                    &mut crossings,
                    &mut label_hits,
                    &mut overlaps,
                );
            }
        } else {
            let mid_y = (route_start.1 + route_end.1) / 2.0 + offset;
            let points = vec![
                route_start,
                (route_start.0, mid_y),
                (route_end.0, mid_y),
                route_end,
            ];
            push_route_candidate_metrics(
                points,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );

            let mid_x = (route_start.0 + route_end.0) / 2.0 + offset;
            let alt = vec![
                route_start,
                (mid_x, route_start.1),
                (mid_x, route_end.1),
                route_end,
            ];
            push_route_candidate_metrics(
                alt,
                ctx,
                existing_segments,
                use_existing,
                &mut candidates,
                &mut intersections,
                &mut crossings,
                &mut label_hits,
                &mut overlaps,
            );

            if use_channel_candidates && offset_rank <= 3 {
                let near_start_y = route_start.1 + offset;
                let near_start = vec![
                    route_start,
                    (route_start.0, near_start_y),
                    (route_end.0, near_start_y),
                    route_end,
                ];
                push_route_candidate_metrics(
                    near_start,
                    ctx,
                    existing_segments,
                    use_existing,
                    &mut candidates,
                    &mut intersections,
                    &mut crossings,
                    &mut label_hits,
                    &mut overlaps,
                );

                let near_end_y = route_end.1 + offset;
                let near_end = vec![
                    route_start,
                    (route_start.0, near_end_y),
                    (route_end.0, near_end_y),
                    route_end,
                ];
                push_route_candidate_metrics(
                    near_end,
                    ctx,
                    existing_segments,
                    use_existing,
                    &mut candidates,
                    &mut intersections,
                    &mut crossings,
                    &mut label_hits,
                    &mut overlaps,
                );
            }
        }
    }

    let min_hits = intersections.iter().copied().min().unwrap_or(0);
    let min_crossings = crossings.iter().copied().min().unwrap_or(0);
    let min_label_hits = label_hits.iter().copied().min().unwrap_or(0);
    let mut needs_detour = min_crossings > 0 || min_label_hits > 0;
    if min_hits == 0
        && let Some(occ) = occupancy
    {
        let mut best_idx = 0usize;
        let mut best_score = u32::MAX;
        let mut best_bends = usize::MAX;
        let mut best_len = f32::MAX;
        for (idx, points) in candidates.iter().enumerate() {
            let score = occ.score_path(points);
            let bends = path_bend_count(points);
            let len = path_length(points);
            if score < best_score
                || (score == best_score && bends < best_bends)
                || (score == best_score && bends == best_bends && len < best_len)
            {
                best_score = score;
                best_bends = bends;
                best_len = len;
                best_idx = idx;
            }
        }
        if let Some(points) = candidates.get(best_idx) {
            let overlap = occ.overlap_count(points);
            let path_len = path_length(points);
            let overlap_trigger = ((path_len / occ.cell) * 0.35).max(4.0).ceil() as u32;
            if overlap >= overlap_trigger {
                needs_detour = true;
            }
        }
    }

    if min_hits > 0 || needs_detour {
        for i in 7..=9 {
            let delta = step * i as f32;
            for sign in [1.0, -1.0] {
                let offset = ctx.base_offset + sign * delta;
                let points = if is_horizontal(ctx.direction) {
                    let mid_x = (route_start.0 + route_end.0) / 2.0 + offset;
                    vec![
                        route_start,
                        (mid_x, route_start.1),
                        (mid_x, route_end.1),
                        route_end,
                    ]
                } else {
                    let mid_y = (route_start.1 + route_end.1) / 2.0 + offset;
                    vec![
                        route_start,
                        (route_start.0, mid_y),
                        (route_end.0, mid_y),
                        route_end,
                    ]
                };
                push_route_candidate_metrics(
                    points,
                    ctx,
                    existing_segments,
                    use_existing,
                    &mut candidates,
                    &mut intersections,
                    &mut crossings,
                    &mut label_hits,
                    &mut overlaps,
                );
            }
        }
    }

    let min_hits = intersections.iter().copied().min().unwrap_or(0);
    if (min_hits > 0 || needs_detour)
        && let Some(grid) = grid
        && let Some(points) = route_edge_with_grid(ctx, grid, occupancy, route_start, route_end)
    {
        push_route_candidate_metrics(
            points,
            ctx,
            existing_segments,
            use_existing,
            &mut candidates,
            &mut intersections,
            &mut crossings,
            &mut label_hits,
            &mut overlaps,
        );
    }

    if let Some(grid) = occupancy {
        let mut best_idx = 0usize;
        let mut best_hits = usize::MAX;
        let mut best_cross = usize::MAX;
        let mut best_label_hits = usize::MAX;
        let mut best_overlap = f32::MAX;
        let mut best_bends = usize::MAX;
        let mut best_score = u32::MAX;
        let mut best_len = f32::MAX;
        for (idx, points) in candidates.iter().enumerate() {
            let hits = intersections.get(idx).copied().unwrap_or(0);
            let cross = crossings.get(idx).copied().unwrap_or(0);
            let label = label_hits.get(idx).copied().unwrap_or(0);
            let overlap = overlaps.get(idx).copied().unwrap_or(0.0);
            let bends = path_bend_count(points);
            let score = grid.score_path(points);
            let len = path_length(points);
            if hits < best_hits
                || (hits == best_hits && cross < best_cross)
                || (hits == best_hits && cross == best_cross && label < best_label_hits)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && bends < best_bends)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && bends == best_bends
                    && overlap < best_overlap)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && (overlap - best_overlap).abs() <= 1e-4
                    && bends == best_bends
                    && score < best_score)
                || (hits == best_hits
                    && cross == best_cross
                    && label == best_label_hits
                    && (overlap - best_overlap).abs() <= 1e-4
                    && bends == best_bends
                    && score == best_score
                    && len < best_len)
            {
                best_hits = hits;
                best_cross = cross;
                best_label_hits = label;
                best_overlap = overlap;
                best_bends = bends;
                best_score = score;
                best_len = len;
                best_idx = idx;
            }
        }
        let mut combined = Vec::with_capacity(candidates[best_idx].len() + 2);
        combined.push(start);
        combined.extend(candidates.swap_remove(best_idx));
        combined.push(end);
        return compress_path(&combined);
    }

    let mut best_idx = 0usize;
    let mut best_hits = usize::MAX;
    let mut best_cross = usize::MAX;
    let mut best_label_hits = usize::MAX;
    let mut best_overlap = f32::MAX;
    let mut best_bends = usize::MAX;
    let mut best_len = f32::MAX;
    for (idx, points) in candidates.iter().enumerate() {
        let hits = intersections.get(idx).copied().unwrap_or(0);
        let cross = crossings.get(idx).copied().unwrap_or(0);
        let label = label_hits.get(idx).copied().unwrap_or(0);
        let overlap = overlaps.get(idx).copied().unwrap_or(0.0);
        let bends = path_bend_count(points);
        let len = path_length(points);
        if hits < best_hits
            || (hits == best_hits && cross < best_cross)
            || (hits == best_hits && cross == best_cross && label < best_label_hits)
            || (hits == best_hits
                && cross == best_cross
                && label == best_label_hits
                && bends < best_bends)
            || (hits == best_hits
                && cross == best_cross
                && label == best_label_hits
                && bends == best_bends
                && overlap < best_overlap)
            || (hits == best_hits
                && cross == best_cross
                && label == best_label_hits
                && (overlap - best_overlap).abs() <= 1e-4
                && bends == best_bends
                && len < best_len)
        {
            best_hits = hits;
            best_cross = cross;
            best_label_hits = label;
            best_overlap = overlap;
            best_bends = bends;
            best_len = len;
            best_idx = idx;
        }
    }
    let mut combined = Vec::with_capacity(candidates[best_idx].len() + 2);
    combined.push(start);
    combined.extend(candidates.swap_remove(best_idx));
    combined.push(end);
    compress_path(&combined)
}

fn path_obstacle_intersections(
    points: &[(f32, f32)],
    obstacles: &[Obstacle],
    from_id: &str,
    to_id: &str,
) -> usize {
    if points.len() < 2 {
        return 0;
    }
    let mut count = 0usize;
    for segment in points.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        for obstacle in obstacles {
            if obstacle.id == from_id || obstacle.id == to_id {
                continue;
            }
            if let Some(members) = &obstacle.members
                && (members.contains(from_id) || members.contains(to_id))
            {
                continue;
            }
            if segment_intersects_rect(a, b, obstacle) {
                count += 1;
            }
        }
    }
    count
}

fn path_label_intersections(points: &[(f32, f32)], label_obstacles: &[Obstacle]) -> usize {
    if points.len() < 2 || label_obstacles.is_empty() {
        return 0;
    }
    let mut count = 0usize;
    for segment in points.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        for obstacle in label_obstacles {
            if segment_intersects_rect(a, b, obstacle) {
                count += 1;
            }
        }
    }
    count
}

fn path_length(points: &[(f32, f32)]) -> f32 {
    let mut length = 0.0;
    for segment in points.windows(2) {
        let dx = segment[1].0 - segment[0].0;
        let dy = segment[1].1 - segment[0].1;
        length += (dx * dx + dy * dy).sqrt();
    }
    length
}

fn path_bend_count(points: &[(f32, f32)]) -> usize {
    if points.len() < 3 {
        return 0;
    }
    let mut bends = 0usize;
    for idx in 1..points.len() - 1 {
        let p0 = points[idx - 1];
        let p1 = points[idx];
        let p2 = points[idx + 1];
        let dx1 = p1.0 - p0.0;
        let dy1 = p1.1 - p0.1;
        let dx2 = p2.0 - p1.0;
        let dy2 = p2.1 - p1.1;
        if (dx1.abs() <= 1e-4 && dy1.abs() <= 1e-4) || (dx2.abs() <= 1e-4 && dy2.abs() <= 1e-4) {
            continue;
        }
        let cross = dx1 * dy2 - dy1 * dx2;
        if cross.abs() > 1e-4 {
            bends += 1;
        }
    }
    bends
}

fn edge_label_anchor_from_points(points: &[(f32, f32)]) -> Option<(f32, f32)> {
    if points.len() < 2 {
        return None;
    }
    let segment_count = points.len() - 1;
    let (start_idx, end_idx) = if segment_count >= 3 {
        (1, segment_count - 1)
    } else {
        (0, segment_count)
    };
    let mut best_idx = None;
    let mut best_len = 0.0f32;
    for idx in start_idx..end_idx {
        let p1 = points[idx];
        let p2 = points[idx + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let len = dx * dx + dy * dy;
        if len > best_len {
            best_len = len;
            best_idx = Some(idx);
        }
    }
    if best_idx.is_none() {
        for idx in 0..segment_count {
            let p1 = points[idx];
            let p2 = points[idx + 1];
            let dx = p2.0 - p1.0;
            let dy = p2.1 - p1.1;
            let len = dx * dx + dy * dy;
            if len > best_len {
                best_len = len;
                best_idx = Some(idx);
            }
        }
    }
    let idx = best_idx.unwrap_or(0);
    let p1 = points[idx];
    let p2 = points[idx + 1];
    Some(((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0))
}

fn route_self_loop(
    node: &NodeLayout,
    direction: Direction,
    config: &LayoutConfig,
) -> Vec<(f32, f32)> {
    let pad = config.node_spacing.max(20.0) * 0.6;
    if is_horizontal(direction) {
        let start = (node.x + node.width, node.y + node.height / 2.0);
        let p1 = (node.x + node.width + pad, node.y + node.height / 2.0);
        let p2 = (node.x + node.width + pad, node.y - pad);
        let p3 = (node.x + node.width / 2.0, node.y - pad);
        let end = (node.x + node.width / 2.0, node.y);
        vec![start, p1, p2, p3, end]
    } else {
        let start = (node.x + node.width / 2.0, node.y + node.height);
        let p1 = (node.x + node.width / 2.0, node.y + node.height + pad);
        let p2 = (node.x + node.width + pad, node.y + node.height + pad);
        let p3 = (node.x + node.width + pad, node.y + node.height / 2.0);
        let end = (node.x + node.width, node.y + node.height / 2.0);
        vec![start, p1, p2, p3, end]
    }
}

fn route_self_loop_candidates(node: &NodeLayout, pad: f32) -> Vec<Vec<(f32, f32)>> {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let left = (x, cy);
    let right = (x + w, cy);
    let top = (cx, y);
    let bottom = (cx, y + h);
    let left_x = x - pad;
    let right_x = x + w + pad;
    let top_y = y - pad;
    let bottom_y = y + h + pad;

    vec![
        // Right-side loops
        vec![right, (right_x, cy), (right_x, top_y), (cx, top_y), top],
        vec![
            right,
            (right_x, cy),
            (right_x, bottom_y),
            (cx, bottom_y),
            bottom,
        ],
        // Left-side loops
        vec![left, (left_x, cy), (left_x, top_y), (cx, top_y), top],
        vec![
            left,
            (left_x, cy),
            (left_x, bottom_y),
            (cx, bottom_y),
            bottom,
        ],
        // Top-side loops
        vec![top, (cx, top_y), (right_x, top_y), (right_x, cy), right],
        vec![top, (cx, top_y), (left_x, top_y), (left_x, cy), left],
        // Bottom-side loops
        vec![
            bottom,
            (cx, bottom_y),
            (right_x, bottom_y),
            (right_x, cy),
            right,
        ],
        vec![
            bottom,
            (cx, bottom_y),
            (left_x, bottom_y),
            (left_x, cy),
            left,
        ],
    ]
}

fn build_obstacles(
    nodes: &BTreeMap<String, NodeLayout>,
    subgraphs: &[SubgraphLayout],
    config: &LayoutConfig,
) -> Vec<Obstacle> {
    let mut obstacles = Vec::new();
    let pad = (config.node_spacing * 0.35).max(6.0);
    for node in nodes.values() {
        if node.hidden {
            continue;
        }
        if node.anchor_subgraph.is_some() {
            continue;
        }
        obstacles.push(Obstacle {
            id: node.id.clone(),
            x: node.x - pad,
            y: node.y - pad,
            width: node.width + pad * 2.0,
            height: node.height + pad * 2.0,
            members: None,
        });
    }

    for (idx, sub) in subgraphs.iter().enumerate() {
        let invisible_region = sub.label.trim().is_empty()
            && sub.style.stroke.as_deref() == Some("none")
            && sub.style.fill.as_deref() == Some("none");
        if invisible_region {
            continue;
        }
        let mut members: HashSet<String> = sub.nodes.iter().cloned().collect();
        for node in nodes.values() {
            if node.anchor_subgraph == Some(idx) {
                members.insert(node.id.clone());
            }
        }
        obstacles.push(Obstacle {
            id: format!("subgraph:{}", sub.label),
            x: sub.x - pad,
            y: sub.y - pad,
            width: sub.width + pad * 2.0,
            height: sub.height + pad * 2.0,
            members: Some(members),
        });
    }
    obstacles
}

fn build_label_obstacles_for_routing(
    nodes: &BTreeMap<String, NodeLayout>,
    subgraphs: &[SubgraphLayout],
) -> Vec<Obstacle> {
    let mut obstacles = Vec::new();

    let node_pad = 2.0;
    for node in nodes.values() {
        if node.hidden || node.anchor_subgraph.is_some() {
            continue;
        }
        if node.label.width <= 0.0
            || node.label.height <= 0.0
            || node.label.lines.iter().all(|line| line.trim().is_empty())
        {
            continue;
        }
        let x = node.x + (node.width - node.label.width) / 2.0 - node_pad;
        let y = node.y + (node.height - node.label.height) / 2.0 - node_pad;
        obstacles.push(Obstacle {
            id: format!("node-label:{}", node.id),
            x,
            y,
            width: node.label.width + node_pad * 2.0,
            height: node.label.height + node_pad * 2.0,
            members: None,
        });
    }

    let sub_pad = 3.0;
    for sub in subgraphs {
        if sub.label.trim().is_empty()
            || sub.label_block.width <= 0.0
            || sub.label_block.height <= 0.0
        {
            continue;
        }
        // Approximate the header label box as rendered in flowchart/subgraph mode.
        let x = sub.x + 12.0 - sub_pad;
        let y = sub.y + 6.0 - sub_pad;
        obstacles.push(Obstacle {
            id: format!("subgraph-label:{}", sub.label),
            x,
            y,
            width: sub.label_block.width + sub_pad * 2.0,
            height: sub.label_block.height + sub_pad * 2.0,
            members: None,
        });
    }

    obstacles
}

fn edge_pair_key(edge: &crate::mermaid_renderer::ir::Edge) -> (String, String) {
    if edge.from <= edge.to {
        (edge.from.clone(), edge.to.clone())
    } else {
        (edge.to.clone(), edge.from.clone())
    }
}

fn build_edge_pair_counts(
    edges: &[crate::mermaid_renderer::ir::Edge],
) -> HashMap<(String, String), usize> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();
    for edge in edges {
        let key = edge_pair_key(edge);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn segment_intersects_rect(a: (f32, f32), b: (f32, f32), rect: &Obstacle) -> bool {
    let (x1, y1) = a;
    let (x2, y2) = b;
    let min_x = x1.min(x2);
    let max_x = x1.max(x2);
    let min_y = y1.min(y2);
    let max_y = y1.max(y2);
    if max_x < rect.x
        || min_x > rect.x + rect.width
        || max_y < rect.y
        || min_y > rect.y + rect.height
    {
        return false;
    }
    if x1 >= rect.x && x1 <= rect.x + rect.width && y1 >= rect.y && y1 <= rect.y + rect.height {
        return true;
    }
    if x2 >= rect.x && x2 <= rect.x + rect.width && y2 >= rect.y && y2 <= rect.y + rect.height {
        return true;
    }
    let corners = [
        (rect.x, rect.y),
        (rect.x + rect.width, rect.y),
        (rect.x + rect.width, rect.y + rect.height),
        (rect.x, rect.y + rect.height),
    ];
    let edges = [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ];
    for (c, d) in edges {
        if segments_intersect(a, b, c, d) {
            return true;
        }
    }
    false
}

type Segment = ((f32, f32), (f32, f32));

fn segments_intersect(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> bool {
    fn orient(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }
    fn on_segment(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
        let min_x = a.0.min(b.0);
        let max_x = a.0.max(b.0);
        let min_y = a.1.min(b.1);
        let max_y = a.1.max(b.1);
        c.0 >= min_x - 1e-6 && c.0 <= max_x + 1e-6 && c.1 >= min_y - 1e-6 && c.1 <= max_y + 1e-6
    }
    let o1 = orient(a, b, c);
    let o2 = orient(a, b, d);
    let o3 = orient(c, d, a);
    let o4 = orient(c, d, b);
    if (o1 > 0.0 && o2 < 0.0 || o1 < 0.0 && o2 > 0.0)
        && (o3 > 0.0 && o4 < 0.0 || o3 < 0.0 && o4 > 0.0)
    {
        return true;
    }
    if o1.abs() <= 1e-6 && on_segment(a, b, c) {
        return true;
    }
    if o2.abs() <= 1e-6 && on_segment(a, b, d) {
        return true;
    }
    if o3.abs() <= 1e-6 && on_segment(c, d, a) {
        return true;
    }
    if o4.abs() <= 1e-6 && on_segment(c, d, b) {
        return true;
    }
    false
}

fn collinear_overlap_length(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> f32 {
    let cross1 = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let cross2 = (b.0 - a.0) * (d.1 - a.1) - (b.1 - a.1) * (d.0 - a.0);
    if cross1.abs() > 1e-6 || cross2.abs() > 1e-6 {
        return 0.0;
    }
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let seg_len_sq = dx * dx + dy * dy;
    if seg_len_sq < 1e-6 {
        return 0.0;
    }
    let proj = |p: (f32, f32)| ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / seg_len_sq;
    let t1 = proj(c);
    let t2 = proj(d);
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    let overlap = (tmax.min(1.0) - tmin.max(0.0)).max(0.0);
    overlap * seg_len_sq.sqrt()
}

fn edge_crossings_with_existing(points: &[(f32, f32)], existing: &[Segment]) -> (usize, f32) {
    if points.len() < 2 || existing.is_empty() {
        return (0, 0.0);
    }
    let mut crossings = 0usize;
    let mut overlap = 0.0f32;
    for segment in points.windows(2) {
        let a1 = segment[0];
        let a2 = segment[1];
        for &(b1, b2) in existing {
            if (a1.0 - b1.0).abs() < 1e-6 && (a1.1 - b1.1).abs() < 1e-6
                || (a1.0 - b2.0).abs() < 1e-6 && (a1.1 - b2.1).abs() < 1e-6
                || (a2.0 - b1.0).abs() < 1e-6 && (a2.1 - b1.1).abs() < 1e-6
                || (a2.0 - b2.0).abs() < 1e-6 && (a2.1 - b2.1).abs() < 1e-6
            {
                continue;
            }
            overlap += collinear_overlap_length(a1, a2, b1, b2);
            if segments_intersect(a1, a2, b1, b2) {
                crossings += 1;
            }
        }
    }
    (crossings, overlap)
}

fn measure_label(text: &str, theme: &Theme, config: &LayoutConfig) -> TextBlock {
    // Mermaid's layout sizing appears to use a baseline font size (~16px)
    // even when the configured theme font size is smaller. Using that
    // baseline improves parity with mermaid-cli node sizes.
    let measure_font_size = theme.font_size.max(16.0);
    measure_label_with_font_size(
        text,
        measure_font_size,
        config,
        true,
        theme.font_family.as_str(),
    )
}

fn measure_label_with_font_size(
    text: &str,
    font_size: f32,
    config: &LayoutConfig,
    wrap: bool,
    font_family: &str,
) -> TextBlock {
    let raw_lines = split_lines(text);
    let mut lines = Vec::new();
    let fast_metrics = config.fast_text_metrics;
    let max_width_px = max_label_width_px(
        config.max_label_width_chars,
        font_size,
        font_family,
        fast_metrics,
    );
    for line in raw_lines {
        if wrap {
            let wrapped = wrap_line(&line, max_width_px, font_size, font_family, fast_metrics);
            lines.extend(wrapped);
        } else {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    let max_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);
    let max_width = lines
        .iter()
        .map(|line| text_width(line, font_size, font_family, fast_metrics))
        .fold(0.0, f32::max);
    let avg_char = average_char_width(font_family, font_size, fast_metrics);
    let guard_width = max_len as f32 * avg_char;
    let width = max_width.max(guard_width);
    let height = lines.len() as f32 * font_size * config.label_line_height;

    TextBlock {
        lines,
        width,
        height,
    }
}

fn char_width_factor(ch: char) -> f32 {
    // Calibrated per-character widths against mermaid-cli output using the
    // default font stack and a 16px measurement baseline.
    match ch {
        ' ' => 0.306,
        '\\' | '.' | ',' | ':' | ';' | '|' | '!' | '(' | ')' | '[' | ']' | '{' | '}' => 0.321,
        'A' => 0.652,
        'B' => 0.648,
        'C' => 0.734,
        'D' => 0.723,
        'E' => 0.594,
        'F' => 0.575,
        'G' | 'H' => 0.742,
        'I' => 0.272,
        'J' => 0.557,
        'K' => 0.648,
        'L' => 0.559,
        'M' => 0.903,
        'N' => 0.763,
        'O' => 0.754,
        'P' => 0.623,
        'Q' => 0.755,
        'R' => 0.637,
        'S' => 0.633,
        'T' => 0.599,
        'U' => 0.746,
        'V' => 0.661,
        'W' => 0.958,
        'X' => 0.655,
        'Y' => 0.646,
        'Z' => 0.621,
        'a' => 0.550,
        'b' => 0.603,
        'c' => 0.547,
        'd' => 0.609,
        'e' => 0.570,
        'f' => 0.340,
        'g' | 'h' => 0.600,
        'i' => 0.235,
        'j' => 0.227,
        'k' => 0.522,
        'l' => 0.239,
        'm' => 0.867,
        'n' => 0.585,
        'o' => 0.574,
        'p' => 0.595,
        'q' => 0.585,
        'r' => 0.364,
        's' => 0.523,
        't' => 0.305,
        'u' => 0.585,
        'v' => 0.545,
        'w' => 0.811,
        'x' => 0.538,
        'y' => 0.556,
        'z' => 0.550,
        '0' => 0.613,
        '1' => 0.396,
        '2' => 0.609,
        '3' => 0.597,
        '4' => 0.614,
        '5' => 0.586,
        '6' => 0.608,
        '7' => 0.559,
        '8' => 0.611,
        '9' => 0.595,
        '@' | '#' | '%' | '&' => 0.946,
        _ => 0.568,
    }
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = text.replace("<br/>", "\n").replace("<br>", "\n");
    current = current.replace("\\n", "\n");
    for line in current.split('\n') {
        lines.push(line.trim().to_string());
    }
    lines
}

fn wrap_line(
    line: &str,
    max_width: f32,
    font_size: f32,
    font_family: &str,
    fast_metrics: bool,
) -> Vec<String> {
    if text_width(line, font_size, font_family, fast_metrics) <= max_width {
        return vec![line.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if text_width(&candidate, font_size, font_family, fast_metrics) > max_width {
            if !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }
            current.push_str(word);
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn text_width(text: &str, font_size: f32, font_family: &str, fast_metrics: bool) -> f32 {
    if fast_metrics && text.is_ascii() {
        return fallback_text_width(text, font_size);
    }
    text_metrics::measure_text_width(text, font_size, font_family)
        .unwrap_or_else(|| fallback_text_width(text, font_size))
}

fn fallback_text_width(text: &str, font_size: f32) -> f32 {
    text.chars().map(char_width_factor).sum::<f32>() * font_size
}

fn average_char_width(font_family: &str, font_size: f32, fast_metrics: bool) -> f32 {
    if fast_metrics {
        return font_size * 0.56;
    }
    text_metrics::average_char_width(font_family, font_size).unwrap_or(font_size * 0.56)
}

fn max_label_width_px(
    max_chars: usize,
    font_size: f32,
    font_family: &str,
    fast_metrics: bool,
) -> f32 {
    let avg_char = average_char_width(font_family, font_size, fast_metrics);
    (max_chars.max(1) as f32) * avg_char
}

fn resolve_node_style(node_id: &str, graph: &Graph) -> crate::mermaid_renderer::ir::NodeStyle {
    let mut style = crate::mermaid_renderer::ir::NodeStyle::default();

    if let Some(classes) = graph.node_classes.get(node_id) {
        for class_name in classes {
            if let Some(class_style) = graph.class_defs.get(class_name) {
                merge_node_style(&mut style, class_style);
            }
        }
    }

    if let Some(node_style) = graph.node_styles.get(node_id) {
        merge_node_style(&mut style, node_style);
    }

    style
}

fn resolve_subgraph_style(
    sub: &crate::mermaid_renderer::ir::Subgraph,
    graph: &Graph,
) -> crate::mermaid_renderer::ir::NodeStyle {
    let mut style = crate::mermaid_renderer::ir::NodeStyle::default();
    let Some(id) = sub.id.as_ref() else {
        return style;
    };

    if let Some(classes) = graph.subgraph_classes.get(id) {
        for class_name in classes {
            if let Some(class_style) = graph.class_defs.get(class_name) {
                merge_node_style(&mut style, class_style);
            }
        }
    }

    if let Some(sub_style) = graph.subgraph_styles.get(id) {
        merge_node_style(&mut style, sub_style);
    }

    style
}

/// Enforce a minimum gap between top-level subgraphs along the main axis.
fn enforce_top_level_subgraph_gap(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart
        || graph.subgraphs.len() < 2
    {
        return;
    }

    let top_level = top_level_subgraph_indices(graph);
    if top_level.len() < 2 {
        return;
    }

    // Only attempt this when top-level subgraphs are disjoint to avoid
    // double-shifting shared nodes.
    let mut seen: HashSet<&str> = HashSet::new();
    for &idx in &top_level {
        for node_id in &graph.subgraphs[idx].nodes {
            if !seen.insert(node_id.as_str()) {
                return;
            }
        }
    }

    // If no edges connect top-level subgraphs, skip this function.
    // Let `separate_sibling_subgraphs` handle them on the cross axis instead.
    let node_to_top_level_sg: HashMap<&str, usize> = top_level
        .iter()
        .flat_map(|&idx| {
            graph.subgraphs[idx]
                .nodes
                .iter()
                .map(move |n| (n.as_str(), idx))
        })
        .collect();
    let has_cross_sg_edge = graph.edges.iter().any(|e| {
        let from_sg = node_to_top_level_sg.get(e.from.as_str());
        let to_sg = node_to_top_level_sg.get(e.to.as_str());
        matches!((from_sg, to_sg), (Some(a), Some(b)) if a != b)
    });
    if !has_cross_sg_edge {
        return;
    }

    #[derive(Clone, Copy)]
    struct Bounds {
        idx: usize,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        pad_main: f32,
    }

    let direction = graph.direction;
    let horizontal = is_horizontal(direction);
    let mut bounds: Vec<Bounds> = Vec::new();

    for &idx in &top_level {
        let sub = &graph.subgraphs[idx];
        if is_region_subgraph(sub) || sub.nodes.is_empty() {
            continue;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
                max_x = max_x.max(node.x + node.width);
                max_y = max_y.max(node.y + node.height);
            }
        }
        if min_x == f32::MAX {
            continue;
        }

        let label_empty = sub.label.trim().is_empty();
        let mut label_block = measure_label(&sub.label, theme, config);
        if label_empty {
            label_block.width = 0.0;
            label_block.height = 0.0;
        }
        let (pad_x, pad_y, top_padding) =
            subgraph_padding_from_label(graph, sub, theme, &label_block);

        let padded_min_x = min_x - pad_x;
        let padded_max_x = max_x + pad_x;
        let padded_min_y = min_y - top_padding;
        let padded_max_y = max_y + pad_y;
        let pad_main = if horizontal { pad_x } else { pad_y };

        bounds.push(Bounds {
            idx,
            min_x: padded_min_x,
            min_y: padded_min_y,
            max_x: padded_max_x,
            max_y: padded_max_y,
            pad_main,
        });
    }

    if bounds.len() < 2 {
        return;
    }

    bounds.sort_by(|a, b| {
        let a_key = if horizontal { a.min_x } else { a.min_y };
        let b_key = if horizontal { b.min_x } else { b.min_y };
        a_key
            .partial_cmp(&b_key)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.idx.cmp(&b.idx))
    });

    let pad_main = bounds.iter().map(|b| b.pad_main).fold(0.0_f32, f32::max);
    let desired_gap = (config.node_spacing * 1.6).max(pad_main * 2.0);

    let mut prev_max_main: Option<f32> = None;
    for bound in &mut bounds {
        let min_main = if horizontal { bound.min_x } else { bound.min_y };
        let mut max_main = if horizontal { bound.max_x } else { bound.max_y };

        let mut delta = 0.0_f32;
        if let Some(prev_max) = prev_max_main {
            let required_min = prev_max + desired_gap;
            if min_main < required_min {
                delta = required_min - min_main;
            }
        }

        if delta > 0.0 {
            let sub = &graph.subgraphs[bound.idx];
            for node_id in &sub.nodes {
                if let Some(node) = nodes.get_mut(node_id) {
                    if horizontal {
                        node.x += delta;
                    } else {
                        node.y += delta;
                    }
                }
            }

            if horizontal {
                bound.min_x += delta;
                bound.max_x += delta;
            } else {
                bound.min_y += delta;
                bound.max_y += delta;
            }

            max_main += delta;
        }

        prev_max_main = Some(max_main);
    }
}

/// Separate sibling subgraphs that don't share nodes to avoid overlap
fn separate_sibling_subgraphs(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) {
    if graph.subgraphs.len() < 2 {
        return;
    }

    // Build node sets for each subgraph
    let sets: Vec<HashSet<String>> = graph
        .subgraphs
        .iter()
        .map(|sub| sub.nodes.iter().cloned().collect())
        .collect();

    // Find pairs of sibling subgraphs (non-overlapping node sets)
    let mut sibling_groups: Vec<Vec<usize>> = Vec::new();
    let mut assigned: HashSet<usize> = HashSet::new();

    for i in 0..graph.subgraphs.len() {
        if assigned.contains(&i) {
            continue;
        }
        let mut group = vec![i];
        assigned.insert(i);

        for j in (i + 1)..graph.subgraphs.len() {
            if assigned.contains(&j) {
                continue;
            }
            // Check if j is a sibling (not nested with any in group)
            let j_set = &sets[j];
            let is_sibling = group.iter().all(|&k| {
                let k_set = &sets[k];
                // Neither is subset of the other
                !j_set.is_subset(k_set) && !k_set.is_subset(j_set)
            });
            if is_sibling {
                group.push(j);
                assigned.insert(j);
            }
        }
        if group.len() > 1 {
            sibling_groups.push(group);
        }
    }

    // For each group of siblings, compute bounds and separate them
    let is_horizontal = is_horizontal(graph.direction);
    for group in sibling_groups {
        // Compute bounding box for each subgraph
        let mut bounds: Vec<(usize, f32, f32, f32, f32)> = Vec::new(); // (idx, min_x, min_y, max_x, max_y)
        for &idx in &group {
            let sub = &graph.subgraphs[idx];
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            for node_id in &sub.nodes {
                if let Some(node) = nodes.get(node_id) {
                    min_x = min_x.min(node.x);
                    min_y = min_y.min(node.y);
                    max_x = max_x.max(node.x + node.width);
                    max_y = max_y.max(node.y + node.height);
                }
            }
            if min_x != f32::MAX {
                // Include subgraph padding in bounds calculation
                let label_block = measure_label(&sub.label, theme, config);
                let (pad_x, pad_y, top_padding) =
                    subgraph_padding_from_label(graph, sub, theme, &label_block);
                let padded_min_x = min_x - pad_x;
                let padded_min_y = min_y - top_padding;
                let padded_max_x = max_x + pad_x;
                let padded_max_y = max_y + pad_y;
                bounds.push((idx, padded_min_x, padded_min_y, padded_max_x, padded_max_y));
            }
        }

        if bounds.len() < 2 {
            continue;
        }

        // Sort by position along the separation axis for stable, deterministic shifts.
        if is_horizontal {
            bounds.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal));
        } else {
            bounds.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        }

        let gap = config.node_spacing.max(8.0);
        let overlaps =
            |a_min: f32, a_max: f32, b_min: f32, b_max: f32| a_min < b_max && b_min < a_max;

        let mut placed: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
        for (idx, min_x, min_y, max_x, max_y) in bounds {
            let mut shift = 0.0_f32;

            for &(_, px1, py1, px2, py2) in &placed {
                let other_axis_overlaps = if is_horizontal {
                    overlaps(min_x, max_x, px1, px2)
                } else {
                    overlaps(min_y, max_y, py1, py2)
                };
                if !other_axis_overlaps {
                    continue;
                }

                let shifted_min = if is_horizontal {
                    min_y + shift
                } else {
                    min_x + shift
                };
                let shifted_max = if is_horizontal {
                    max_y + shift
                } else {
                    max_x + shift
                };
                let placed_min = if is_horizontal { py1 } else { px1 };
                let placed_max = if is_horizontal { py2 } else { px2 };

                if overlaps(shifted_min, shifted_max, placed_min, placed_max) {
                    let needed = placed_max + gap - shifted_min;
                    if needed > shift {
                        shift = needed;
                    }
                }
            }

            if shift > 0.0 {
                let sub = &graph.subgraphs[idx];
                for node_id in &sub.nodes {
                    if let Some(node) = nodes.get_mut(node_id) {
                        if is_horizontal {
                            node.y += shift;
                        } else {
                            node.x += shift;
                        }
                    }
                }
            }

            let shifted_bounds = if is_horizontal {
                (idx, min_x, min_y + shift, max_x, max_y + shift)
            } else {
                (idx, min_x + shift, min_y, max_x + shift, max_y)
            };
            placed.push(shifted_bounds);
        }
    }
}

fn align_disconnected_top_level_subgraphs(graph: &Graph, nodes: &mut BTreeMap<String, NodeLayout>) {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart
        || graph.subgraphs.len() < 2
    {
        return;
    }

    let top_level = top_level_subgraph_indices(graph);
    if top_level.len() < 2 {
        return;
    }

    let mut seen: HashSet<&str> = HashSet::new();
    let mut union_count = 0usize;
    for &idx in &top_level {
        let sub = &graph.subgraphs[idx];
        for node_id in &sub.nodes {
            if !seen.insert(node_id.as_str()) {
                return;
            }
            union_count += 1;
        }
        if let Some(anchor_id) = subgraph_anchor_id(sub, nodes) {
            if !seen.insert(anchor_id) {
                return;
            }
            union_count += 1;
        }
    }
    if union_count != graph.nodes.len() {
        return;
    }

    let mut node_to_top_level: HashMap<&str, usize> = HashMap::new();
    for &idx in &top_level {
        let sub = &graph.subgraphs[idx];
        for node_id in &sub.nodes {
            node_to_top_level.insert(node_id.as_str(), idx);
        }
        if let Some(anchor_id) = subgraph_anchor_id(sub, nodes) {
            node_to_top_level.insert(anchor_id, idx);
        }
    }
    let has_cross_edges = graph.edges.iter().any(|edge| {
        let from = node_to_top_level.get(edge.from.as_str());
        let to = node_to_top_level.get(edge.to.as_str());
        matches!((from, to), (Some(a), Some(b)) if a != b)
    });
    if has_cross_edges {
        return;
    }

    #[derive(Clone)]
    struct Bounds {
        idx: usize,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        anchor_id: Option<String>,
    }

    let mut bounds: Vec<Bounds> = Vec::new();
    for &idx in &top_level {
        let sub = &graph.subgraphs[idx];
        if sub.nodes.is_empty() {
            continue;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
                max_x = max_x.max(node.x + node.width);
                max_y = max_y.max(node.y + node.height);
            }
        }
        let anchor_id = subgraph_anchor_id(sub, nodes).map(|id| id.to_string());
        if let Some(anchor) = anchor_id.as_deref().and_then(|id| nodes.get(id)) {
            min_x = min_x.min(anchor.x);
            min_y = min_y.min(anchor.y);
            max_x = max_x.max(anchor.x + anchor.width);
            max_y = max_y.max(anchor.y + anchor.height);
        }
        if min_x == f32::MAX {
            continue;
        }
        bounds.push(Bounds {
            idx,
            min_x,
            min_y,
            max_x,
            max_y,
            anchor_id,
        });
    }

    if bounds.len() < 2 {
        return;
    }

    let horizontal = is_horizontal(graph.direction);
    bounds.sort_by(|a, b| {
        let a_key = if horizontal { a.min_x } else { a.min_y };
        let b_key = if horizontal { b.min_x } else { b.min_y };
        a_key
            .partial_cmp(&b_key)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.idx.cmp(&b.idx))
    });

    let mut prev_max: Option<f32> = None;
    for bound in &bounds {
        let min_main = if horizontal { bound.min_x } else { bound.min_y };
        let max_main = if horizontal { bound.max_x } else { bound.max_y };
        if let Some(prev) = prev_max {
            if min_main < prev {
                return;
            }
        }
        prev_max = Some(max_main);
    }

    let target_cross = bounds
        .iter()
        .map(|b| if horizontal { b.min_y } else { b.min_x })
        .fold(f32::MAX, f32::min);

    for bound in bounds {
        let current_cross = if horizontal { bound.min_y } else { bound.min_x };
        let delta = target_cross - current_cross;
        if delta.abs() < 0.5 {
            continue;
        }
        let sub = &graph.subgraphs[bound.idx];
        for node_id in &sub.nodes {
            if let Some(node) = nodes.get_mut(node_id) {
                if horizontal {
                    node.y += delta;
                } else {
                    node.x += delta;
                }
            }
        }
        if let Some(anchor_id) = bound.anchor_id.as_deref() {
            if let Some(node) = nodes.get_mut(anchor_id) {
                if horizontal {
                    node.y += delta;
                } else {
                    node.x += delta;
                }
            }
        }
    }
}

fn align_disconnected_components(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart
        || !graph.subgraphs.is_empty()
    {
        return;
    }

    let mut visible_nodes: Vec<String> = nodes
        .values()
        .filter(|node| !node.hidden)
        .map(|node| node.id.clone())
        .collect();
    if visible_nodes.len() < 2 {
        return;
    }
    visible_nodes.sort();

    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for node_id in &visible_nodes {
        adjacency.entry(node_id.clone()).or_default();
    }
    for edge in &graph.edges {
        if !adjacency.contains_key(&edge.from) || !adjacency.contains_key(&edge.to) {
            continue;
        }
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        adjacency
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut components: Vec<Vec<String>> = Vec::new();
    for node_id in &visible_nodes {
        if visited.contains(node_id) {
            continue;
        }
        let mut stack = vec![node_id.clone()];
        let mut comp = Vec::new();
        visited.insert(node_id.clone());
        while let Some(cur) = stack.pop() {
            comp.push(cur.clone());
            if let Some(neigh) = adjacency.get(&cur) {
                for next in neigh {
                    if visited.insert(next.clone()) {
                        stack.push(next.clone());
                    }
                }
            }
        }
        if comp.len() > 0 {
            components.push(comp);
        }
    }

    if components.len() < 2 {
        return;
    }

    #[derive(Clone)]
    struct CompBounds {
        nodes: Vec<String>,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
    }

    let mut bounds: Vec<CompBounds> = Vec::new();
    for comp in components {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node_id in &comp {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
                max_x = max_x.max(node.x + node.width);
                max_y = max_y.max(node.y + node.height);
            }
        }
        if min_x == f32::MAX {
            continue;
        }
        bounds.push(CompBounds {
            nodes: comp,
            min_x,
            min_y,
            max_x,
            max_y,
        });
    }

    if bounds.len() < 2 {
        return;
    }

    let horizontal = is_horizontal(graph.direction);
    bounds.sort_by(|a, b| {
        let a_key = if horizontal { a.min_x } else { a.min_y };
        let b_key = if horizontal { b.min_x } else { b.min_y };
        a_key.partial_cmp(&b_key).unwrap_or(Ordering::Equal)
    });

    let target_cross = bounds
        .iter()
        .map(|b| if horizontal { b.min_y } else { b.min_x })
        .fold(f32::MAX, f32::min);
    let spacing = config.node_spacing.max(16.0);
    let mut cursor = if horizontal {
        bounds.iter().map(|b| b.min_x).fold(f32::MAX, f32::min)
    } else {
        bounds.iter().map(|b| b.min_y).fold(f32::MAX, f32::min)
    };

    for bound in bounds {
        let min_main = if horizontal { bound.min_x } else { bound.min_y };
        let max_main = if horizontal { bound.max_x } else { bound.max_y };
        let current_cross = if horizontal { bound.min_y } else { bound.min_x };
        let delta_main = cursor - min_main;
        let delta_cross = target_cross - current_cross;
        for node_id in &bound.nodes {
            if let Some(node) = nodes.get_mut(node_id) {
                if horizontal {
                    node.x += delta_main;
                    node.y += delta_cross;
                } else {
                    node.x += delta_cross;
                    node.y += delta_main;
                }
            }
        }
        let size = (max_main - min_main).max(1.0);
        cursor += size + spacing;
    }
}

fn apply_visual_objectives(
    graph: &Graph,
    layout_edges: &[crate::mermaid_renderer::ir::Edge],
    nodes: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) {
    if !config.flowchart.objective.enabled {
        return;
    }
    relax_edge_span_constraints(graph, layout_edges, nodes, theme, config);
    rebalance_top_level_subgraphs_aspect(graph, nodes, config);
    let overlap_pass_enabled = match graph.kind {
        crate::mermaid_renderer::ir::DiagramKind::Class => true,
        crate::mermaid_renderer::ir::DiagramKind::Flowchart
        | crate::mermaid_renderer::ir::DiagramKind::State
        | crate::mermaid_renderer::ir::DiagramKind::Er
        | crate::mermaid_renderer::ir::DiagramKind::Requirement => has_visible_node_overlap(nodes),
        _ => false,
    };
    if overlap_pass_enabled {
        resolve_node_overlaps(graph, nodes, config);
    }
}

fn node_main_center(node: &NodeLayout, horizontal: bool) -> f32 {
    if horizontal {
        node.x + node.width / 2.0
    } else {
        node.y + node.height / 2.0
    }
}

fn node_main_half(node: &NodeLayout, horizontal: bool) -> f32 {
    if horizontal {
        node.width / 2.0
    } else {
        node.height / 2.0
    }
}

fn shift_node_main(node: &mut NodeLayout, horizontal: bool, delta: f32) {
    if horizontal {
        node.x += delta;
    } else {
        node.y += delta;
    }
}

fn shift_node_cross(node: &mut NodeLayout, horizontal: bool, delta: f32) {
    if horizontal {
        node.y += delta;
    } else {
        node.x += delta;
    }
}

fn relax_edge_span_constraints(
    graph: &Graph,
    layout_edges: &[crate::mermaid_renderer::ir::Edge],
    nodes: &mut BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) {
    if layout_edges.is_empty() {
        return;
    }
    match graph.kind {
        crate::mermaid_renderer::ir::DiagramKind::Class
        | crate::mermaid_renderer::ir::DiagramKind::Flowchart
        | crate::mermaid_renderer::ir::DiagramKind::State
        | crate::mermaid_renderer::ir::DiagramKind::Er
        | crate::mermaid_renderer::ir::DiagramKind::Requirement => {}
        _ => return,
    }
    let horizontal = is_horizontal(graph.direction);
    let objective = &config.flowchart.objective;
    let passes = objective.edge_relax_passes.max(1);
    let step_limit = (config.rank_spacing + config.node_spacing).max(24.0);
    let mut label_cache: HashMap<String, TextBlock> = HashMap::new();

    for _ in 0..passes {
        let mut changed = false;
        for edge in layout_edges {
            let Some(from_node) = nodes.get(&edge.from) else {
                continue;
            };
            let Some(to_node) = nodes.get(&edge.to) else {
                continue;
            };
            if from_node.hidden || to_node.hidden {
                continue;
            }
            let from_main = node_main_center(from_node, horizontal);
            let to_main = node_main_center(to_node, horizontal);
            let from_main_half = node_main_half(from_node, horizontal);
            let to_main_half = node_main_half(to_node, horizontal);
            let main_delta = to_main - from_main;
            let current_main_gap = if main_delta >= 0.0 {
                (to_main - to_main_half) - (from_main + from_main_half)
            } else {
                (from_main - from_main_half) - (to_main + to_main_half)
            };

            let has_center_label = edge
                .label
                .as_deref()
                .is_some_and(|label| !label.trim().is_empty());
            let has_start_label = edge
                .start_label
                .as_deref()
                .is_some_and(|label| !label.trim().is_empty());
            let has_end_label = edge
                .end_label
                .as_deref()
                .is_some_and(|label| !label.trim().is_empty());
            let has_endpoint_label = has_start_label || has_end_label;
            if !has_center_label && !has_endpoint_label {
                continue;
            }

            let mut required_main_gap =
                (config.node_spacing * objective.edge_gap_floor_ratio).max(8.0);
            if let Some(label) = edge
                .label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
            {
                let label_block = label_cache
                    .entry(label.to_string())
                    .or_insert_with(|| measure_label(label, theme, config))
                    .clone();
                let label_extent = if horizontal {
                    label_block.width
                } else {
                    label_block.height
                };
                required_main_gap += label_extent * objective.edge_label_weight;
                required_main_gap += theme.font_size * 0.35;
            }
            if let Some(label) = edge
                .start_label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
            {
                let label_block = label_cache
                    .entry(label.to_string())
                    .or_insert_with(|| measure_label(label, theme, config))
                    .clone();
                let label_extent = if horizontal {
                    label_block.width
                } else {
                    label_block.height
                };
                required_main_gap += label_extent * objective.endpoint_label_weight;
                required_main_gap += theme.font_size * 0.2;
            }
            if let Some(label) = edge
                .end_label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
            {
                let label_block = label_cache
                    .entry(label.to_string())
                    .or_insert_with(|| measure_label(label, theme, config))
                    .clone();
                let label_extent = if horizontal {
                    label_block.width
                } else {
                    label_block.height
                };
                required_main_gap += label_extent * objective.endpoint_label_weight;
                required_main_gap += theme.font_size * 0.2;
            }
            if has_start_label && has_end_label {
                required_main_gap += theme.font_size * 0.45;
            }
            let max_main_gap = (config.rank_spacing + config.node_spacing) * 6.0;
            required_main_gap = required_main_gap.min(max_main_gap);

            if current_main_gap + 0.5 < required_main_gap {
                let delta = (required_main_gap - current_main_gap).min(step_limit);
                let ahead_id = if main_delta >= 0.0 {
                    edge.to.as_str()
                } else {
                    edge.from.as_str()
                };
                if let Some(node) = nodes.get_mut(ahead_id) {
                    shift_node_main(node, horizontal, delta);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn resolve_node_overlaps(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    let horizontal = is_horizontal(graph.direction);
    let min_gap = (config.node_spacing * 0.2).max(4.0);
    let mut ids: Vec<String> = nodes
        .values()
        .filter(|node| !node.hidden)
        .map(|node| node.id.clone())
        .collect();
    if ids.len() < 2 {
        return;
    }
    ids.sort_by_key(|id| graph.node_order.get(id).copied().unwrap_or(usize::MAX));

    for _ in 0..6 {
        let mut moved = false;
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let id_a = &ids[i];
                let id_b = &ids[j];
                let (ax, ay, aw, ah, bx, by, bw, bh) = {
                    let Some(a) = nodes.get(id_a) else {
                        continue;
                    };
                    let Some(b) = nodes.get(id_b) else {
                        continue;
                    };
                    (a.x, a.y, a.width, a.height, b.x, b.y, b.width, b.height)
                };
                let overlap_x = (ax + aw).min(bx + bw) - ax.max(bx);
                let overlap_y = (ay + ah).min(by + bh) - ay.max(by);
                if overlap_x <= 0.0 || overlap_y <= 0.0 {
                    continue;
                }
                let (center_a, center_b) = if horizontal {
                    (ay + ah / 2.0, by + bh / 2.0)
                } else {
                    (ax + aw / 2.0, bx + bw / 2.0)
                };
                let mut sign = if center_b >= center_a { 1.0 } else { -1.0 };
                if (center_b - center_a).abs() < 0.5 {
                    let order_a = graph.node_order.get(id_a).copied().unwrap_or(usize::MAX);
                    let order_b = graph.node_order.get(id_b).copied().unwrap_or(usize::MAX);
                    sign = if order_b >= order_a { 1.0 } else { -1.0 };
                }
                let delta = if horizontal {
                    overlap_y + min_gap
                } else {
                    overlap_x + min_gap
                };
                if let Some(node_b) = nodes.get_mut(id_b) {
                    shift_node_cross(node_b, horizontal, sign * delta);
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

fn has_visible_node_overlap(nodes: &BTreeMap<String, NodeLayout>) -> bool {
    let mut visible: Vec<&NodeLayout> = nodes.values().filter(|node| !node.hidden).collect();
    if visible.len() < 2 {
        return false;
    }
    visible.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
    for i in 0..visible.len() {
        let a = visible[i];
        for b in visible.iter().skip(i + 1) {
            if b.x >= a.x + a.width {
                break;
            }
            let overlap_x = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
            let overlap_y = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
            if overlap_x > 0.0 && overlap_y > 0.0 {
                return true;
            }
        }
    }
    false
}

fn graph_has_directed_cycle(graph: &Graph) -> bool {
    if graph.nodes.len() < 2 || graph.edges.len() < 2 {
        return false;
    }
    let mut indegree: HashMap<String, usize> =
        graph.nodes.keys().map(|id| (id.clone(), 0)).collect();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &graph.edges {
        if !indegree.contains_key(&edge.from) || !indegree.contains_key(&edge.to) {
            continue;
        }
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
        *indegree.entry(edge.to.clone()).or_default() += 1;
    }
    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter_map(|(id, deg)| if *deg == 0 { Some(id.clone()) } else { None })
        .collect();
    let mut visited = 0usize;
    while let Some(node_id) = queue.pop_front() {
        visited += 1;
        if let Some(next) = outgoing.get(&node_id) {
            for target in next {
                if let Some(deg) = indegree.get_mut(target) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(target.clone());
                    }
                }
            }
        }
    }
    visited < indegree.len()
}

#[derive(Clone)]
struct VisualGroup {
    sub_idx: usize,
    nodes: Vec<String>,
    min_main: f32,
    max_main: f32,
    min_cross: f32,
    max_cross: f32,
}

fn rebalance_top_level_subgraphs_aspect(
    graph: &Graph,
    nodes: &mut BTreeMap<String, NodeLayout>,
    config: &LayoutConfig,
) {
    if graph.kind != crate::mermaid_renderer::ir::DiagramKind::Flowchart {
        return;
    }
    if graph.subgraphs.len() < 2 {
        return;
    }
    if graph.nodes.len() < 120 {
        return;
    }
    let horizontal = is_horizontal(graph.direction);
    let mut groups = collect_top_level_visual_groups(graph, nodes, horizontal);
    let objective = &config.flowchart.objective;
    if groups.len() < objective.wrap_min_groups {
        return;
    }

    let min_main = groups
        .iter()
        .map(|group| group.min_main)
        .fold(f32::MAX, f32::min);
    let max_main = groups
        .iter()
        .map(|group| group.max_main)
        .fold(f32::MIN, f32::max);
    let min_cross = groups
        .iter()
        .map(|group| group.min_cross)
        .fold(f32::MAX, f32::min);
    let max_cross = groups
        .iter()
        .map(|group| group.max_cross)
        .fold(f32::MIN, f32::max);
    if min_main == f32::MAX || min_cross == f32::MAX {
        return;
    }

    let main_span = (max_main - min_main).max(1.0);
    let cross_span = (max_cross - min_cross).max(1.0);
    let target_aspect = objective.max_aspect_ratio.max(1.0);
    let aspect = main_span / cross_span;
    if aspect <= target_aspect {
        return;
    }

    let row_count = if top_level_subgraph_chain_like(graph, &groups) {
        ((aspect / target_aspect).ceil() as usize).clamp(2, groups.len())
    } else {
        ((aspect / target_aspect).sqrt().ceil() as usize).clamp(2, groups.len())
    };
    let base_row_len = groups.len() / row_count;
    let extra_rows = groups.len() % row_count;
    let gap_main = config.node_spacing.max(12.0) * objective.wrap_main_gap_scale.max(0.1);
    let gap_cross = config.rank_spacing.max(12.0) * objective.wrap_cross_gap_scale.max(0.1);

    let mut row_start = 0usize;
    let mut cursor_cross = min_cross;
    for row in 0..row_count {
        let row_len = base_row_len + usize::from(row < extra_rows);
        if row_len == 0 {
            continue;
        }
        let row_end = row_start + row_len;
        let mut cursor_main = min_main;
        let mut row_cross_span = 0.0_f32;
        for group in &mut groups[row_start..row_end] {
            let delta_main = cursor_main - group.min_main;
            let delta_cross = cursor_cross - group.min_cross;
            for node_id in &group.nodes {
                if let Some(node) = nodes.get_mut(node_id) {
                    shift_node_main(node, horizontal, delta_main);
                    shift_node_cross(node, horizontal, delta_cross);
                }
            }
            group.min_main += delta_main;
            group.max_main += delta_main;
            group.min_cross += delta_cross;
            group.max_cross += delta_cross;
            cursor_main = group.max_main + gap_main;
            row_cross_span = row_cross_span.max(group.max_cross - group.min_cross);
        }
        cursor_cross += row_cross_span + gap_cross;
        row_start = row_end;
    }
}

fn collect_top_level_visual_groups(
    graph: &Graph,
    nodes: &BTreeMap<String, NodeLayout>,
    horizontal: bool,
) -> Vec<VisualGroup> {
    let top_level = top_level_subgraph_indices(graph);
    if top_level.len() < 2 {
        return Vec::new();
    }

    let mut seen: HashSet<&str> = HashSet::new();
    for &idx in &top_level {
        for node_id in &graph.subgraphs[idx].nodes {
            if !seen.insert(node_id.as_str()) {
                return Vec::new();
            }
        }
    }

    let mut groups = Vec::new();
    for &idx in &top_level {
        let sub = &graph.subgraphs[idx];
        if is_region_subgraph(sub) {
            continue;
        }
        let mut ids: Vec<String> = Vec::new();
        let mut min_main = f32::MAX;
        let mut max_main = f32::MIN;
        let mut min_cross = f32::MAX;
        let mut max_cross = f32::MIN;
        for node_id in &sub.nodes {
            let Some(node) = nodes.get(node_id) else {
                continue;
            };
            if node.hidden {
                continue;
            }
            ids.push(node_id.clone());
            let (main_start, main_end) = if horizontal {
                (node.x, node.x + node.width)
            } else {
                (node.y, node.y + node.height)
            };
            let (cross_start, cross_end) = if horizontal {
                (node.y, node.y + node.height)
            } else {
                (node.x, node.x + node.width)
            };
            min_main = min_main.min(main_start);
            max_main = max_main.max(main_end);
            min_cross = min_cross.min(cross_start);
            max_cross = max_cross.max(cross_end);
        }
        if ids.is_empty() {
            continue;
        }
        groups.push(VisualGroup {
            sub_idx: idx,
            nodes: ids,
            min_main,
            max_main,
            min_cross,
            max_cross,
        });
    }
    groups.sort_by(|a, b| {
        a.min_main
            .partial_cmp(&b.min_main)
            .unwrap_or(Ordering::Equal)
    });
    groups
}

fn top_level_subgraph_chain_like(graph: &Graph, groups: &[VisualGroup]) -> bool {
    if groups.len() < 3 {
        return false;
    }
    let mut node_to_subgraph: HashMap<&str, usize> = HashMap::new();
    for group in groups {
        for node_id in &group.nodes {
            node_to_subgraph.insert(node_id.as_str(), group.sub_idx);
        }
    }

    let mut indegree: HashMap<usize, usize> = HashMap::new();
    let mut outdegree: HashMap<usize, usize> = HashMap::new();
    let mut cross_edges = 0usize;
    for edge in &graph.edges {
        let Some(&from_sub) = node_to_subgraph.get(edge.from.as_str()) else {
            continue;
        };
        let Some(&to_sub) = node_to_subgraph.get(edge.to.as_str()) else {
            continue;
        };
        if from_sub == to_sub {
            continue;
        }
        cross_edges += 1;
        *outdegree.entry(from_sub).or_default() += 1;
        *indegree.entry(to_sub).or_default() += 1;
    }
    if cross_edges < groups.len().saturating_sub(1) {
        return false;
    }
    for group in groups {
        if indegree.get(&group.sub_idx).copied().unwrap_or(0) > 1 {
            return false;
        }
        if outdegree.get(&group.sub_idx).copied().unwrap_or(0) > 1 {
            return false;
        }
    }
    true
}

fn build_subgraph_layouts(
    graph: &Graph,
    nodes: &BTreeMap<String, NodeLayout>,
    theme: &Theme,
    config: &LayoutConfig,
) -> Vec<SubgraphLayout> {
    let mut subgraphs = Vec::new();
    for sub in &graph.subgraphs {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node_id in &sub.nodes {
            if let Some(node) = nodes.get(node_id) {
                min_x = min_x.min(node.x);
                min_y = min_y.min(node.y);
                max_x = max_x.max(node.x + node.width);
                max_y = max_y.max(node.y + node.height);
            }
        }

        if min_x == f32::MAX {
            continue;
        }

        let style = resolve_subgraph_style(sub, graph);
        let mut label_block = measure_label(&sub.label, theme, config);
        let label_empty = sub.label.trim().is_empty();
        if label_empty {
            label_block.width = 0.0;
            label_block.height = 0.0;
        }
        let (padding_x, padding_y, top_padding) =
            subgraph_padding_from_label(graph, sub, theme, &label_block);

        let node_width = max_x - min_x;
        let base_width = node_width + padding_x * 2.0;
        let min_label_width = if label_empty {
            base_width
        } else {
            label_block.width + padding_x * 2.0
        };
        let width = base_width.max(min_label_width);
        let extra_width = width - base_width;

        subgraphs.push(SubgraphLayout {
            label: sub.label.clone(),
            label_block,
            nodes: sub.nodes.clone(),
            x: min_x - padding_x - extra_width / 2.0,
            y: min_y - top_padding,
            width,
            height: (max_y - min_y) + padding_y + top_padding,
            style,
        });
    }

    if subgraphs.len() > 1 {
        let sets: Vec<HashSet<String>> = graph
            .subgraphs
            .iter()
            .map(|sub| sub.nodes.iter().cloned().collect())
            .collect();

        let mut order: Vec<usize> = (0..subgraphs.len()).collect();
        order.sort_by_key(|i| sets[*i].len());

        for &i in &order {
            for &j in &order {
                if i == j {
                    continue;
                }
                if is_region_subgraph(&graph.subgraphs[j]) {
                    continue;
                }
                if sets[j].len() >= sets[i].len() {
                    continue;
                }
                if !sets[j].is_subset(&sets[i]) {
                    continue;
                }
                let pad = 12.0;
                let (child_x, child_y, child_w, child_h) = {
                    let child = &subgraphs[j];
                    (child.x, child.y, child.width, child.height)
                };
                let parent = &mut subgraphs[i];
                let min_x = parent.x.min(child_x - pad);
                let min_y = parent.y.min(child_y - pad);
                let max_x = (parent.x + parent.width).max(child_x + child_w + pad);
                let max_y = (parent.y + parent.height).max(child_y + child_h + pad);
                parent.x = min_x;
                parent.y = min_y;
                parent.width = max_x - min_x;
                parent.height = max_y - min_y;
            }
        }
    }

    subgraphs.sort_by(|a, b| {
        let area_a = a.width * a.height;
        let area_b = b.width * b.height;
        area_b.partial_cmp(&area_a).unwrap_or(Ordering::Equal)
    });
    subgraphs
}

fn merge_node_style(
    target: &mut crate::mermaid_renderer::ir::NodeStyle,
    source: &crate::mermaid_renderer::ir::NodeStyle,
) {
    if source.fill.is_some() {
        target.fill = source.fill.clone();
    }
    if source.stroke.is_some() {
        target.stroke = source.stroke.clone();
    }
    if source.text_color.is_some() {
        target.text_color = source.text_color.clone();
    }
    if source.stroke_width.is_some() {
        target.stroke_width = source.stroke_width;
    }
    if source.stroke_dasharray.is_some() {
        target.stroke_dasharray = source.stroke_dasharray.clone();
    }
    if source.line_color.is_some() {
        target.line_color = source.line_color.clone();
    }
}

fn shape_padding_factors(shape: crate::mermaid_renderer::ir::NodeShape) -> (f32, f32) {
    match shape {
        crate::mermaid_renderer::ir::NodeShape::Stadium => (0.43, 0.5),
        crate::mermaid_renderer::ir::NodeShape::Subroutine => (0.54, 0.5),
        crate::mermaid_renderer::ir::NodeShape::Parallelogram => (0.894, 0.5),
        crate::mermaid_renderer::ir::NodeShape::ParallelogramAlt => (0.904, 0.5),
        _ => (1.0, 1.0),
    }
}

fn has_divider_line(label: &TextBlock) -> bool {
    label.lines.iter().any(|line| line.trim() == "---")
}

fn shape_size(
    shape: crate::mermaid_renderer::ir::NodeShape,
    label: &TextBlock,
    config: &LayoutConfig,
    theme: &Theme,
    kind: crate::mermaid_renderer::ir::DiagramKind,
) -> (f32, f32) {
    let (pad_x_factor, pad_y_factor) = shape_padding_factors(shape);
    let (kind_pad_x_scale, kind_pad_y_scale) = match kind {
        crate::mermaid_renderer::ir::DiagramKind::Class => {
            let pad_x_scale = if has_divider_line(label) { 0.85 } else { 0.4 };
            (pad_x_scale, 0.8)
        }
        crate::mermaid_renderer::ir::DiagramKind::Er => (1.05, 1.15),
        crate::mermaid_renderer::ir::DiagramKind::Kanban => (2.3, 0.67),
        crate::mermaid_renderer::ir::DiagramKind::Requirement => (0.1, 1.0),
        _ => (1.0, 1.0),
    };
    let mut pad_x = config.node_padding_x * pad_x_factor * kind_pad_x_scale;
    let mut pad_y = config.node_padding_y * pad_y_factor * kind_pad_y_scale;
    if kind == crate::mermaid_renderer::ir::DiagramKind::State {
        let dynamic_pad_x = (theme.font_size * 0.9).max(label.width * 0.12);
        let dynamic_pad_y = (theme.font_size * 0.65).max(label.height * 0.22);
        pad_x = dynamic_pad_x;
        pad_y = dynamic_pad_y;
    }
    let base_width = label.width + pad_x * 2.0;
    let base_height = label.height + pad_y * 2.0;
    let mut width = base_width;
    let mut height = base_height;
    let label_empty = label.lines.len() == 1 && label.lines[0].trim().is_empty();

    match shape {
        crate::mermaid_renderer::ir::NodeShape::Diamond => {
            // Mermaid renders diamonds as squares sized off the larger
            // dimension rather than stretching width/height independently.
            let size = base_width.max(base_height) * 0.95;
            width = size;
            height = size;
        }
        crate::mermaid_renderer::ir::NodeShape::ForkJoin => {
            width = width.max(50.0);
            height = (config.node_padding_y * 0.4).max(8.0);
        }
        crate::mermaid_renderer::ir::NodeShape::Circle
        | crate::mermaid_renderer::ir::NodeShape::DoubleCircle => {
            let size = if label_empty {
                (config.node_padding_y * 1.4).max(14.0)
            } else {
                width.max(height)
            };
            width = size;
            height = size;
        }
        crate::mermaid_renderer::ir::NodeShape::Stadium => {}
        crate::mermaid_renderer::ir::NodeShape::RoundRect => {
            width *= 1.1;
            height *= 1.05;
        }
        crate::mermaid_renderer::ir::NodeShape::Cylinder => {
            width *= 1.1;
            height *= 1.1;
        }
        crate::mermaid_renderer::ir::NodeShape::Hexagon => {
            width *= 1.2;
            height *= 1.1;
        }
        crate::mermaid_renderer::ir::NodeShape::Parallelogram
        | crate::mermaid_renderer::ir::NodeShape::ParallelogramAlt => {}
        crate::mermaid_renderer::ir::NodeShape::Trapezoid
        | crate::mermaid_renderer::ir::NodeShape::TrapezoidAlt
        | crate::mermaid_renderer::ir::NodeShape::Asymmetric => {
            width *= 1.2;
        }
        crate::mermaid_renderer::ir::NodeShape::Subroutine => {}
        _ => {}
    }

    if kind == crate::mermaid_renderer::ir::DiagramKind::Class {
        let min_height = theme.font_size * 6.5;
        height = height.max(min_height);
    }

    if kind == crate::mermaid_renderer::ir::DiagramKind::Requirement {
        let min_width = theme.font_size * 9.5;
        let min_height = theme.font_size * 10.5;
        width = width.max(min_width);
        height = height.max(min_height);
    }

    if kind == crate::mermaid_renderer::ir::DiagramKind::Kanban {
        let min_width = theme.font_size * 11.0;
        let min_height = theme.font_size * 2.6;
        width = width.max(min_width);
        height = height.max(min_height);
    }

    (width, height)
}

fn requirement_edge_label_text(label: &str, config: &LayoutConfig) -> String {
    let trimmed = label
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if config.requirement.edge_label_brackets {
        format!("<<{}>>", trimmed)
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid_renderer::ir::{Direction, Graph, NodeShape};

    #[test]
    fn wraps_long_labels() {
        let theme = Theme::modern();
        let mut config = LayoutConfig::default();
        config.max_label_width_chars = 8;
        let block = measure_label("this is a long label", &theme, &config);
        assert!(block.lines.len() > 1);
    }

    #[test]
    fn layout_places_nodes() {
        let mut graph = Graph::new();
        graph.direction = Direction::LeftRight;
        graph.ensure_node("A", Some("Alpha".to_string()), Some(NodeShape::Rectangle));
        graph.ensure_node("B", Some("Beta".to_string()), Some(NodeShape::Rectangle));
        graph.edges.push(crate::mermaid_renderer::ir::Edge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            start_label: None,
            end_label: None,
            directed: true,
            arrow_start: false,
            arrow_end: true,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
        });
        let layout = compute_layout(&graph, &Theme::modern(), &LayoutConfig::default());
        let a = layout.nodes.get("A").unwrap();
        let b = layout.nodes.get("B").unwrap();
        assert!(b.x >= a.x);
    }

    #[test]
    fn edge_style_merges_default_and_override() {
        let mut graph = Graph::new();
        graph.ensure_node("A", Some("Alpha".to_string()), Some(NodeShape::Rectangle));
        graph.ensure_node("B", Some("Beta".to_string()), Some(NodeShape::Rectangle));
        graph.edges.push(crate::mermaid_renderer::ir::Edge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            start_label: None,
            end_label: None,
            directed: true,
            arrow_start: false,
            arrow_end: true,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style: crate::mermaid_renderer::ir::EdgeStyle::Solid,
        });

        graph.edge_style_default = Some(crate::mermaid_renderer::ir::EdgeStyleOverride {
            stroke: Some("#111111".to_string()),
            stroke_width: None,
            dasharray: None,
            label_color: Some("#222222".to_string()),
        });
        graph.edge_styles.insert(
            0,
            crate::mermaid_renderer::ir::EdgeStyleOverride {
                stroke: None,
                stroke_width: Some(4.0),
                dasharray: None,
                label_color: None,
            },
        );

        let layout = compute_layout(&graph, &Theme::modern(), &LayoutConfig::default());
        let edge = &layout.edges[0];
        assert_eq!(edge.override_style.stroke.as_deref(), Some("#111111"));
        assert_eq!(edge.override_style.stroke_width, Some(4.0));
        assert_eq!(edge.override_style.label_color.as_deref(), Some("#222222"));
    }

    fn make_node(id: &str, x: f32, y: f32, width: f32, height: f32) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            x,
            y,
            width,
            height,
            label: TextBlock {
                lines: vec![String::new()],
                width: 0.0,
                height: 0.0,
            },
            shape: crate::mermaid_renderer::ir::NodeShape::Rectangle,
            style: crate::mermaid_renderer::ir::NodeStyle::default(),
            link: None,
            anchor_subgraph: None,
            hidden: false,
        }
    }

    fn make_edge(
        from: &str,
        to: &str,
        style: crate::mermaid_renderer::ir::EdgeStyle,
    ) -> crate::mermaid_renderer::ir::Edge {
        crate::mermaid_renderer::ir::Edge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            start_label: None,
            end_label: None,
            directed: true,
            arrow_start: false,
            arrow_end: true,
            arrow_start_kind: None,
            arrow_end_kind: None,
            start_decoration: None,
            end_decoration: None,
            style,
        }
    }

    #[test]
    fn path_bend_count_tracks_turns() {
        let straight = vec![(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)];
        let orth = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (20.0, 10.0)];
        assert_eq!(path_bend_count(&straight), 0);
        assert_eq!(path_bend_count(&orth), 2);
    }

    #[test]
    fn rank_edges_prefers_non_dotted_flow_edges_when_coverage_is_good() {
        let graph = Graph::new();
        let nodes = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];
        let edges = vec![
            make_edge("A", "B", crate::mermaid_renderer::ir::EdgeStyle::Solid),
            make_edge("B", "C", crate::mermaid_renderer::ir::EdgeStyle::Solid),
            make_edge("C", "D", crate::mermaid_renderer::ir::EdgeStyle::Solid),
            make_edge("A", "D", crate::mermaid_renderer::ir::EdgeStyle::Dotted),
        ];
        let rank_edges = rank_edges_for_manual_layout(&graph, &nodes, &edges);
        assert_eq!(rank_edges.len(), 3);
        assert!(
            rank_edges
                .iter()
                .all(|edge| edge.style != crate::mermaid_renderer::ir::EdgeStyle::Dotted)
        );
    }

    #[test]
    fn rank_edges_falls_back_when_primary_coverage_is_too_small() {
        let graph = Graph::new();
        let nodes = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ];
        let edges = vec![
            make_edge("A", "B", crate::mermaid_renderer::ir::EdgeStyle::Solid),
            make_edge("C", "D", crate::mermaid_renderer::ir::EdgeStyle::Dotted),
            make_edge("D", "E", crate::mermaid_renderer::ir::EdgeStyle::Dotted),
            make_edge("E", "C", crate::mermaid_renderer::ir::EdgeStyle::Dotted),
        ];
        let rank_edges = rank_edges_for_manual_layout(&graph, &nodes, &edges);
        assert_eq!(rank_edges.len(), edges.len());
    }

    #[test]
    fn routing_avoids_occupied_lane_when_possible() {
        let config = LayoutConfig::default();
        let from = make_node("A", 0.0, 0.0, 40.0, 40.0);
        let to = make_node("B", 200.0, 0.0, 40.0, 40.0);
        let obstacles: Vec<Obstacle> = Vec::new();
        let label_obstacles: Vec<Obstacle> = Vec::new();
        let ctx = RouteContext {
            from_id: &from.id,
            to_id: &to.id,
            from: &from,
            to: &to,
            direction: Direction::LeftRight,
            config: &config,
            obstacles: &obstacles,
            label_obstacles: &label_obstacles,
            base_offset: 0.0,
            start_side: EdgeSide::Right,
            end_side: EdgeSide::Left,
            start_offset: 0.0,
            end_offset: 0.0,
            fast_route: false,
        };
        let mut occupancy = EdgeOccupancy::new(config.node_spacing.max(16.0) * 0.6);
        let start = anchor_point_for_node(&from, EdgeSide::Right, 0.0);
        let end = anchor_point_for_node(&to, EdgeSide::Left, 0.0);
        occupancy.add_path(&[start, end]);

        let points = route_edge_with_avoidance(&ctx, Some(&occupancy), None, None);
        assert!(
            points.len() > 2,
            "expected a detoured path to avoid occupied lane"
        );
    }

    #[test]
    fn routing_handles_tiny_nodes_without_panicking() {
        let config = LayoutConfig::default();
        let from = make_node("A", 0.0, 0.0, 1.0, 1.0);
        let to = make_node("B", 50.0, 0.0, 1.0, 1.0);
        let obstacles: Vec<Obstacle> = Vec::new();
        let label_obstacles: Vec<Obstacle> = Vec::new();
        let ctx = RouteContext {
            from_id: &from.id,
            to_id: &to.id,
            from: &from,
            to: &to,
            direction: Direction::LeftRight,
            config: &config,
            obstacles: &obstacles,
            label_obstacles: &label_obstacles,
            base_offset: 0.0,
            start_side: EdgeSide::Right,
            end_side: EdgeSide::Left,
            start_offset: 0.0,
            end_offset: 0.0,
            fast_route: false,
        };
        let points = route_edge_with_avoidance(&ctx, None, None, None);
        assert!(!points.is_empty());
    }

    #[test]
    fn grid_router_avoids_blocking_obstacle() {
        let mut config = LayoutConfig::default();
        config.flowchart.routing.enable_grid_router = true;
        config.flowchart.routing.grid_cell = 10.0;
        let from = make_node("A", 0.0, 0.0, 40.0, 40.0);
        let to = make_node("B", 220.0, 0.0, 40.0, 40.0);
        let obstacles = vec![Obstacle {
            id: "blocker".to_string(),
            x: 90.0,
            y: -10.0,
            width: 80.0,
            height: 60.0,
            members: None,
        }];
        let label_obstacles: Vec<Obstacle> = Vec::new();
        let grid = build_routing_grid(&obstacles, &config).expect("routing grid");
        let ctx = RouteContext {
            from_id: &from.id,
            to_id: &to.id,
            from: &from,
            to: &to,
            direction: Direction::LeftRight,
            config: &config,
            obstacles: &obstacles,
            label_obstacles: &label_obstacles,
            base_offset: 0.0,
            start_side: EdgeSide::Right,
            end_side: EdgeSide::Left,
            start_offset: 0.0,
            end_offset: 0.0,
            fast_route: false,
        };
        let start = anchor_point_for_node(&from, EdgeSide::Right, 0.0);
        let end = anchor_point_for_node(&to, EdgeSide::Left, 0.0);
        let stub_len = port_stub_length(&config, &from, &to);
        let start_stub = port_stub_point(start, EdgeSide::Right, stub_len);
        let end_stub = port_stub_point(end, EdgeSide::Left, stub_len);
        let points =
            route_edge_with_grid(&ctx, &grid, None, start_stub, end_stub).expect("grid route");
        let hits = path_obstacle_intersections(&points, &obstacles, &from.id, &to.id);
        assert_eq!(hits, 0, "grid path should avoid obstacle");
    }
}
