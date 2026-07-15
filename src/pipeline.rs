use anyhow::{Context, Result};
use kube::{Client, Error};

use crate::{
    aggregate::{NodeRow, aggregate},
    aggregate_dra::{aggregate_dra, device_plugin_conflicts, has_driver_slices, partition_nodes},
    cli::{Args, Source},
    collect::fetch_all,
    collect_dra::{fetch_claims, try_fetch_slices},
};

pub async fn collect_rows(client: Client, args: &Args) -> Result<Vec<NodeRow>> {
    match args.source {
        Source::DevicePlugin => {
            let (nodes, pods) = fetch_all(client).await?;
            aggregate(&nodes, &pods, args.include_empty)
        }
        Source::Dra => {
            let (all_res, slices_res, claims_res) = tokio::join!(
                fetch_all(client.clone()),
                try_fetch_slices(client.clone()),
                fetch_claims(client),
            );
            let (nodes, _) = all_res?;
            let slices = slices_res.context("listing resource slices")?;
            let claims = claims_res?;
            aggregate_dra(&nodes, &slices, &claims, &args.driver, args.include_empty)
        }
        Source::Auto => {
            let (all_res, slices_res) =
                tokio::join!(fetch_all(client.clone()), try_fetch_slices(client.clone()));
            let (nodes, pods) = all_res?;
            let slices = match slices_res {
                Ok(slices) => slices,
                Err(err) if should_fallback_to_device_plugin(&err) => {
                    tracing::debug!(error = %err, "DRA auto-detection unavailable; falling back to device-plugin mode");
                    return aggregate(&nodes, &pods, args.include_empty);
                }
                Err(err) => return Err(err).context("listing resource slices"),
            };

            if !has_driver_slices(&slices, &args.driver) {
                return aggregate(&nodes, &pods, args.include_empty);
            }

            let (dra_nodes, dp_nodes) = partition_nodes(&nodes, &slices, &args.driver)?;
            for node_name in device_plugin_conflicts(&dra_nodes) {
                tracing::debug!(
                    node = %node_name,
                    "node has both DRA slices and furiosa.ai/rngd capacity; preferring DRA"
                );
            }

            let claims = fetch_claims(client).await?;
            let mut rows = aggregate_dra(
                &dra_nodes,
                &slices,
                &claims,
                &args.driver,
                args.include_empty,
            )?;
            rows.extend(aggregate(&dp_nodes, &pods, args.include_empty)?);
            rows.sort_by(|a, b| a.node_name.cmp(&b.node_name));
            Ok(rows)
        }
    }
}

fn should_fallback_to_device_plugin(err: &Error) -> bool {
    match err {
        Error::Api(resp) => resp.code == 403 || resp.code == 404,
        _ => false,
    }
}
