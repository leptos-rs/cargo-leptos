use crate::config::Config;
use crate::ext::anyhow::{Context, Result};
use crate::service::serve;
use crate::signal::{ProductChange, ProductSet};

pub async fn serve(conf: &Config) -> Result<()> {
    super::build::build(conf).await.dot()?;
    let server = serve::spawn(conf).await;
    // the server waits for the first product change before starting
    ProductChange::send(ProductSet::empty());

    server.await??;
    Ok(())
}
