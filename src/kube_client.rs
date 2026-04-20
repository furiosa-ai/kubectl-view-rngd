use std::path::Path;

use anyhow::{Context, Result};
use kube::{
    Client, Config,
    config::{KubeConfigOptions, Kubeconfig},
};

pub async fn build_client(kubeconfig_path: Option<&Path>, context: Option<&str>) -> Result<Client> {
    let config = match (kubeconfig_path, context) {
        (None, None) => Config::infer().await.context("inferring kube config")?,
        (Some(path), ctx) => {
            let kc = Kubeconfig::read_from(path)
                .with_context(|| format!("reading kubeconfig from {}", path.display()))?;
            let opts = KubeConfigOptions {
                context: ctx.map(String::from),
                ..Default::default()
            };
            Config::from_custom_kubeconfig(kc, &opts)
                .await
                .context("loading kubeconfig")?
        }
        (None, Some(ctx)) => {
            let kc = Kubeconfig::read().context("reading default kubeconfig")?;
            let opts = KubeConfigOptions {
                context: Some(ctx.to_string()),
                ..Default::default()
            };
            Config::from_custom_kubeconfig(kc, &opts)
                .await
                .context("loading kubeconfig")?
        }
    };
    Client::try_from(config).context("building kube client")
}
