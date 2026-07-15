use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use k8s_openapi::api::{
    core::v1::Node,
    resource::v1::{ResourceClaim, ResourceSlice, ResourceSliceSpec},
};

use crate::RNGD_RESOURCE;
use crate::aggregate::{NodeRow, PodEntry, RowSource};

const ALL_NODES_LABEL: &str = "(all nodes)";
const MULTI_NODE_LABEL: &str = "(multi-node)";

#[derive(Debug, Default)]
struct NodeAccum {
    capacity: i64,
    allocated_devices: HashSet<(String, String)>,
    pod_counts: HashMap<(String, String), i64>,
}

#[derive(Debug, Clone)]
struct PoolInfo {
    node_name: String,
    capacity: i64,
}

pub fn aggregate_dra(
    nodes: &[Node],
    slices: &[ResourceSlice],
    claims: &[ResourceClaim],
    driver: &str,
    include_empty: bool,
) -> Result<Vec<NodeRow>> {
    let mut node_rows: HashMap<String, NodeAccum> = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let name = node
            .metadata
            .name
            .as_deref()
            .ok_or_else(|| anyhow!("node without metadata.name"))?
            .to_string();
        node_rows.entry(name).or_default();
    }

    let pool_infos = collect_pool_infos(slices, driver)?;
    for pool in pool_infos.values() {
        let node = node_rows.entry(pool.node_name.clone()).or_default();
        node.capacity = node
            .capacity
            .checked_add(pool.capacity)
            .ok_or_else(|| anyhow!("DRA capacity sum overflow on node"))?;
    }

    for claim in claims {
        let Some(status) = claim.status.as_ref() else {
            continue;
        };
        let Some(allocation) = status.allocation.as_ref() else {
            continue;
        };
        let Some(device_result) = allocation.devices.as_ref() else {
            continue;
        };
        let Some(results) = device_result.results.as_ref() else {
            continue;
        };

        let namespace = claim.metadata.namespace.clone().unwrap_or_default();
        let reserved_for = status.reserved_for.as_deref().unwrap_or(&[]);
        let mut claim_counts: HashMap<String, i64> = HashMap::new();

        for result in results {
            if result.admin_access == Some(true) || result.driver != driver {
                continue;
            }
            let Some(pool) = pool_infos.get(&result.pool) else {
                tracing::debug!(
                    claim = ?claim.metadata.name,
                    namespace,
                    driver,
                    pool = %result.pool,
                    device = %result.device,
                    "claim references unknown DRA pool; skipping"
                );
                continue;
            };
            let node = node_rows.entry(pool.node_name.clone()).or_default();
            node.allocated_devices
                .insert((result.pool.clone(), result.device.clone()));
            *claim_counts.entry(pool.node_name.clone()).or_insert(0) += 1;
        }

        for (node_name, count) in claim_counts {
            let Some(node) = node_rows.get_mut(&node_name) else {
                continue;
            };
            for consumer in reserved_for {
                if consumer.resource != "pods" {
                    continue;
                }
                let key = (namespace.clone(), consumer.name.clone());
                let entry = node.pod_counts.entry(key).or_insert(0);
                *entry = entry
                    .checked_add(count)
                    .ok_or_else(|| anyhow!("DRA pod count overflow on node"))?;
            }
        }
    }

    let mut rows: Vec<NodeRow> = node_rows
        .into_iter()
        .filter(|(_, node)| include_empty || node.capacity > 0)
        .map(|(node_name, node)| {
            let mut pods: Vec<PodEntry> = node
                .pod_counts
                .into_iter()
                .map(|((namespace, name), count)| PodEntry {
                    namespace,
                    name,
                    count,
                })
                .collect();
            pods.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
            let allocated = i64::try_from(node.allocated_devices.len())
                .map_err(|_| anyhow!("DRA allocated device count overflow on node"))?;
            let source = if node.capacity > 0 || allocated > 0 {
                Some(RowSource::Dra)
            } else {
                None
            };
            Ok(NodeRow {
                node_name,
                source,
                capacity: node.capacity,
                allocated,
                pods,
            })
        })
        .collect::<Result<_>>()?;
    rows.sort_by(|a, b| a.node_name.cmp(&b.node_name));
    Ok(rows)
}

pub fn has_driver_slices(slices: &[ResourceSlice], driver: &str) -> bool {
    slices.iter().any(|slice| slice.spec.driver == driver)
}

pub fn partition_nodes(
    nodes: &[Node],
    slices: &[ResourceSlice],
    driver: &str,
) -> Result<(Vec<Node>, Vec<Node>)> {
    let dra_node_names: HashSet<String> = collect_pool_infos(slices, driver)?
        .into_values()
        .filter_map(|pool| match pool.node_name.as_str() {
            ALL_NODES_LABEL | MULTI_NODE_LABEL => None,
            _ => Some(pool.node_name),
        })
        .collect();

    let mut dra_nodes = Vec::new();
    let mut dp_nodes = Vec::new();
    for node in nodes {
        let name = node
            .metadata
            .name
            .as_deref()
            .ok_or_else(|| anyhow!("node without metadata.name"))?;
        if dra_node_names.contains(name) {
            dra_nodes.push(node.clone());
        } else {
            dp_nodes.push(node.clone());
        }
    }
    Ok((dra_nodes, dp_nodes))
}

pub fn device_plugin_conflicts(dra_nodes: &[Node]) -> Vec<String> {
    dra_nodes
        .iter()
        .filter(|node| {
            node.status
                .as_ref()
                .and_then(|status| status.capacity.as_ref())
                .is_some_and(|capacity| capacity.contains_key(RNGD_RESOURCE))
        })
        .filter_map(|node| node.metadata.name.clone())
        .collect()
}

