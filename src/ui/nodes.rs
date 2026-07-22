use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
};

use crate::state::AppState;

pub fn draw(f: &mut ratatui::Frame, state: &mut AppState<'_>, area: Rect) {
    let mut items = vec![];
    for cc in &state.countries {
        let is_selected = state.selected_country.as_deref() == Some(cc.alpha2());
        let prefix = if is_selected { "[*]" } else { "[ ]" };
        let content = format!("{} {} - {}", prefix, cc.alpha2(), cc.name());
        items.push(ListItem::new(content));
    }

    let title = if state.switch_in_progress {
        "Regions (Up/Down to select, Enter to apply, 'a' for Auto) [switching...]"
    } else {
        "Regions (Up/Down to select, Enter to apply, 'a' for Auto)"
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, area, &mut state.node_list_state);
}
