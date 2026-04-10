//! UI module — submodule declarations, type re-exports, and rendering.
//!
//! All rendering is driven by [`render`], which is called once per frame from
//! the main event loop.

pub mod detail;
pub mod theme;
pub mod tree;

pub use tree::{build_tree, enumerate_all_nodes, NodeId, TreeNode};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use xtce_core::ValidationError;

use crate::app::{
    App, AppMode, CalibratorStep, CreateStep, EntryAddStep, EntryLocationStep, Focus,
    RestrictionEditStep, TypeVariant, UnitEditStep, CALIBRATOR_KIND_LABELS,
    RESTRICTION_OPERATOR_LABELS, integer_encoding_labels, float_size_labels,
};
use crate::event::EditField;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level render entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Render one frame. Called by `terminal.draw(|frame| ui::render(app, frame))`.
pub fn render(app: &mut App, frame: &mut Frame) {
    let [title_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [tree_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .areas(main_area);

    render_title(app, frame, title_area);
    render_tree(app, frame, tree_area);
    render_detail(app, frame, detail_area);
    render_status(app, frame, status_area);

    // Overlays are rendered last so they appear on top.
    if app.show_errors {
        render_errors_overlay(app, frame);
    }
    if app.show_help {
        render_help_overlay(app, frame);
    }
    // Edit mode overlay — shown while in Edit mode (and no other modal is open)
    if app.mode == AppMode::Edit
        && app.picker_state.is_none()
        && app.encoding_state.is_none()
        && app.create_state.is_none()
        && app.entry_add_state.is_none()
        && app.delete_confirm.is_none()
        && !app.reload_confirm
        && app.edit_state.is_none()
        && app.restriction_edit_state.is_none()
        && app.entry_location_state.is_none()
        && app.calibrator_state.is_none()
        && app.unit_edit_state.is_none()
        && app.enum_entry_state.is_none()
    {
        render_edit_mode_overlay(app, frame);
    }
    if let Some(cs) = &app.create_state {
        match &cs.step {
            CreateStep::TypeVariantSelect { selector_cursor } => {
                render_type_variant_select(*selector_cursor, frame);
            }
            CreateStep::PickerPrompt { filter, items, picker_cursor, .. } => {
                render_picker_overlay("Pick type", filter, items, *picker_cursor, frame);
            }
            CreateStep::NamePrompt { .. } => {} // shown in status bar
        }
    }
    if let Some(ea) = &app.entry_add_state {
        match &ea.step {
            EntryAddStep::ContainerTypeSelect { cursor } => {
                render_list_select(
                    "Add entry — select type",
                    &["ParameterRef", "ContainerRef", "FixedValue"],
                    *cursor,
                    frame,
                );
            }
            EntryAddStep::ParameterPicker { filter, items, cursor } => {
                render_picker_overlay("Add ParameterRef entry", filter, items, *cursor, frame);
            }
            EntryAddStep::ContainerPicker { filter, items, cursor } => {
                render_picker_overlay("Add ContainerRef entry", filter, items, *cursor, frame);
            }
            EntryAddStep::ArgumentPicker { filter, items, cursor } => {
                render_picker_overlay("Add ArgumentRef entry", filter, items, *cursor, frame);
            }
            EntryAddStep::FixedValueSizePrompt { .. } => {} // shown in status bar
        }
    }
    if let Some(res) = &app.restriction_edit_state {
        match &res.step {
            RestrictionEditStep::PickParameter { filter, items, cursor } => {
                render_picker_overlay("Restriction: pick parameter", filter, items, *cursor, frame);
            }
            RestrictionEditStep::PickOperator { cursor, .. } => {
                render_list_select("Restriction: pick operator", RESTRICTION_OPERATOR_LABELS, *cursor, frame);
            }
            RestrictionEditStep::EnterValue { .. } => {} // shown in status bar
        }
    }
    if let Some(els) = &app.entry_location_state {
        if let EntryLocationStep::PickEntry { items, cursor } = &els.step {
            render_picker_overlay("Set entry location — pick entry", "", items, *cursor, frame);
        }
        // EnterOffset is shown in the status bar.
    }
    if let Some(ps) = &app.picker_state {
        let title = match ps.purpose {
            crate::app::PickerPurpose::ChangeTypeRef      => "Change type reference",
            crate::app::PickerPurpose::SetBaseType        => "Set base type",
            crate::app::PickerPurpose::SetBaseContainer   => "Set base container",
            crate::app::PickerPurpose::SetBaseMetaCommand => "Set base MetaCommand",
        };
        render_picker_overlay(title, &ps.filter, &ps.items, ps.cursor, frame);
    }
    if let Some(es) = &app.encoding_state {
        match &es.step {
            crate::app::EncodingStep::IntegerFormatSelect { cursor } => {
                render_list_select("Select integer encoding format", integer_encoding_labels(), *cursor, frame);
            }
            crate::app::EncodingStep::FloatSizeSelect { cursor } => {
                render_list_select("Select float size", float_size_labels(), *cursor, frame);
            }
            crate::app::EncodingStep::IntegerSizePrompt { .. } => {} // shown in status bar
        }
    }
    if let Some(cs) = &app.calibrator_state {
        match &cs.step {
            CalibratorStep::KindSelect { cursor } => {
                render_list_select("Calibrator kind", CALIBRATOR_KIND_LABELS, *cursor, frame);
            }
            CalibratorStep::PolynomialReview { coefficients } => {
                render_polynomial_review(coefficients, frame);
            }
            CalibratorStep::SplineReview { points, order, .. } => {
                render_spline_review(points, *order, frame);
            }
            // Buffer input steps are shown in the status bar.
            _ => {}
        }
    }
    if let Some(us) = &app.unit_edit_state {
        if let UnitEditStep::Review = &us.step {
            render_unit_review(&us.units, frame);
        }
        // AddUnit step is shown in the status bar.
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main panel renderers
// ─────────────────────────────────────────────────────────────────────────────

fn render_title(app: &App, frame: &mut Frame, area: Rect) {
    let label = if app.dirty {
        format!("XTCE Editor  [{}*]", app.path.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
    } else {
        format!("XTCE Editor  [{}]", app.path.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
    };
    let title = Paragraph::new(label)
        .style(theme::title_bar())
        .alignment(Alignment::Center);
    frame.render_widget(title, area);
}

fn render_tree(app: &mut App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Tree;
    let border_color = if focused { theme::BORDER_FOCUSED } else { theme::BORDER_UNFOCUSED };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Tree ")
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build match lookup from the full (expansion-independent) NodeId list.
    let current_match_id: Option<&NodeId> = if app.search_matches.is_empty() {
        None
    } else {
        Some(&app.search_matches[app.search_match_cursor])
    };

    let items: Vec<ListItem> = app
        .tree
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth);
            let icon = if node.expandable {
                if node.expanded { "▼ " } else { "▶ " }
            } else {
                "  "
            };
            // Search match overrides the normal label colour.
            let label_style = if current_match_id == Some(&node.node_id) {
                theme::search_current_match()
            } else if app.search_matches_set.contains(&node.node_id) {
                theme::search_match()
            } else {
                match &node.node_id {
                    NodeId::SpaceSystem(_) => theme::space_system(),
                    NodeId::TmSection(_) | NodeId::CmdSection(_) => theme::section_header(),
                    NodeId::TmParameterTypes(_)
                    | NodeId::TmParameters(_)
                    | NodeId::TmContainers(_)
                    | NodeId::CmdArgumentTypes(_)
                    | NodeId::CmdMetaCommands(_) => theme::group_node(),
                    _ => theme::leaf_node(),
                }
            };
            let mut spans = vec![
                Span::raw(indent),
                Span::raw(icon),
                Span::styled(node.label.clone(), label_style),
            ];
            if let Some(ann) = &node.annotation {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(ann.clone(), theme::type_annotation()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let highlight_style = if focused {
        theme::selected_focused()
    } else {
        theme::selected_unfocused()
    };

    // Record visible height for jump_to centering.
    app.tree_panel_height = inner.height as usize;

    let list = List::new(items).highlight_style(highlight_style);
    frame.render_stateful_widget(list, inner, &mut app.list_state);
}

fn render_detail(app: &App, frame: &mut Frame, area: Rect) {
    let focused = app.focus == Focus::Detail;
    let border_color = if focused { theme::BORDER_FOCUSED } else { theme::BORDER_UNFOCUSED };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Detail ")
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = detail::detail_lines(app);
    let content_height = lines.len();
    let visible_height = inner.height as usize;
    let scroll = app.detail_scroll.min(content_height.saturating_sub(visible_height));

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}

fn render_status(app: &App, frame: &mut Frame, area: Rect) {
    let mut spans = Vec::new();

    if let Some(es) = &app.encoding_state {
        if let crate::app::EncodingStep::IntegerSizePrompt { buffer, .. } = &es.step {
            spans.push(Span::styled(" Encoding size in bits: ", theme::section_header()));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            spans.push(Span::styled("  Enter:Confirm  Esc:Cancel", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(es) = &app.enum_entry_state {
        match &es.step {
            crate::app::EnumEntryStep::ValuePrompt { buffer } => {
                spans.push(Span::styled(" Enumeration value (integer): ", theme::section_header()));
                spans.push(Span::styled(buffer.clone(), theme::detail_value()));
                spans.push(Span::styled("_", theme::dim()));
                spans.push(Span::styled("  Enter:Next  Esc:Cancel", theme::dim()));
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                return;
            }
            crate::app::EnumEntryStep::LabelPrompt { value, buffer } => {
                spans.push(Span::styled(format!(" Label for value {}: ", value), theme::section_header()));
                spans.push(Span::styled(buffer.clone(), theme::detail_value()));
                spans.push(Span::styled("_", theme::dim()));
                spans.push(Span::styled("  Enter:Add  Esc:Cancel", theme::dim()));
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                return;
            }
        }
    }

    if let Some(ea) = &app.entry_add_state {
        if let EntryAddStep::FixedValueSizePrompt { buffer } = &ea.step {
            spans.push(Span::styled(" Fixed value size in bits: ", theme::section_header()));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            spans.push(Span::styled("  Enter:Confirm  Esc:Cancel", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(res) = &app.restriction_edit_state {
        if let RestrictionEditStep::EnterValue { parameter_ref, operator_cursor, buffer } = &res.step {
            let op_label = RESTRICTION_OPERATOR_LABELS
                .get(*operator_cursor)
                .copied()
                .unwrap_or("==");
            spans.push(Span::styled(
                format!(" Restriction value  ({} {}): ", parameter_ref, op_label),
                theme::section_header(),
            ));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            spans.push(Span::styled("  Enter:Confirm  Esc:Cancel", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(els) = &app.entry_location_state {
        if let EntryLocationStep::EnterOffset { entry_name, buffer, .. } = &els.step {
            spans.push(Span::styled(
                format!(" Bit offset (containerStart) for {}: ", entry_name),
                theme::section_header(),
            ));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            spans.push(Span::styled("  Enter:Confirm  Esc:Cancel", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(us) = &app.unit_edit_state {
        if let UnitEditStep::AddUnit { buffer } = &us.step {
            spans.push(Span::styled(" Unit value: ", theme::section_header()));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            spans.push(Span::styled("  Enter:Add  Esc:Back", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(cs) = &app.calibrator_state {
        match &cs.step {
            CalibratorStep::PolynomialAddCoeff { buffer, coefficients } => {
                spans.push(Span::styled(
                    format!(" Coefficient a{} value: ", coefficients.len()),
                    theme::section_header(),
                ));
                spans.push(Span::styled(buffer.clone(), theme::detail_value()));
                spans.push(Span::styled("_", theme::dim()));
                spans.push(Span::styled("  Enter:Add  Esc:Back", theme::dim()));
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                return;
            }
            CalibratorStep::SplineAddRaw { buffer, points, .. } => {
                spans.push(Span::styled(
                    format!(" Point {} — raw value: ", points.len()),
                    theme::section_header(),
                ));
                spans.push(Span::styled(buffer.clone(), theme::detail_value()));
                spans.push(Span::styled("_", theme::dim()));
                spans.push(Span::styled("  Enter:Next  Esc:Back", theme::dim()));
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                return;
            }
            CalibratorStep::SplineAddCal { raw, buffer, points, .. } => {
                spans.push(Span::styled(
                    format!(" Point {} — calibrated value (raw={}): ", points.len(), raw),
                    theme::section_header(),
                ));
                spans.push(Span::styled(buffer.clone(), theme::detail_value()));
                spans.push(Span::styled("_", theme::dim()));
                spans.push(Span::styled("  Enter:Add  Esc:Back", theme::dim()));
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                return;
            }
            _ => {}
        }
    }

    if app.reload_confirm {
        spans.push(Span::styled(" Reload and discard unsaved changes?", theme::warn()));
        spans.push(Span::styled("  y:Confirm  n/Esc:Cancel", theme::dim()));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    if let Some(dc) = &app.delete_confirm {
        spans.push(Span::styled(" Delete '", theme::error()));
        spans.push(Span::styled(dc.name.clone(), theme::detail_value()));
        spans.push(Span::styled("'?", theme::error()));
        spans.push(Span::styled("  y:Confirm  n/Esc:Cancel", theme::dim()));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    if let Some(cs) = &app.create_state {
        if let CreateStep::NamePrompt { buffer, variant } = &cs.step {
            let kind_label = cs.kind.label();
            let type_prefix = variant.map(|v| format!("{} ", v.label())).unwrap_or_default();
            let prompt_label = format!(" New {}{} name: ", type_prefix, kind_label);
            spans.push(Span::styled(prompt_label, theme::section_header()));
            spans.push(Span::styled(buffer.clone(), theme::detail_value()));
            spans.push(Span::styled("_", theme::dim()));
            if let Some(err) = &app.create_error {
                spans.push(Span::styled(format!("  {}", err), theme::error()));
            }
            spans.push(Span::styled("  Enter:Confirm  Esc:Cancel", theme::dim()));
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }

    if let Some(edit) = &app.edit_state {
        let label = match edit.field {
            EditField::Name => "Rename",
            EditField::ShortDescription => "Description",
        };
        spans.push(Span::styled(format!(" {}: ", label), theme::section_header()));
        spans.push(Span::styled(edit.buffer.clone(), theme::detail_value()));
        spans.push(Span::styled("_", theme::dim()));
        spans.push(Span::styled("  Enter:Commit  Esc:Cancel", theme::dim()));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    if app.search_mode {
        // Search prompt: /query_  [x/y]  hints
        spans.push(Span::styled(" /", theme::section_header()));
        spans.push(Span::styled(app.search_query.clone(), theme::detail_value()));
        spans.push(Span::styled("_", theme::dim()));
        let match_info = if app.search_query.is_empty() {
            String::new()
        } else if app.search_matches.is_empty() {
            "  [no matches]".to_string()
        } else {
            format!("  [{}/{}]", app.search_match_cursor + 1, app.search_matches.len())
        };
        spans.push(Span::styled(match_info, theme::dim()));
        spans.push(Span::styled("  Esc:Close  n:Next  N:Prev  ", theme::dim()));
    } else {
        if let Some(err) = &app.save_error {
            spans.push(Span::styled(format!(" Save failed: {}  ", err), theme::error()));
        }
        let err_count = app.validation_errors.len() + app.schema_errors.len();
        if err_count > 0 {
            spans.push(Span::styled(
                format!(" {} error(s)  ", err_count),
                theme::error(),
            ));
        }
        if !app.search_matches.is_empty() {
            spans.push(Span::styled(
                format!(" /{} [{}/{}]  ", app.search_query, app.search_match_cursor + 1, app.search_matches.len()),
                theme::warn(),
            ));
        }
        if app.show_errors || app.show_help {
            spans.push(Span::styled(" ↑↓/jk:Scroll  Esc:Close ", theme::dim()));
        } else if app.mode == AppMode::Edit {
            // Edit mode: show mode indicator + available mutations for current node
            spans.push(Span::styled(" [EDIT] ", theme::section_header()));
            spans.push(Span::styled(" Esc:Back  ", theme::dim()));
            if let Some(node) = app.tree.get(app.cursor) {
                let ctx = node_context_hint(&node.node_id);
                if !ctx.is_empty() {
                    spans.push(Span::styled(ctx, theme::key_desc()));
                }
            }
        } else {
            // Explore mode: show base navigation hints
            spans.push(Span::styled(
                " q:Quit  s:Save  u:Undo  ^R:Redo  /:Search  f:FollowRef  [:Back  e:Errors  m:Edit  ?:Help ",
                theme::dim(),
            ));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay renderers
// ─────────────────────────────────────────────────────────────────────────────

fn render_list_select(title: &str, labels: &[&str], cursor: usize, frame: &mut Frame) {
    let area = centered_rect(45, 65, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} — j/k:Navigate  Enter:Confirm  Esc:Cancel ", title))
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = labels
        .iter()
        .map(|l| ListItem::new(Span::raw(format!("  {}", l))))
        .collect();

    let clamped = cursor.min(labels.len().saturating_sub(1));
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(clamped));

    let list = List::new(items).highlight_style(theme::selected_focused());
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_type_variant_select(selector_cursor: usize, frame: &mut Frame) {
    let labels: Vec<&str> = TypeVariant::all().iter().map(|v| v.label()).collect();
    render_list_select("Select type variant", &labels, selector_cursor, frame);
}

fn render_picker_overlay(
    title: &str,
    filter: &str,
    items: &[(String, String)],
    picker_cursor: usize,
    frame: &mut Frame,
) {
    let area = centered_rect(60, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} — j/k:Navigate  Enter:Select  Esc:Cancel ", title))
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [filter_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);

    // Filter line
    let filter_line = Line::from(vec![
        Span::styled(" Filter: ", theme::section_header()),
        Span::styled(filter.to_string(), theme::detail_value()),
        Span::styled("_", theme::dim()),
    ]);
    frame.render_widget(Paragraph::new(filter_line), filter_area);

    // Filtered list
    let q = filter.to_lowercase();
    let filtered: Vec<&(String, String)> = items
        .iter()
        .filter(|(label, _)| q.is_empty() || label.to_lowercase().contains(&q))
        .collect();

    if filtered.is_empty() {
        let msg = if items.is_empty() {
            "  No items — create a type first"
        } else {
            "  No matches"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(msg, theme::dim())),
            list_area,
        );
        return;
    }

    let list_items: Vec<ListItem> = filtered
        .iter()
        .map(|(label, _)| ListItem::new(Span::raw(format!("  {}", label))))
        .collect();

    let mut state = ratatui::widgets::ListState::default();
    let clamped = picker_cursor.min(filtered.len() - 1);
    state.select(Some(clamped));

    let list = List::new(list_items).highlight_style(theme::selected_focused());
    frame.render_stateful_widget(list, list_area, &mut state);
}

fn render_errors_overlay(app: &mut App, frame: &mut Frame) {
    let area = centered_rect(75, 70, frame.area());
    frame.render_widget(Clear, area);

    let sem_count = app.validation_errors.len();
    let xsd_count = app.schema_errors.len();
    let total = sem_count + xsd_count;
    let has_errors = total > 0;
    let border_color = if has_errors { Color::Red } else { theme::BORDER_FOCUSED };
    let title = if has_errors {
        format!(" Validation Errors ({total}) — j/k:select  Ctrl-u/d:page  Enter:jump  e/Esc:close ")
    } else {
        " Validation Errors — e/Esc to close ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    // Update app so cursor-visibility math in apply_action uses the real height.
    app.error_overlay_height = visible_height;

    let lines: Vec<Line<'static>> = if !has_errors {
        vec![Line::from(Span::styled("  No validation errors", theme::dim()))]
    } else {
        let mut out: Vec<Line<'static>> = Vec::new();

        if sem_count > 0 {
            let fold_indicator = if app.error_fold_semantic { "\u{25b6}" } else { "\u{25bc}" };
            out.push(Line::from(vec![
                Span::styled(format!("{fold_indicator} Semantic"), theme::group_node()),
                Span::styled(format!(" ({sem_count})"), theme::dim()),
                Span::styled("  [s:fold]", theme::dim()),
            ]));
            if !app.error_fold_semantic {
                out.push(Line::from(""));

                // Virtual scroll: compute which errors are in the visible window.
                // Header = 2 lines (group line + blank). Only fully style errors
                // whose lines overlap [scroll, scroll+visible_height). All others
                // are emitted as cheap blank lines to preserve scroll offsets.
                const HEADER: usize = 2;
                let scroll = app.error_scroll;
                let vis_end = scroll + visible_height;

                let mut line_cursor = HEADER;
                for (i, e) in app.validation_errors.iter().enumerate() {
                    let lc = e.render_line_count();
                    let in_window = (line_cursor + lc > scroll) && (line_cursor < vis_end);
                    if in_window {
                        out.extend(error_lines_with_cursor(e, i == app.error_cursor));
                    } else {
                        for _ in 0..lc {
                            out.push(Line::from(""));
                        }
                    }
                    line_cursor += lc;
                }
            }
            out.push(Line::from(""));
        }

        if xsd_count > 0 {
            let fold_indicator = if app.error_fold_schema { "▶" } else { "▼" };
            out.push(Line::from(vec![
                Span::styled(format!("{fold_indicator} Schema (XSD)"), theme::group_node()),
                Span::styled(format!(" ({xsd_count})"), theme::dim()),
                Span::styled("  [x to fold]", theme::dim()),
            ]));
            if !app.error_fold_schema {
                out.push(Line::from(""));
                out.extend(app.schema_errors.iter().flat_map(|e| error_lines(e)));
            }
        }
        out
    };

    let content_height = lines.len();
    let scroll = app
        .error_scroll
        .min(content_height.saturating_sub(visible_height));

    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn error_lines_with_cursor(e: &ValidationError, selected: bool) -> Vec<Line<'static>> {
    let mut lines = error_lines(e);
    if selected && !lines.is_empty() {
        // Prepend a cursor indicator on the first line.
        if let Some(first) = lines.first_mut() {
            let mut spans = vec![Span::styled("\u{25b6} ", theme::section_header())];
            spans.extend(first.spans.drain(..));
            *first = Line::from(spans);
        }
    }
    lines
}

fn error_lines(e: &ValidationError) -> Vec<Line<'static>> {
    match e {
        ValidationError::UnresolvedReference { name, context, .. } => vec![
            Line::from(vec![
                Span::styled("  Unresolved:  ", theme::error()),
                Span::styled(name.clone(), theme::detail_value()),
            ]),
            Line::from(Span::styled(format!("    in {}", context), theme::dim())),
            Line::from(""),
        ],
        ValidationError::CyclicInheritance { name, .. } => vec![
            Line::from(vec![
                Span::styled("  Cyclic:      ", theme::error()),
                Span::styled(name.clone(), theme::detail_value()),
            ]),
            Line::from(""),
        ],
        ValidationError::DuplicateName { name, space_system } => vec![
            Line::from(vec![
                Span::styled("  Duplicate:   ", theme::error()),
                Span::styled(name.clone(), theme::detail_value()),
            ]),
            Line::from(Span::styled(
                format!("    in SpaceSystem '{}'", space_system),
                theme::dim(),
            )),
            Line::from(""),
        ],
        ValidationError::MissingRequiredField { field, element, name } => vec![
            Line::from(vec![
                Span::styled("  Missing:     ", theme::error()),
                Span::styled(format!("{} on {} '{}'", field, element, name), theme::detail_value()),
            ]),
            Line::from(""),
        ],
        ValidationError::SchemaError(msg) => vec![
            Line::from(vec![
                Span::styled("  XSD:         ", theme::error()),
                Span::styled(msg.clone(), theme::detail_value()),
            ]),
            Line::from(""),
        ],
    }
}

/// Return a compact hint string for the status bar based on the selected node type.
fn node_context_hint(node_id: &NodeId) -> &'static str {
    match node_id {
        NodeId::SpaceSystem(_) =>
            "i:Rename  C:Desc  a:AddChild  d:Del",
        NodeId::TmSection(_) | NodeId::CmdSection(_) =>
            "a:Add",
        NodeId::TmParameterTypes(_) | NodeId::TmParameters(_) | NodeId::TmContainers(_)
        | NodeId::CmdArgumentTypes(_) | NodeId::CmdMetaCommands(_) =>
            "a:Add",
        NodeId::TmParameterType(_, _) =>
            "f:UsedBy  i:Rename  C:Desc  E:Enc  K:Cal  U:Units  b:BaseType  d:Del",
        NodeId::TmParameter(_, _) =>
            "f:GoToType  i:Rename  C:Desc  t:TypeRef  D:DataSrc  P:ReadOnly  d:Del",
        NodeId::TmContainer(_, _) =>
            "f:GoToBase  i:Rename  C:Desc  b:Base  A:Entries  L:BitOff  R:Criteria  B:Abstract  d:Del",
        NodeId::CmdArgumentType(_, _) =>
            "f:UsedBy  i:Rename  C:Desc  E:Enc  K:Cal  U:Units  b:BaseType  d:Del",
        NodeId::CmdMetaCommand(_, _) =>
            "f:GoToBase  i:Rename  C:Desc  b:Base  g:AddArg  G:RemArg  A:Entries  d:Del",
    }
}

fn render_edit_mode_overlay(app: &App, frame: &mut Frame) {
    // Small floating panel anchored to the bottom-right of the tree panel.
    let full = frame.area();
    // Position: right half, roughly 35% wide, near the top.
    let w = (full.width / 3).max(32).min(full.width.saturating_sub(2));
    let h = 20u16.min(full.height.saturating_sub(4));
    let x = full.width.saturating_sub(w + 1);
    let y = 1u16; // just below the title bar
    let area = ratatui::layout::Rect::new(x, y, w, h);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Edit Mode — Esc: back to Explore ")
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build a context-sensitive list of available edits for the selected node.
    let actions: &[(&str, &str)] = if let Some(node) = app.tree.get(app.cursor) {
        match &node.node_id {
            NodeId::SpaceSystem(_) => &[
                ("i", "Rename"),
                ("C", "Edit description"),
                ("a", "Add child SpaceSystem / item"),
                ("d", "Delete"),
            ],
            NodeId::TmParameterType(_, _) | NodeId::CmdArgumentType(_, _) => &[
                ("i", "Rename"),
                ("C", "Edit description"),
                ("b", "Set base type"),
                ("E", "Edit encoding"),
                ("K", "Edit calibrator"),
                ("U", "Edit unit set"),
                ("d", "Delete"),
            ],
            NodeId::TmParameter(_, _) => &[
                ("i", "Rename"),
                ("C", "Edit description"),
                ("t", "Change type reference"),
                ("D", "Cycle data source"),
                ("P", "Toggle read-only"),
                ("d", "Delete"),
            ],
            NodeId::TmContainer(_, _) => &[
                ("i", "Rename"),
                ("C", "Edit description"),
                ("b", "Set base container"),
                ("A", "Add entry"),
                ("x", "Remove last entry"),
                ("L", "Set entry bit offset"),
                ("R", "Edit restriction criteria"),
                ("B", "Toggle abstract"),
                ("d", "Delete"),
            ],
            NodeId::CmdMetaCommand(_, _) => &[
                ("i", "Rename"),
                ("C", "Edit description"),
                ("b", "Set base MetaCommand"),
                ("A", "Add entry"),
                ("x", "Remove last entry"),
                ("g", "Add argument"),
                ("G", "Remove last argument"),
                ("B", "Toggle abstract"),
                ("d", "Delete"),
            ],
            NodeId::TmSection(_) | NodeId::CmdSection(_)
            | NodeId::TmParameterTypes(_) | NodeId::TmParameters(_)
            | NodeId::TmContainers(_) | NodeId::CmdArgumentTypes(_)
            | NodeId::CmdMetaCommands(_) => &[
                ("a", "Add item"),
            ],
        }
    } else {
        &[]
    };

    let lines: Vec<Line<'static>> = if actions.is_empty() {
        vec![Line::from(Span::styled("  (nothing editable here)", theme::dim()))]
    } else {
        actions.iter().map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {:<4}", key), theme::key_name()),
                Span::styled(desc.to_string(), theme::key_desc()),
            ])
        }).collect()
    };

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_help_overlay(app: &App, frame: &mut Frame) {
    let area = centered_rect(50, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keybindings — ↑↓/jk:Scroll  ?/Esc:Close ")
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let bindings: &[(&str, &str)] = &[
        ("Navigation", ""),
        ("  ↑ / k", "Move up"),
        ("  ↓ / j", "Move down"),
        ("  Ctrl+U / PgUp", "Page up"),
        ("  Ctrl+D / PgDn", "Page down"),
        ("", ""),
        ("Tree", ""),
        ("  Enter / Space", "Toggle expand"),
        ("  → / l", "Expand node"),
        ("  ← / h", "Collapse node"),
        ("", ""),
        ("Panels", ""),
        ("  Tab", "Cycle focus"),
        ("", ""),
        ("Modes", ""),
        ("  m", "Enter Edit mode (mutation menu)"),
        ("  Esc (in Edit)", "Return to Explore mode"),
        ("", ""),
        ("File operations (Explore + Edit)", ""),
        ("  u", "Undo last change"),
        ("  Ctrl+R", "Redo"),
        ("  r", "Reload from disk"),
        ("  s / Ctrl+W", "Save to disk"),
        ("", ""),
        ("Reference navigation", ""),
        ("  f", "Follow reference (Param→Type, Type→Users, Ctnr→Base, Cmd→Base)"),
        ("  [", "Go back (return from last follow)"),
        ("", ""),
        ("Search (Explore mode)", ""),
        ("  /", "Open search prompt — type then Enter to search"),
        ("  n / N", "Next / previous match"),
        ("  Esc", "Cancel search (clear)"),
        ("", ""),
        ("Overlays (Explore mode)", ""),
        ("  e", "Toggle error list"),
        ("  ?", "Toggle this help"),
        ("  Esc", "Close overlay"),
        ("", ""),
        ("  q / Ctrl+C", "Quit"),
        ("", ""),
        ("Edit mode operations", ""),
        ("  i", "Rename selected item"),
        ("  C", "Edit description"),
        ("  a", "Add item (sibling or child)"),
        ("  d", "Delete selected item"),
        ("  A", "Add entry to container / MetaCommand"),
        ("  x", "Remove last entry"),
        ("  E", "Set encoding (Integer / Float types)"),
        ("  S", "Toggle signed/unsigned (Integer type)"),
        ("  b", "Set base type / container / MetaCommand"),
        ("  t", "Change type reference (Parameter)"),
        ("  B", "Toggle abstract flag"),
        ("  D", "Cycle data source (Parameter)"),
        ("  P", "Toggle read-only flag (Parameter)"),
        ("  R", "Edit restriction criteria (Container with base)"),
        ("  L", "Set entry bit offset (Container)"),
        ("  K", "Edit calibrator (Integer / Float type with encoding)"),
        ("  U", "Edit unit set (ParameterType / ArgumentType)"),
        ("  g", "Add argument to MetaCommand"),
        ("  G", "Remove last MetaCommand argument"),
    ];

    let lines: Vec<Line<'static>> = bindings
        .iter()
        .map(|(key, desc)| {
            if desc.is_empty() {
                // Section heading or blank line
                if key.is_empty() {
                    Line::from("")
                } else {
                    Line::from(Span::styled(key.to_string(), theme::group_node()))
                }
            } else {
                Line::from(vec![
                    Span::styled(format!("{:<20}", key), theme::key_name()),
                    Span::styled(desc.to_string(), theme::key_desc()),
                ])
            }
        })
        .collect();

    let content_height = lines.len();
    let visible_height = inner.height as usize;
    let scroll = app
        .detail_scroll
        .min(content_height.saturating_sub(visible_height));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

fn render_unit_review(units: &[xtce_core::model::types::Unit], frame: &mut Frame) {
    let area = centered_rect(50, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Unit Set — a:Add  d:Remove last  Enter:Commit  Esc:Discard ")
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = if units.is_empty() {
        vec![Line::from(Span::styled(
            "  (no units — press 'a' to add)",
            theme::dim(),
        ))]
    } else {
        units
            .iter()
            .map(|u| Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(u.value.clone(), theme::detail_value()),
            ]))
            .collect()
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter to commit, Esc to discard all changes",
        theme::dim(),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_polynomial_review(coefficients: &[f64], frame: &mut Frame) {
    let area = centered_rect(55, 65, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Polynomial Calibrator — a:Add coeff  d:Remove last  Enter:Commit  Esc:Cancel ")
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = if coefficients.is_empty() {
        vec![Line::from(Span::styled(
            "  (no coefficients — press 'a' to add)",
            theme::dim(),
        ))]
    } else {
        coefficients
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Line::from(vec![
                    Span::styled(format!("  a{} = ", i), theme::detail_label()),
                    Span::styled(format!("{}", v), theme::detail_value()),
                ])
            })
            .collect()
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter to commit, Esc to discard",
        theme::dim(),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_spline_review(points: &[xtce_core::model::types::SplinePoint], order: u32, frame: &mut Frame) {
    let area = centered_rect(55, 65, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Spline Calibrator (order {}) — a:Add point  d:Remove last  Enter:Commit  Esc:Cancel ", order))
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = if points.is_empty() {
        vec![Line::from(Span::styled(
            "  (no points — press 'a' to add)",
            theme::dim(),
        ))]
    } else {
        let mut v = vec![Line::from(Span::styled(
            format!("  {:<18}  {}", "Raw", "Calibrated"),
            theme::detail_label(),
        ))];
        v.extend(points.iter().map(|p| {
            Line::from(vec![
                Span::styled(format!("  {:<18}  ", p.raw), theme::detail_value()),
                Span::styled(format!("{}", p.calibrated), theme::detail_value()),
            ])
        }));
        v
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter to commit, Esc to discard",
        theme::dim(),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout helper
// ─────────────────────────────────────────────────────────────────────────────

/// Return a centered [`Rect`] that is `pct_x`% wide and `pct_y`% tall.
fn centered_rect(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let [_, vert, _] = Layout::vertical([
        Constraint::Percentage((100 - pct_y) / 2),
        Constraint::Percentage(pct_y),
        Constraint::Percentage((100 - pct_y) / 2),
    ])
    .areas(area);

    let [_, popup, _] = Layout::horizontal([
        Constraint::Percentage((100 - pct_x) / 2),
        Constraint::Percentage(pct_x),
        Constraint::Percentage((100 - pct_x) / 2),
    ])
    .areas(vert);

    popup
}
