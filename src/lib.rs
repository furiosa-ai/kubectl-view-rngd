pub mod aggregate;
pub mod aggregate_dra;
pub mod cli;
pub mod collect;
pub mod collect_dra;
pub mod kube_client;
pub mod pipeline;
pub mod render;

pub const DEFAULT_DRA_DRIVER: &str = "npu.furiosa.ai";
pub const RNGD_RESOURCE: &str = "furiosa.ai/rngd";
