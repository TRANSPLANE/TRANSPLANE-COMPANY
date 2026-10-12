use clap::Parser;
use company_application::{App, AppConfig};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Parser)]
#[command(version, about = "TransPlane Company server")]
struct Args {
    #[arg(long, env = "COMPANY_DATA_DIR", default_value = "company-data")]
    data_dir: PathBuf,
    #[arg(long, env = "COMPANY_DATABASE_URL", hide_env_values = true)]
    database_url: Option<String>,
    #[arg(long, env = "COMPANY_BIND", default_value = "127.0.0.1:4177")]
    bind: SocketAddr,
    #[arg(long, env = "COMPANY_WEB_DIR")]
    web_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "company_server=info".into()),
        )
        .init();
    let args = Args::parse();
    let app = App::open(AppConfig {
        data_dir: args.data_dir,
        database_url: args.database_url,
    })
    .await?;
    company_server::serve(app, args.bind, args.web_dir).await
}
