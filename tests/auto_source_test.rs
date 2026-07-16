use k8s_openapi::api::{
    core::v1::{Node, Pod},
    resource::v1::{ResourceClaim, ResourceSlice},
};
use kubectl_view_rngd::{
    aggregate::{NodeRow, PodEntry, RowSource, aggregate},
    aggregate_dra::{aggregate_dra, partition_nodes},
};
use serde_json::{Value, json};

fn make_node(name: &str, rngd_capacity: Option<&str>) -> Node {
    let mut value: Value = json!({
        "metadata": { "name": name },
        "status": { "capacity": {} }
    });
    if let Some(capacity) = rngd_capacity {
        value["status"]["capacity"]["furiosa.ai/rngd"] = Value::String(capacity.to_string());
    }
    serde_json::from_value(value).expect("valid Node fixture")
}

fn make_pod(namespace: &str, name: &str, node_name: &str, limits: &[&str]) -> Pod {
    let containers = limits
        .iter()
        .enumerate()
        .map(|(i, limit)| {
            json!({
                "name": format!("c{i}"),
                "image": "img",
                "resources": { "limits": { "furiosa.ai/rngd": limit } }
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "metadata": { "namespace": namespace, "name": name },
        "spec": { "nodeName": node_name, "containers": containers },
        "status": { "phase": "Running" }
    }))
    .expect("valid Pod fixture")
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
    reserved_for: Vec<Value>,
) -> ResourceClaim {
    serde_json::from_value(json!({
        "metadata": { "namespace": namespace, "name": name },
        "status": {
            "allocation": {
                "devices": {
                    "results": results,
                }
            },
            "reservedFor": reserved_for,
        }
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

fn pod_consumer(name: &str, uid: &str) -> Value {
    json!({ "resource": "pods", "name": name, "uid": uid })
}

fn row<'a>(rows: &'a [NodeRow], name: &str) -> &'a NodeRow {
    rows.iter()
        .find(|row| row.node_name == name)
        .unwrap_or_else(|| panic!("no row for node {name} in {rows:#?}"))
}

#[test]
fn mixed_cluster_merges_device_plugin_and_dra_rows_per_node() {
    let nodes = vec![make_node("node1", Some("4")), make_node("node2", None)];
    let pods = vec![
        make_pod("ns-dp", "pod-dp", "node1", &["2"]),
        make_pod("ns-dra", "pod-ignored", "node2", &["9"]),
    ];
    let slices = vec![make_slice(
        "slice-1",
        "npu.furiosa.ai",
        "pool-a",
        1,
        1,
        Some("node2"),
        None,
        &["dev0", "dev1", "dev2", "dev3"],
    )];
    let claims = vec![make_claim(
        "ns-dra",
        "claim-a",
        &[
            result("npu.furiosa.ai", "pool-a", "dev0"),
            result("npu.furiosa.ai", "pool-a", "dev1"),
        ],
        vec![pod_consumer("pod-dra", "uid-a")],
    )];

    let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, "npu.furiosa.ai").unwrap();
    let mut rows = aggregate_dra(&dra_nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    rows.extend(aggregate(&dp_nodes, &pods, false).unwrap());
    rows.sort_by(|a, b| a.node_name.cmp(&b.node_name));

    let node1 = row(&rows, "node1");
    assert_eq!(node1.source, Some(RowSource::DevicePlugin));
    assert_eq!(node1.capacity, 4);
    assert_eq!(node1.allocated, 2);
    assert_eq!(
        node1.pods,
        vec![PodEntry {
            namespace: "ns-dp".into(),
            name: "pod-dp".into(),
            count: 2,
            devices: vec![],
        }]
    );

    let node2 = row(&rows, "node2");
    assert_eq!(node2.source, Some(RowSource::Dra));
    assert_eq!(node2.capacity, 4);
    assert_eq!(node2.allocated, 2);
    assert_eq!(
        node2.pods,
        vec![PodEntry {
            namespace: "ns-dra".into(),
            name: "pod-dra".into(),
            count: 2,
            devices: vec!["dev0".into(), "dev1".into()],
        }]
    );
}

#[test]
fn conflict_prefers_dra_for_node_with_both_sources() {
    let nodes = vec![make_node("node1", Some("8"))];
    let pods = vec![make_pod("ns-dp", "pod-dp", "node1", &["8"])];
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
        "ns-dra",
        "claim-a",
        &[result("npu.furiosa.ai", "pool-a", "dev0")],
        vec![pod_consumer("pod-dra", "uid-a")],
    )];

    let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, "npu.furiosa.ai").unwrap();
    assert!(dp_nodes.is_empty());

    let mut rows = aggregate_dra(&dra_nodes, &slices, &claims, "npu.furiosa.ai", false).unwrap();
    rows.extend(aggregate(&dp_nodes, &pods, false).unwrap());

    assert_eq!(rows.len(), 1);
    let node = row(&rows, "node1");
    assert_eq!(node.source, Some(RowSource::Dra));
    assert_eq!(node.capacity, 4);
    assert_eq!(node.allocated, 1);
    assert_eq!(
        node.pods,
        vec![PodEntry {
            namespace: "ns-dra".into(),
            name: "pod-dra".into(),
            count: 1,
            devices: vec!["dev0".into()],
        }]
    );
}
