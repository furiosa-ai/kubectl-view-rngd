use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::DEFAULT_DRA_DRIVER;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Source {
    #[default]
    Auto,
    DevicePlugin,
    Dra,
}

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

    #[arg(
        long,
        value_enum,
        default_value_t = Source::Auto,
        help = "Data source to use: auto-detect each node from DRA or device-plugin data, or force device-plugin / DRA mode"
    )]
    pub source: Source,

    #[arg(
        long,
        default_value = DEFAULT_DRA_DRIVER,
        value_name = "NAME",
        help = "DRA driver whose devices are shown"
    )]
    pub driver: String,

    #[arg(short, long, help = "Enable debug logging to stderr")]
    pub verbose: bool,
}
