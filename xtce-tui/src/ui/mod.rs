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
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use xtce_core::ValidationError;

use crate::app::{App, CreateStep, Focus, TypeVariant};
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
        render_help_overlay(frame);
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
        let title = match &ea.node_id {
            tree::NodeId::TmContainer(_, _) => "Add ParameterRef entry",
            tree::NodeId::CmdMetaCommand(_, _) => "Add ArgumentRef entry",
            _ => "Add entry",
        };
        render_picker_overlay(title, &ea.filter, &ea.items, ea.cursor, frame);
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
            } else if app.search_matches.contains(&node.node_id) {
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
        let err_count = app.validation_errors.len();
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
        let hint = if app.show_errors || app.show_help {
            " Esc:Close  "
        } else {
            " q:Quit  Tab:Focus  ←→/hl:Expand  ↑↓/jk:Navigate  r:Reload  s/^W:Save  /:Search  a:Add  d:Del  A:AddEntry  x:RemLast  e:Errors  ?:Help "
        };
        spans.push(Span::styled(hint, theme::dim()));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Overlay renderers
// ─────────────────────────────────────────────────────────────────────────────

fn render_type_variant_select(selector_cursor: usize, frame: &mut Frame) {
    let area = centered_rect(40, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select type variant — Enter:Confirm  Esc:Cancel ")
        .border_style(Style::default().fg(theme::BORDER_FOCUSED));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = TypeVariant::all()
        .iter()
        .map(|v| ListItem::new(Span::raw(format!("  {}", v.label()))))
        .collect();

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selector_cursor));

    let list = List::new(items).highlight_style(theme::selected_focused());
    frame.render_stateful_widget(list, inner, &mut state);
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

fn render_errors_overlay(app: &App, frame: &mut Frame) {
    let area = centered_rect(75, 70, frame.area());
    frame.render_widget(Clear, area);

    let has_errors = !app.validation_errors.is_empty();
    let border_color = if has_errors { Color::Red } else { theme::BORDER_FOCUSED };
    let title = if has_errors {
        format!(" Validation Errors ({}) — e/Esc to close ", app.validation_errors.len())
    } else {
        " Validation Errors — e/Esc to close ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line<'static>> = if app.validation_errors.is_empty() {
        vec![Line::from(Span::styled("  No validation errors", theme::dim()))]
    } else {
        app.validation_errors
            .iter()
            .flat_map(|e| error_lines(e))
            .collect()
    };

    let content_height = lines.len();
    let visible_height = inner.height as usize;
    let scroll = app
        .detail_scroll
        .min(content_height.saturating_sub(visible_height));

    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

fn error_lines(e: &ValidationError) -> Vec<Line<'static>> {
    match e {
        ValidationError::UnresolvedReference { name, context } => vec![
            Line::from(vec![
                Span::styled("  Unresolved:  ", theme::error()),
                Span::styled(name.clone(), theme::detail_value()),
            ]),
            Line::from(Span::styled(format!("    in {}", context), theme::dim())),
            Line::from(""),
        ],
        ValidationError::CyclicInheritance { name } => vec![
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
    }
}

fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(50, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keybindings — ?/Esc to close ")
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
        ("Edit (tree focus)", ""),
        ("  i", "Rename selected item"),
        ("  C", "Edit description"),
        ("", ""),
        ("Create / Delete (tree focus)", ""),
        ("  a", "Add item (sibling or child)"),
        ("  d", "Delete selected item"),
        ("  A", "Add entry to container / MetaCommand"),
        ("  x", "Remove last entry from container / MetaCommand"),
        ("", ""),
        ("File", ""),
        ("  r", "Reload from disk"),
        ("  s / Ctrl+W", "Save to disk"),
        ("", ""),
        ("Search", ""),
        ("  /", "Open search prompt"),
        ("  n / N", "Next / previous match"),
        ("  Esc / Enter", "Close search prompt"),
        ("", ""),
        ("Overlays", ""),
        ("  e", "Toggle error list"),
        ("  ?", "Toggle this help"),
        ("  Esc", "Close overlay"),
        ("", ""),
        ("  q / Ctrl+C", "Quit"),
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