fn collect_pool_infos(slices: &[ResourceSlice], driver: &str) -> Result<HashMap<String, PoolInfo>> {
    let mut max_generation_by_pool: HashMap<String, i64> = HashMap::new();
    for slice in slices {
        if slice.spec.driver != driver {
            continue;
        }
        let pool = &slice.spec.pool;
        max_generation_by_pool
            .entry(pool.name.clone())
            .and_modify(|generation| *generation = (*generation).max(pool.generation))
            .or_insert(pool.generation);
    }

    let mut pool_infos: HashMap<String, PoolInfo> = HashMap::new();
    for slice in slices {
        if slice.spec.driver != driver {
            continue;
        }
        let pool = &slice.spec.pool;
        let Some(max_generation) = max_generation_by_pool.get(&pool.name) else {
            continue;
        };
        if pool.generation != *max_generation {
            continue;
        }

        let node_name = pool_node_name(&slice.spec);
        let devices = slice.spec.devices.as_ref().map_or(Ok(0_i64), |devices| {
            i64::try_from(devices.len()).map_err(|_| anyhow!("DRA slice device count overflow"))
        })?;
        let entry = pool_infos
            .entry(pool.name.clone())
            .or_insert_with(|| PoolInfo {
                node_name: node_name.clone(),
                capacity: 0,
            });
        if entry.node_name != node_name {
            entry.node_name = MULTI_NODE_LABEL.to_string();
        }
        entry.capacity = entry
            .capacity
            .checked_add(devices)
            .ok_or_else(|| anyhow!("DRA pool capacity overflow"))?;
    }

    Ok(pool_infos)
}

fn pool_node_name(spec: &ResourceSliceSpec) -> String {
    if let Some(node_name) = spec.node_name.as_ref() {
        node_name.clone()
    } else if spec.all_nodes == Some(true) {
        ALL_NODES_LABEL.to_string()
    } else {
        MULTI_NODE_LABEL.to_string()
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::{core::v1::Node, resource::v1::ResourceSlice};
    use serde_json::{Value, json};

    use super::{has_driver_slices, partition_nodes};

    fn make_node(name: &str) -> Node {
        serde_json::from_value(json!({
            "metadata": { "name": name },
        }))
        .expect("valid Node fixture")
    }

    fn make_node_with_capacity(name: &str, capacity: &str) -> Node {
        let mut value: Value = json!({
            "metadata": { "name": name },
            "status": { "capacity": {} }
        });
        value["status"]["capacity"]["furiosa.ai/rngd"] = Value::String(capacity.to_string());
        serde_json::from_value(value).expect("valid Node fixture")
    }

    fn make_slice(driver: &str) -> ResourceSlice {
        serde_json::from_value(json!({
            "metadata": { "name": format!("slice-{driver}") },
            "spec": {
                "driver": driver,
                "pool": {
                    "name": "pool-a",
                    "generation": 1,
                    "resourceSliceCount": 1
                },
                "nodeName": "node1",
                "devices": [{ "name": "dev0" }]
            }
        }))
        .expect("valid ResourceSlice fixture")
    }

    #[test]
    fn has_driver_slices_returns_true_when_matching_driver_exists() {
        let slices = vec![
            make_slice("npu.furiosa.ai"),
            make_slice("other.example.com"),
        ];
        assert!(has_driver_slices(&slices, "npu.furiosa.ai"));
    }

    #[test]
    fn has_driver_slices_returns_false_when_slice_list_is_empty() {
        let slices: Vec<ResourceSlice> = Vec::new();
        assert!(!has_driver_slices(&slices, "npu.furiosa.ai"));
    }

    #[test]
    fn has_driver_slices_returns_false_when_only_other_drivers_exist() {
        let slices = vec![make_slice("other.example.com")];
        assert!(!has_driver_slices(&slices, "npu.furiosa.ai"));
    }

    #[test]
    fn partition_nodes_splits_mixed_cluster() {
        let nodes = vec![make_node("node1"), make_node("node2")];
        let slices = vec![make_slice("npu.furiosa.ai")];
        let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, "npu.furiosa.ai").unwrap();
        assert_eq!(dra_nodes[0].metadata.name.as_deref(), Some("node1"));
        assert_eq!(dp_nodes[0].metadata.name.as_deref(), Some("node2"));
    }

    #[test]
    fn partition_nodes_marks_all_nodes_as_dra_when_all_match() {
        let nodes = vec![make_node("node1")];
        let slices = vec![make_slice("npu.furiosa.ai")];
        let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, "npu.furiosa.ai").unwrap();
        assert_eq!(dra_nodes.len(), 1);
        assert!(dp_nodes.is_empty());
    }

    #[test]
    fn partition_nodes_marks_none_as_dra_when_no_slices() {
        let nodes = vec![make_node("node1")];
        let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &[], "npu.furiosa.ai").unwrap();
        assert!(dra_nodes.is_empty());
        assert_eq!(dp_nodes.len(), 1);
    }

    #[test]
    fn partition_nodes_ignores_other_driver_only_slices() {
        let nodes = vec![make_node_with_capacity("node1", "4")];
        let slices = vec![make_slice("other.example.com")];
        let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, "npu.furiosa.ai").unwrap();
        assert!(dra_nodes.is_empty());
        assert_eq!(dp_nodes.len(), 1);
    }
}
