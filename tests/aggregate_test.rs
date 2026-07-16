use k8s_openapi::api::core::v1::{Node, Pod};
use kubectl_view_rngd::aggregate::{NodeRow, PodEntry, RowSource, aggregate};
use kubectl_view_rngd::render::render;
use serde_json::{Value, json};

fn make_node(name: &str, rngd_capacity: Option<&str>) -> Node {
    let mut v: Value = json!({
        "metadata": { "name": name },
        "status": { "capacity": {} }
    });
    if let Some(cap) = rngd_capacity {
        v["status"]["capacity"]["furiosa.ai/rngd"] = Value::String(cap.to_string());
    }
    serde_json::from_value(v).expect("valid Node fixture")
}

fn make_pod(
    namespace: &str,
    name: &str,
    node_name: Option<&str>,
    phase: &str,
    container_rngd_limits: &[Option<&str>],
) -> Pod {
    let containers: Vec<Value> = container_rngd_limits
        .iter()
        .enumerate()
        .map(|(i, lim)| match lim {
            Some(v) => json!({
                "name": format!("c{i}"),
                "image": "img",
                "resources": { "limits": { "furiosa.ai/rngd": v } }
            }),
            None => json!({ "name": format!("c{i}"), "image": "img" }),
        })
        .collect();
    let mut spec = json!({ "containers": containers });
    if let Some(nn) = node_name {
        spec["nodeName"] = Value::String(nn.to_string());
    }
    let pod_val = json!({
        "metadata": { "namespace": namespace, "name": name },
        "spec": spec,
        "status": { "phase": phase }
    });
    serde_json::from_value(pod_val).expect("valid Pod fixture")
}

fn row<'a>(rows: &'a [NodeRow], name: &str) -> &'a NodeRow {
    rows.iter()
        .find(|r| r.node_name == name)
        .unwrap_or_else(|| panic!("no row for node {name} in {rows:#?}"))
}

#[test]
fn happy_path_sorts_nodes_and_sums_pods() {
    let nodes = vec![make_node("node2", Some("8")), make_node("node1", Some("8"))];
    let pods = vec![
        make_pod("ns1", "pod-foo", Some("node1"), "Running", &[Some("1")]),
        make_pod("ns2", "pod-foo", Some("node1"), "Running", &[Some("1")]),
        make_pod("ns2", "pod-bar", Some("node1"), "Running", &[Some("2")]),
        make_pod(
            "ns1",
            "pod-zzz",
            Some("node2"),
            "Pending",
            &[Some("4"), Some("4")],
        ),
    ];

    let rows = aggregate(&nodes, &pods, false).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].node_name, "node1");
    assert_eq!(rows[1].node_name, "node2");

    let n1 = row(&rows, "node1");
    assert_eq!(n1.source, Some(RowSource::DevicePlugin));
    assert_eq!(n1.capacity, 8);
    assert_eq!(n1.allocated, 4);
    assert_eq!(
        n1.pods,
        vec![
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-foo".into(),
                count: 1,
                devices: vec![]
            },
            PodEntry {
                namespace: "ns2".into(),
                name: "pod-bar".into(),
                count: 2,
                devices: vec![]
            },
            PodEntry {
                namespace: "ns2".into(),
                name: "pod-foo".into(),
                count: 1,
                devices: vec![]
            },
        ]
    );

    let n2 = row(&rows, "node2");
    assert_eq!(n2.source, Some(RowSource::DevicePlugin));
    assert_eq!(n2.capacity, 8);
    assert_eq!(n2.allocated, 8);
    assert_eq!(
        n2.pods,
        vec![PodEntry {
            namespace: "ns1".into(),
            name: "pod-zzz".into(),
            count: 8,
            devices: vec![]
        }]
    );
}

#[test]
fn ignores_succeeded_and_failed_pods() {
    let nodes = vec![make_node("n", Some("4"))];
    let pods = vec![
        make_pod("ns", "done", Some("n"), "Succeeded", &[Some("2")]),
        make_pod("ns", "bad", Some("n"), "Failed", &[Some("2")]),
        make_pod("ns", "live", Some("n"), "Running", &[Some("1")]),
    ];
    let rows = aggregate(&nodes, &pods, false).unwrap();
    let n = row(&rows, "n");
    assert_eq!(n.allocated, 1);
    assert_eq!(n.pods.len(), 1);
    assert_eq!(n.pods[0].name, "live");
}

#[test]
fn ignores_unscheduled_pods() {
    let nodes = vec![make_node("n", Some("4"))];
    let pods = vec![make_pod("ns", "waiting", None, "Pending", &[Some("2")])];
    let rows = aggregate(&nodes, &pods, false).unwrap();
    assert_eq!(row(&rows, "n").allocated, 0);
    assert!(row(&rows, "n").pods.is_empty());
}

