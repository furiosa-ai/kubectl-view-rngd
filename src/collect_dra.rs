use anyhow::{Context, Result};
use k8s_openapi::api::resource::v1::{ResourceClaim, ResourceSlice};
use kube::{Api, Client, Error, api::ListParams};

pub async fn try_fetch_slices(client: Client) -> std::result::Result<Vec<ResourceSlice>, Error> {
    let slices_api: Api<ResourceSlice> = Api::all(client);
    let lp = ListParams::default();
    slices_api.list(&lp).await.map(|slices| slices.items)
}

pub async fn fetch_claims(client: Client) -> Result<Vec<ResourceClaim>> {
    let claims_api: Api<ResourceClaim> = Api::all(client);
    let lp = ListParams::default();
    let claims = claims_api
        .list(&lp)
        .await
        .context("listing resource claims")?;
    Ok(claims.items)
}
