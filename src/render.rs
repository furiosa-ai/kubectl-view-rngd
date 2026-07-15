use comfy_table::{ContentArrangement, Table, TableComponent, presets::UTF8_FULL};

use crate::aggregate::NodeRow;

pub fn render(rows: &[NodeRow]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        // UTF8_FULL defaults to dashed inner lines (┆, ╌); prefer solid.
        .set_style(TableComponent::VerticalLines, '│')
        .set_style(TableComponent::HorizontalLines, '─')
        .set_style(TableComponent::MiddleIntersections, '┼')
        .set_style(TableComponent::LeftBorderIntersections, '├')
        .set_style(TableComponent::RightBorderIntersections, '┤')
        .set_style(TableComponent::TopBorderIntersections, '┬')
        .set_style(TableComponent::BottomBorderIntersections, '┴')
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(["Node", "Source", "Usage", "Pods"]);

    for row in rows {
        let source = row
            .source
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".to_string());
        let pods_cell = if row.pods.is_empty() {
            "-".to_string()
        } else {
            row.pods
                .iter()
                .map(|p| format!("{}/{} ({})", p.namespace, p.name, p.count))
                .collect::<Vec<_>>()
                .join("\n")
        };
        table.add_row([
            row.node_name.clone(),
            source,
            format!("{} / {}", row.allocated, row.capacity),
            pods_cell,
        ]);
    }

    table.to_string()
}
