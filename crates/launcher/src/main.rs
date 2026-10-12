#![cfg_attr(windows, windows_subsystem = "windows")]

mod desktop;
mod instance;

use anyhow::{Context, Result};
use clap::Parser;
use company_application::{App, AppConfig};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Start TRANSPLANE-COMPANY and open its game interface"
)]
struct Args {
    /// Defaults to company-data beside this executable, independent of the working directory.
    #[arg(long, env = "COMPANY_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, env = "COMPANY_DATABASE_URL", hide_env_values = true)]
    database_url: Option<String>,
    #[arg(long, env = "COMPANY_BIND", default_value = "127.0.0.1:4177")]
    bind: SocketAddr,
    #[arg(long, env = "COMPANY_WEB_DIR")]
    web_dir: Option<PathBuf>,
    #[arg(long, default_value = "en", value_parser = ["en", "zh-CN"])]
    lang: String,
    /// Keep the server in this process without opening a browser or desktop window.
    #[arg(long)]
    no_browser: bool,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    // Parse errors go to stdout/stderr for Agents instead of creating a desktop window.
    let args = Args::parse();
    match launch(&args).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let message = format!("{error:#}");
            eprintln!("{message}");
            if !args.no_browser {
                desktop::show_error(&message, &args.lang);
            }
            std::process::ExitCode::FAILURE
        }
    }
}

async fn launch(args: &Args) -> Result<()> {
    let executable = std::env::current_exe().context("Cannot locate this executable")?;
    let data_dir = instance::data_directory(&executable, args.data_dir.as_deref())?;
    std::fs::create_dir_all(data_dir.join("config")).with_context(|| {
        format!(
            "Cannot write {}. Use --data-dir with a writable folder.",
            data_dir.display()
        )
    })?;
    let data_dir = std::fs::canonicalize(data_dir)?;
    let mut instance = match instance::Instance::acquire(&data_dir)? {
        instance::Acquisition::Existing(existing) => {
            println!(
                "{}",
                json!({"status":"already_running","url":existing.url,"data_dir":data_dir})
            );
            if !args.no_browser {
                desktop::open_browser(&existing.url)?;
            }
            return Ok(());
        }
        instance::Acquisition::Acquired(instance) => instance,
    };

    // Claim the requested port before touching credentials or opening any browser.
    // An unrelated service on that port must never be treated as this deployment.
    let listener = tokio::net::TcpListener::bind(args.bind).await.with_context(|| {
        format!("Cannot start on {}. Another service may be using this address; choose a different --bind port.", args.bind)
    })?;
    let address = listener.local_addr()?;
    let url = instance::browser_url(address);
    let app = App::open(AppConfig {
        data_dir: data_dir.clone(),
        database_url: args.database_url.clone(),
    })
    .await
    .context("Cannot open company data")?;
    let owner_token = app
        .ensure_owner_token()
        .await
        .context("Cannot initialize owner access")?;
    let marker = instance.marker().to_owned();
    let probe_path = format!("/_company-launcher/{marker}");
    let service = company_server::router(app, args.web_dir.clone()).route(
        &probe_path,
        axum::routing::get(move || {
            let marker = marker.clone();
            async move { marker }
        }),
    );
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, service)
            .with_graceful_shutdown(async {
                let _ = stop_receiver.await;
            })
            .await
    });
    instance.publish(&url)?;

    if args.no_browser || !cfg!(windows) {
        let mut ready = json!({"status":"running","url":url,"data_dir":data_dir});
        if let Some(token) = owner_token.as_ref() {
            ready["owner_token"] = token.clone().into();
        }
        println!("{ready}");
    }
    if args.no_browser || !cfg!(windows) {
        if !args.no_browser {
            desktop::open_browser(&url)?;
        }
        tokio::select! {
            _ = stop_signal() => {},
            result = &mut server => { result.context("Server task failed")??; return Ok(()); }
        }
        let _ = stop_sender.send(());
        server.await.context("Server task failed")??;
        return Ok(());
    }

    let session = desktop::Session {
        url,
        data_dir,
        owner_token,
        lang: args.lang.clone(),
    };
    let mut desktop = tokio::task::spawn_blocking(move || desktop::run(session));
    tokio::select! {
        result = &mut desktop => {
            let _ = stop_sender.send(());
            server.await.context("Server task failed")??;
            result.context("Desktop task failed")??;
        },
        result = &mut server => {
            desktop::close_window();
            desktop.await.context("Desktop task failed")??;
            result.context("Server task failed")??;
        }
    }
    Ok(())
}

async fn stop_signal() {
    #[cfg(unix)]
    if let Ok(mut signal) =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = signal.recv() => {} }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}
