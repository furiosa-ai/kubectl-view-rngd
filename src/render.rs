use comfy_table::{ContentArrangement, Table, TableComponent, presets::UTF8_FULL};

use crate::aggregate::NodeRow;

fn short_device_name(name: &str) -> &str {
    let shortened = name.trim_start_matches(|ch: char| !ch.is_ascii_digit());
    if !shortened.is_empty() && shortened.chars().all(|ch| ch.is_ascii_digit()) {
        shortened
    } else {
        name
    }
}

pub fn render(rows: &[NodeRow], show_devices: bool) -> String {
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
                .map(|p| {
                    if show_devices && !p.devices.is_empty() {
                        let mut devices: Vec<&str> = p
                            .devices
                            .iter()
                            .map(|device| short_device_name(device))
                            .collect();
                        devices.sort_by(|a, b| match (a.parse::<u64>(), b.parse::<u64>()) {
                            (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                            _ => a.cmp(b),
                        });
                        format!("{}/{} [{}]", p.namespace, p.name, devices.join(", "))
                    } else {
                        format!("{}/{} ({})", p.namespace, p.name, p.count)
                    }
                })
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

#[cfg(test)]
mod tests {
    use super::short_device_name;

    #[test]
    fn short_device_name_strips_only_digit_suffixes() {
        assert_eq!(short_device_name("npu0"), "0");
        assert_eq!(short_device_name("dev12"), "12");
        assert_eq!(short_device_name("abc"), "abc");
        assert_eq!(short_device_name("a1b2"), "a1b2");
    }
}
