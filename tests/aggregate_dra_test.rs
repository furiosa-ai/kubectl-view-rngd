use k8s_openapi::api::{
    core::v1::Node,
    resource::v1::{ResourceClaim, ResourceSlice},
};
use kubectl_view_rngd::aggregate::{NodeRow, PodEntry, RowSource};
use kubectl_view_rngd::aggregate_dra::aggregate_dra;
use serde_json::{Value, json};

fn make_node(name: &str) -> Node {
    serde_json::from_value(json!({
        "metadata": { "name": name }
    }))
    .expect("valid Node fixture")
}

#[allow(clippy::too_many_arguments)]
fn make_slice(
    name: &str,
    driver: &str,
    pool: &str,
    generation: i64,
    resource_slice_count: i64,
    node_name: Option<&str>,
    all_nodes: Option<bool>,
    devices: &[&str],
) -> ResourceSlice {
    let mut spec = json!({
        "driver": driver,
        "pool": {
            "name": pool,
            "generation": generation,
            "resourceSliceCount": resource_slice_count
        },
        "devices": devices.iter().map(|device| json!({ "name": device })).collect::<Vec<_>>()
    });
    if let Some(node_name) = node_name {
        spec["nodeName"] = Value::String(node_name.to_string());
    }
    if let Some(all_nodes) = all_nodes {
        spec["allNodes"] = Value::Bool(all_nodes);
    }
    serde_json::from_value(json!({
        "metadata": { "name": name },
        "spec": spec,
    }))
    .expect("valid ResourceSlice fixture")
}

fn make_claim(
    namespace: &str,
    name: &str,
    results: &[Value],
    reserved_for: Option<Vec<Value>>,
) -> ResourceClaim {
    let mut status = json!({
        "allocation": {
            "devices": {
                "results": results,
            }
        }
    });
    if let Some(reserved_for) = reserved_for {
        status["reservedFor"] = Value::Array(reserved_for);
    }
    serde_json::from_value(json!({
        "metadata": { "namespace": namespace, "name": name },
        "status": status,
    }))
    .expect("valid ResourceClaim fixture")
}

fn result(driver: &str, pool: &str, device: &str) -> Value {
    json!({
        "driver": driver,
        "pool": pool,
        "device": device,
        "request": "default",
    })
}

fn admin_result(driver: &str, pool: &str, device: &str) -> Value {
    json!({
        "driver": driver,
        "pool": pool,
        "device": device,
        "request": "default",
        "adminAccess": true,
    })
}

fn pod_consumer(name: &str, uid: &str) -> Value {
    json!({
        "resource": "pods",
        "name": name,
        "uid": uid,
    })
}

fn row<'a>(rows: &'a [NodeRow], name: &str) -> &'a NodeRow {
    rows.iter()
        .find(|r| r.node_name == name)
        .unwrap_or_else(|| panic!("no row for node {name} in {rows:#?}"))
}

#[test]
fn basic_claim_maps_to_node_usage_and_pod() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0", "dev1", "dev2", "dev3"],
    )];
    let claims = vec![make_claim(
        "ns1",
        "claim-a",
        &[
            result("npu.furiosa.ai", "pool-a", "dev0"),
            result("npu.furiosa.ai", "pool-a", "dev1"),
        ],
        Some(vec![pod_consumer("pod-a", "uid-a")]),
    )];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.source, Some(RowSource::Dra));
    assert_eq!(node.capacity, 4);
    assert_eq!(node.allocated, 2);
    assert_eq!(
        node.pods,
        vec![PodEntry {
            namespace: "ns1".into(),
            name: "pod-a".into(),
            count: 2,
            devices: vec!["dev0".into(), "dev1".into()],
        }]
    );
}

#[test]
fn admin_access_result_is_excluded() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0", "dev1"],
    )];
    let claims = vec![make_claim(
        "ns1",
        "claim-a",
        &[
            result("npu.furiosa.ai", "pool-a", "dev0"),
            admin_result("npu.furiosa.ai", "pool-a", "dev1"),
        ],
        Some(vec![pod_consumer("pod-a", "uid-a")]),
    )];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.allocated, 1);
    assert_eq!(node.pods[0].count, 1);
}

#[test]
fn stale_generation_is_ignored() {
    let nodes = vec![make_node("node1")];
    let slices = vec![
        make_slice(
            "slice-old",
            "npu.furiosa.ai",
            "pool-a",
            1,
            1,
            Some("node1"),
            None,
            &["dev0", "dev1", "dev2", "dev3"],
        ),
        make_slice(
            "slice-new-1",
            "npu.furiosa.ai",
            "pool-a",
            2,
            2,
            Some("node1"),
            None,
            &["dev4"],
        ),
        make_slice(
            "slice-new-2",
            "npu.furiosa.ai",
            "pool-a",
            2,
            2,
            Some("node1"),
            None,
            &["dev5"],
        ),
    ];

    let rows = aggregate_dra(&nodes, &slices, &[], "npu.furiosa.ai", false).unwrap();
    assert_eq!(row(&rows, "node1").capacity, 2);
}

