use std::collections::HashMap;

use anyhow::{Result, anyhow};
use k8s_openapi::api::core::v1::{Node, Pod};

use crate::RNGD_RESOURCE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodEntry {
    pub namespace: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRow {
    pub node_name: String,
    pub capacity: i64,
    pub allocated: i64,
    pub pods: Vec<PodEntry>,
}

pub fn aggregate(nodes: &[Node], pods: &[Pod], include_empty: bool) -> Result<Vec<NodeRow>> {
    let mut node_cap: HashMap<String, i64> = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let name = node
            .metadata
            .name
            .as_deref()
            .ok_or_else(|| anyhow!("node without metadata.name"))?
            .to_string();
        let cap = node
            .status
            .as_ref()
            .and_then(|s| s.capacity.as_ref())
            .and_then(|c| c.get(RNGD_RESOURCE))
            .map(|q| parse_int_quantity(&q.0))
            .transpose()?
            .unwrap_or(0);
        node_cap.insert(name, cap);
    }

    let mut node_pods: HashMap<String, Vec<PodEntry>> = HashMap::new();
    for pod in pods {
        let Some(spec) = pod.spec.as_ref() else {
            continue;
        };
        let Some(node_name) = spec.node_name.as_deref() else {
            continue;
        };
        let phase = pod
            .status
            .as_ref()
            .and_then(|s| s.phase.as_deref())
            .unwrap_or("");
        if phase != "Pending" && phase != "Running" {
            continue;
        }
        let count = sum_pod_rngd(pod)?;
        if count == 0 {
            continue;
        }
        if !node_cap.contains_key(node_name) {
            tracing::debug!(
                node = %node_name,
                pod = ?pod.metadata.name,
                "pod scheduled on node not in listed node set; skipping"
            );
            continue;
        }
        let ns = pod.metadata.namespace.clone().unwrap_or_default();
        let name = pod.metadata.name.clone().unwrap_or_default();
        node_pods
            .entry(node_name.to_string())
            .or_default()
            .push(PodEntry {
                namespace: ns,
                name,
                count,
            });
    }

    let mut rows: Vec<NodeRow> = node_cap
        .into_iter()
        .filter(|(_, cap)| include_empty || *cap > 0)
        .map(|(name, cap)| {
            let mut pods = node_pods.remove(&name).unwrap_or_default();
            pods.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
            let allocated: i64 = pods.iter().map(|p| p.count).sum();
            NodeRow {
                node_name: name,
                capacity: cap,
                allocated,
                pods,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.node_name.cmp(&b.node_name));
    Ok(rows)
}

fn sum_pod_rngd(pod: &Pod) -> Result<i64> {
    let Some(spec) = pod.spec.as_ref() else {
        return Ok(0);
    };
    let mut sum: i64 = 0;
    for c in &spec.containers {
        let Some(reqs) = c.resources.as_ref() else {
            continue;
        };
        let Some(limits) = reqs.limits.as_ref() else {
            continue;
        };
        let Some(q) = limits.get(RNGD_RESOURCE) else {
            continue;
        };
        let v = parse_int_quantity(&q.0)?;
        sum = sum
            .checked_add(v)
            .ok_or_else(|| anyhow!("rngd sum overflow on pod"))?;
    }
    Ok(sum)
}

fn parse_int_quantity(raw: &str) -> Result<i64> {
    raw.trim()
        .parse::<i64>()
        .map_err(|e| anyhow!("failed to parse {RNGD_RESOURCE} quantity {raw:?} as integer: {e}"))
}
