use kubectl_view_rngd::aggregate::{NodeRow, PodEntry};
use kubectl_view_rngd::render::render;

fn main() {
    let rows = vec![
        NodeRow {
            node_name: "node1".into(),
            capacity: 8,
            allocated: 8,
            pods: vec![
                PodEntry {
                    namespace: "ns1".into(),
                    name: "pod-foo".into(),
                    count: 4,
                },
                PodEntry {
                    namespace: "ns1".into(),
                    name: "pod-bar".into(),
                    count: 4,
                },
            ],
        },
        NodeRow {
            node_name: "node2".into(),
            capacity: 4,
            allocated: 0,
            pods: vec![],
        },
        NodeRow {
            node_name: "node3".into(),
            capacity: 8,
            allocated: 2,
            pods: vec![PodEntry {
                namespace: "ns1".into(),
                name: "pod-baz".into(),
                count: 2,
            }],
        },
        NodeRow {
            node_name: "node4".into(),
            capacity: 8,
            allocated: 8,
            pods: vec![
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-a".into(),
                    count: 4,
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-b".into(),
                    count: 1,
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-c".into(),
                    count: 1,
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-d".into(),
                    count: 1,
                },
                PodEntry {
                    namespace: "ns2".into(),
                    name: "pod-e".into(),
                    count: 1,
                },
            ],
        },
    ];
    println!("{}", render(&rows));
}
