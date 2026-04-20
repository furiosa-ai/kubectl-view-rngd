use anyhow::Result;
use clap::Parser;
use kubectl_view_rngd::{
    aggregate::aggregate, cli::Args, collect::fetch_all, kube_client::build_client, render::render,
};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    // rustls 0.23+ requires an explicit process-wide CryptoProvider.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args = Args::parse();
    init_tracing(args.verbose);

    let client = build_client(args.kubeconfig.as_deref(), args.context.as_deref()).await?;
    let (nodes, pods) = fetch_all(client).await?;
    let rows = aggregate(&nodes, &pods, args.include_empty)?;
    println!("{}", render(&rows));
    Ok(())
}

fn init_tracing(verbose: bool) {
    let level = if verbose { "debug" } else { "warn" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}
