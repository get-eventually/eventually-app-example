use envconfig::Envconfig;
use eventually_app_example::{self, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::init_from_env()?;
    eventually_app_example::run(config).await
}
