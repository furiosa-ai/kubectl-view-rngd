use kubectl_view_rngd::aggregate::{NodeRow, PodEntry, RowSource};
use kubectl_view_rngd::render::render;

fn main() {
    let rows = vec![
        NodeRow {
            node_name: "node1".into(),
            source: Some(RowSource::DevicePlugin),
            capacity: 8,
            allocated: 8,
            pods: vec![
                PodEntry {
                    namespace: "ns1".into(),
                    name: "pod-foo".into(),
                    count: 4,
                    devices: vec![],
                },
                PodEntry {
                    namespace: "ns1".into(),
                    name: "pod-bar".into(),
                    count: 4,
                    devices: vec![],
                },
            ],
        },
        NodeRow {
            node_name: "node2".into(),
            source: Some(RowSource::DevicePlugin),
            capacity: 4,
            allocated: 0,
            pods: vec![],
        },
        NodeRow {
            node_name: "node3".into(),
            source: Some(RowSource::Dra),
            capacity: 8,
            allocated: 2,
            pods: vec![PodEntry {
                namespace: "ns1".into(),
                name: "pod-baz".into(),
                count: 2,
                devices: vec!["dev0".into(), "dev1".into()],
            }],
        },
        NodeRow {
            node_name: "node4".into(),
            source: Some(RowSource::Dra),
            capacity: 8,
            allocated: 8,
            pods: vec![
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-a".into(),
                    count: 4,
                    devices: vec!["dev0".into(), "dev1".into(), "dev2".into(), "dev3".into()],
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-b".into(),
                    count: 1,
                    devices: vec!["dev0".into()],
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-c".into(),
                    count: 1,
                    devices: vec!["dev1".into()],
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-d".into(),
                    count: 1,
                    devices: vec!["dev2".into()],
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-e".into(),
                    count: 1,
                    devices: vec!["dev3".into()],
                },
            ],
        },
    ];
    println!("{}", render(&rows, false));
}