#[test]
fn split_pool_sums_same_generation_slices() {
    let nodes = vec![make_node("node1")];
    let slices = vec![
        make_slice(
            "slice-1",
            "npu.furiosa.ai",
            "pool-a",
            3,
            2,
            Some("node1"),
            None,
            &["dev0", "dev1"],
        ),
        make_slice(
            "slice-2",
            "npu.furiosa.ai",
            "pool-a",
            3,
            2,
            Some("node1"),
            None,
            &["dev2", "dev3"],
        ),
    ];

    let rows = aggregate_dra(&nodes, &slices, &[], "npu.furiosa.ai", false).unwrap();
    assert_eq!(row(&rows, "node1").capacity, 4);
}

#[test]
fn shared_device_counts_once_in_allocated_and_once_per_pod() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0"],
    )];
    let claims = vec![
        make_claim(
            "ns1",
            "claim-a",
            &[result("npu.furiosa.ai", "pool-a", "dev0")],
            Some(vec![pod_consumer("pod-a", "uid-a")]),
        ),
        make_claim(
            "ns1",
            "claim-b",
            &[result("npu.furiosa.ai", "pool-a", "dev0")],
            Some(vec![pod_consumer("pod-b", "uid-b")]),
        ),
    ];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.allocated, 1);
    assert_eq!(
        node.pods,
        vec![
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-a".into(),
                count: 1,
                devices: vec!["dev0".into()],
            },
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-b".into(),
                count: 1,
                devices: vec!["dev0".into()],
            },
        ]
    );
}

#[test]
fn other_driver_slices_and_claims_are_ignored() {
    let nodes = vec![make_node("node1")];
    let slices = vec![
        make_slice(
            "slice-good",
            "npu.furiosa.ai",
            "pool-a",
            1,
            1,
            Some("node1"),
            None,
            &["dev0", "dev1"],
        ),
        make_slice(
            "slice-other",
            "other.example.com",
            "pool-b",
            1,
            1,
            Some("node1"),
            None,
            &["dev9", "dev10", "dev11"],
        ),
    ];
    let claims = vec![
        make_claim(
            "ns1",
            "claim-good",
            &[result("npu.furiosa.ai", "pool-a", "dev0")],
            Some(vec![pod_consumer("pod-a", "uid-a")]),
        ),
        make_claim(
            "ns1",
            "claim-other",
            &[result("other.example.com", "pool-b", "dev9")],
            Some(vec![pod_consumer("pod-b", "uid-b")]),
        ),
    ];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.capacity, 2);
    assert_eq!(node.allocated, 1);
    assert_eq!(node.pods.len(), 1);
    assert_eq!(node.pods[0].name, "pod-a");
}

#[test]
fn include_empty_controls_zero_capacity_nodes() {
    let nodes = vec![make_node("cpu-only"), make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0"],
    )];

    let rows = aggregate_dra(&nodes, &slices, &[], "npu.furiosa.ai", false).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].node_name, "node1");

    let rows = aggregate_dra(&nodes, &slices, &[], "npu.furiosa.ai", true).unwrap();
    assert_eq!(rows.len(), 2);
    let cpu = row(&rows, "cpu-only");
    assert_eq!(cpu.source, None);
    assert_eq!(cpu.capacity, 0);
    assert_eq!(cpu.allocated, 0);
    assert!(cpu.pods.is_empty());
}

#[test]
fn allocated_but_unreserved_claim_has_no_pod_row() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0", "dev1"],
    )];
    let claims = vec![make_claim(
        "ns1",
        "claim-a",
        &[result("npu.furiosa.ai", "pool-a", "dev0")],
        None,
    )];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.allocated, 1);
    assert!(node.pods.is_empty());
}

#[test]
fn claim_referencing_unknown_pool_is_skipped() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0"],
    )];
    let claims = vec![make_claim(
        "ns1",
        "claim-a",
        &[result("npu.furiosa.ai", "pool-missing", "dev0")],
        Some(vec![pod_consumer("pod-a", "uid-a")]),
    )];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.allocated, 0);
    assert!(node.pods.is_empty());
}

#[test]
fn claim_shared_by_multiple_pods_counts_devices_once() {
    let nodes = vec![make_node("node1")];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node1"),
        None,
        &["dev0", "dev1", "dev2", "dev3"],
    )];
    let claims = vec![make_claim(
        "ns1",
        "shared-claim",
        &[
            result("npu.furiosa.ai", "pool-a", "dev0"),
            result("npu.furiosa.ai", "pool-a", "dev1"),
        ],
        Some(vec![
            pod_consumer("pod-a", "uid-a"),
            pod_consumer("pod-b", "uid-b"),
            pod_consumer("pod-c", "uid-c"),
        ]),
    )];

    let rows = aggregate_dra(&nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    let node = row(&rows, "node1");
    assert_eq!(node.capacity, 4);
    assert_eq!(node.allocated, 2);
    assert_eq!(
        node.pods,
        vec![
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-a".into(),
                count: 2,
                devices: vec!["dev0".into(), "dev1".into()],
            },
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-b".into(),
                count: 2,
                devices: vec!["dev0".into(), "dev1".into()],
            },
            PodEntry {
                namespace: "ns1".into(),
                name: "pod-c".into(),
                count: 2,
                devices: vec!["dev0".into(), "dev1".into()],
            },
        ]
    );
}