#[test]
fn ignores_pods_with_zero_rngd() {
    let nodes = vec![make_node("n", Some("4"))];
    let pods = vec![
        make_pod("ns", "idle", Some("n"), "Running", &[None]),
        make_pod("ns", "busy", Some("n"), "Running", &[Some("3")]),
    ];
    let rows = aggregate(&nodes, &pods, false).unwrap();
    let n = row(&rows, "n");
    assert_eq!(n.allocated, 3);
    assert_eq!(n.pods.len(), 1);
    assert_eq!(n.pods[0].name, "busy");
}

#[test]
fn default_hides_nodes_without_rngd() {
    let nodes = vec![make_node("cpu-only", None), make_node("rngd-1", Some("2"))];
    let rows = aggregate(&nodes, &[], false).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].node_name, "rngd-1");
}

#[test]
fn include_empty_shows_nodes_without_rngd() {
    let nodes = vec![make_node("cpu-only", None), make_node("rngd-1", Some("2"))];
    let rows = aggregate(&nodes, &[], true).unwrap();
    assert_eq!(rows.len(), 2);
    let cpu = row(&rows, "cpu-only");
    assert_eq!(cpu.source, None);
    assert_eq!(cpu.capacity, 0);
    assert_eq!(cpu.allocated, 0);
    assert!(cpu.pods.is_empty());
}

#[test]
fn node_with_rngd_but_no_pods_renders_dash() {
    let nodes = vec![make_node("idle", Some("8"))];
    let rows = aggregate(&nodes, &[], false).unwrap();
    let out = render(&rows, false);
    assert!(out.contains("idle"), "output missing node name:\n{out}");
    assert!(out.contains("0 / 8"), "output missing ratio:\n{out}");
    assert!(
        out.lines().any(|l| l.contains("idle") && l.contains(" - ")),
        "expected dash in Pods cell:\n{out}"
    );
}

#[test]
fn empty_cluster_renders_header_only() {
    let rows = aggregate(&[], &[], false).unwrap();
    assert!(rows.is_empty());
    let out = render(&rows, false);
    assert!(out.contains("Node") && out.contains("Pods"));
}

#[test]
fn malformed_quantity_surfaces_error() {
    let nodes = vec![make_node("n", Some("not-a-number"))];
    let err = aggregate(&nodes, &[], false).expect_err("expected parse failure");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("furiosa.ai/rngd") && msg.contains("not-a-number"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn render_snapshot_matches_expected_layout() {
    let nodes = vec![make_node("node1", Some("8")), make_node("node2", Some("8"))];
    let pods = vec![
        make_pod("ns1", "pod-foo", Some("node1"), "Running", &[Some("1")]),
        make_pod("ns2", "pod-foo", Some("node1"), "Running", &[Some("1")]),
        make_pod("ns2", "pod-bar", Some("node1"), "Running", &[Some("2")]),
        make_pod("ns1", "pod-zzz", Some("node2"), "Running", &[Some("8")]),
    ];
    let rows = aggregate(&nodes, &pods, false).unwrap();
    let out = render(&rows, false);

    // Table must be bordered Unicode and contain every row we expect.
    for needle in [
        "│ Node",
        "│ Source",
        "│ Usage",
        "│ Pods",
        "node1",
        "device-plugin",
        "4 / 8",
        "ns1/pod-foo (1)",
        "ns2/pod-bar (2)",
        "ns2/pod-foo (1)",
        "node2",
        "8 / 8",
        "ns1/pod-zzz (8)",
    ] {
        assert!(
            out.contains(needle),
            "missing {needle:?} in rendered table:\n{out}"
        );
    }

    // 2nd+ pod rows for node1 must not repeat "node1" in the Node column.
    let node1_line_count = out.matches("node1").count();
    assert_eq!(
        node1_line_count, 1,
        "node name should appear exactly once per node group:\n{out}"
    );
}

#[test]
fn render_with_show_devices_uses_names_for_dra_rows_only() {
    let rows = vec![
        NodeRow {
            node_name: "node-dp".into(),
            source: Some(RowSource::DevicePlugin),
            capacity: 4,
            allocated: 2,
            pods: vec![PodEntry {
                namespace: "ns1".into(),
                name: "pod-dp".into(),
                count: 2,
                devices: vec![],
            }],
        },
        NodeRow {
            node_name: "node-dra".into(),
            source: Some(RowSource::Dra),
            capacity: 4,
            allocated: 2,
            pods: vec![PodEntry {
                namespace: "ns2".into(),
                name: "pod-dra".into(),
                count: 2,
                devices: vec!["dev0".into(), "dev1".into()],
            }],
        },
    ];
    let out = render(&rows, true);

    for needle in [
        "node-dp",
        "ns1/pod-dp (2)",
        "node-dra",
        "ns2/pod-dra (dev0,dev1)",
    ] {
        assert!(
            out.contains(needle),
            "missing {needle:?} in rendered table:\n{out}"
        );
    }
}
