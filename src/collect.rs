use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{Api, Client, api::ListParams};

pub async fn fetch_all(client: Client) -> Result<(Vec<Node>, Vec<Pod>)> {
    let nodes_api: Api<Node> = Api::all(client.clone());
    let pods_api: Api<Pod> = Api::all(client);
    let lp = ListParams::default();

    let (nodes_res, pods_res) = tokio::join!(nodes_api.list(&lp), pods_api.list(&lp));

    let nodes = nodes_res.context("listing nodes")?;
    let pods = pods_res.context("listing pods")?;
    Ok((nodes.items, pods.items))
}
