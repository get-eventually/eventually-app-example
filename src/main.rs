use envconfig::Envconfig;
use eventually_app_example::{run, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::init_from_env()?;
    run(config).await
}
