use super::{ctrl_c_monitor, send_product_change};
use crate::config::Config;
use crate::ext::anyhow::{Context, Result};
use crate::service::serve;
use crate::task::compile::ProductSet;

pub async fn run(conf: &Config) -> Result<()> {
    let _ = ctrl_c_monitor();
    super::build::run(conf).await.dot()?;
    let server = serve::run(&conf).await;
    // the server waits for the first product change before starting
    send_product_change(ProductSet::from(vec![]));

    server.await??;
    Ok(())
}
