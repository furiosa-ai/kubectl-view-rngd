use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "kubectl view-rngd",
    bin_name = "kubectl view-rngd",
    about = "Visualize furiosa.ai/rngd allocation across nodes",
    version
)]
pub struct Args {
    #[arg(long, value_name = "PATH", help = "Path to kubeconfig file")]
    pub kubeconfig: Option<PathBuf>,

    #[arg(long, value_name = "NAME", help = "Kube context to use")]
    pub context: Option<String>,

    #[arg(
        long,
        help = "Also list nodes without furiosa.ai/rngd capacity (shown as 0 / 0 with `-` Pods)"
    )]
    pub include_empty: bool,

    #[arg(short, long, help = "Enable debug logging to stderr")]
    pub verbose: bool,
}
