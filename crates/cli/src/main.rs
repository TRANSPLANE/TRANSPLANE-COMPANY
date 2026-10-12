use anyhow::Context;
use clap::{Parser, Subcommand};
use company_application::{App, AppConfig, BatchOperation, OperationRequest, Principal};
use company_core::{DomainError, Locale};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "company",
    version,
    about = "TransPlane Company local maintenance interface"
)]
struct Cli {
    #[arg(
        long,
        env = "COMPANY_DATA_DIR",
        default_value = "company-data",
        global = true
    )]
    data_dir: PathBuf,
    #[arg(
        long,
        env = "COMPANY_DATABASE_URL",
        global = true,
        hide_env_values = true
    )]
    database_url: Option<String>,
    #[arg(long,env="COMPANY_LANG",default_value="en",global=true,value_parser=["en","zh-CN"])]
    lang: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a deployment and display the new owner token once
    Init,
    /// Start the company console
    Serve {
        #[arg(long, env = "COMPANY_BIND", default_value = "127.0.0.1:4177")]
        bind: SocketAddr,
        #[arg(long, env = "COMPANY_WEB_DIR")]
        web_dir: Option<PathBuf>,
    },
    /// Discover services, operations and exact contracts
    Discover {
        #[arg(long)]
        view: Option<String>,
        #[arg(long)]
        service: Option<String>,
        #[arg(long)]
        operation: Option<String>,
    },
    /// Read guidance for the current maintenance topic
    Guide {
        #[arg(long, default_value = "connect")]
        topic: String,
    },
    /// List companies in this deployment
    Companies,
    /// Read the current event, exact choices, saved intent, state and page links
    Play {
        #[arg(long)]
        company: String,
    },
    /// Read company relationships and attributed revenue and costs
    Network {
        #[arg(long)]
        company: String,
        #[arg(long)]
        period_start: Option<String>,
        #[arg(long)]
        period_end: Option<String>,
        #[arg(long,value_parser=["cash","recognized"])]
        basis: Option<String>,
    },
    /// Read quests, warnings, level and experience
    Game {
        #[arg(long)]
        company: String,
    },
    /// Discover hosted operating Skills; no local skill installation required
    Skills {
        #[arg(long)]
        company: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, requires = "id")]
        revision: Option<i64>,
        #[arg(long, requires="id", requires="company", conflicts_with_all=["revision","document"])]
        history: bool,
        #[arg(long, requires = "id")]
        document: bool,
        #[arg(long, requires = "company")]
        include_deleted: bool,
    },
    /// Read registered document preview metadata at an exact content revision
    Preview {
        #[arg(long)]
        company: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        revision: Option<i64>,
    },
    /// Read a company snapshot, records or historical revision
    Query {
        #[arg(long)]
        company: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        history: bool,
        #[arg(long)]
        records: bool,
        #[arg(long, default_value = "0")]
        offset: i64,
        #[arg(long, conflicts_with_all=["kind","id","history","records","offset"])]
        revision: Option<i64>,
    },
    /// Calculate company metrics for the selected date
    Metrics {
        #[arg(long)]
        company: String,
        #[arg(long)]
        as_of: Option<String>,
    },
    /// Submit an operation; the JSON file contains its input value
    Apply {
        #[arg(long)]
        company: Option<String>,
        #[arg(long)]
        operation: String,
        #[arg(long, help = "Input JSON file; use - to read standard input")]
        input: PathBuf,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        expected_revision: Option<i64>,
    },
    /// Validate and atomically submit a batch of operations
    Import {
        #[arg(long)]
        company: String,
        #[arg(long)]
        file: PathBuf,
    },
    /// Register a company file and preserve its content revision
    Sync {
        #[arg(long)]
        company: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        request_id: String,
    },
    /// Export a snapshot that can be restored on either database backend
    Export {
        #[arg(long)]
        company: Option<String>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Create a complete restorable backup
    Backup {
        #[arg(long)]
        company: Option<String>,
        #[arg(long)]
        destination: Option<PathBuf>,
    },
    /// Restore an export or backup into an empty deployment
    Restore {
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// Manage revocable network credentials
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Manage fixed plugin versions and retained results
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
enum TokenAction {
    List,
    Grant {
        #[arg(long)]
        company: Option<String>,
        #[arg(long = "scope", default_value = "company.read")]
        scopes: Vec<String>,
        #[arg(long)]
        owner: bool,
    },
    Revoke {
        id: String,
    },
}
#[derive(Subcommand)]
enum PluginAction {
    List,
    Catalog,
    Install {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    Uninstall {
        id: String,
        #[arg(long)]
        purge: bool,
    },
    DeliverBackup {
        #[arg(long)]
        plugin: String,
        #[arg(long)]
        snapshot: PathBuf,
    },
    Request {
        id: String,
        #[arg(long)]
        company: String,
        #[arg(long)]
        request_id: String,
    },
    DataExit {
        id: String,
        #[arg(long)]
        company: String,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        sha256: String,
    },
    Reconcile {
        id: String,
        #[arg(long)]
        company: String,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        result: PathBuf,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ImportDocument {
    Envelope { operations: Vec<BatchOperation> },
    Array(Vec<BatchOperation>),
}
impl ImportDocument {
    fn into_operations(self) -> Vec<BatchOperation> {
        match self {
            Self::Envelope { operations } => operations,
            Self::Array(operations) => operations,
        }
    }
}
fn read_json(path: &Path) -> anyhow::Result<Value> {
    let mut bytes = Vec::new();
    if path == Path::new("-") {
        std::io::stdin()
            .take(24 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
    } else {
        let file =
            std::fs::File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
        file.take(24 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    }
    if bytes.len() > 24 * 1024 * 1024 {
        anyhow::bail!("输入 JSON 超过 24 MiB。")
    }
    serde_json::from_slice(&bytes).context("输入文件必须是有效 JSON。")
}
fn local_actor() -> String {
    format!(
        "local:{}",
        std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "owner".into())
    )
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let locale = selected_locale(&cli.lang);
    let raw_document = matches!(&cli.command, Command::Skills { document: true, .. });
    match run(cli).await {
        Ok(value) if raw_document => println!("{}", value.as_str().unwrap_or("")),
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
        ),
        Err(error) => {
            let value = if let Some(domain) = error.downcast_ref::<company_core::DomainError>() {
                json!({"error":{"code":domain.code,"message":company_core::localize_error(domain,locale)}})
            } else {
                let domain = DomainError::new("operation_failed", error.to_string());
                json!({"error":{"code":"operation_failed","message":company_core::localize_error(&domain,locale)}})
            };
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
            );
            std::process::exit(1);
        }
    }
}
fn selected_locale(lang: &str) -> Locale {
    if lang == "zh-CN" {
        Locale::ZhCn
    } else {
        Locale::En
    }
}

async fn run(cli: Cli) -> anyhow::Result<Value> {
    let locale = selected_locale(&cli.lang);
    let app = App::open(AppConfig {
        data_dir: cli.data_dir,
        database_url: cli.database_url,
    })
    .await?;
    let actor = local_actor();
    match cli.command {
        Command::Init => {
            let token = app.ensure_owner_token().await?;
            Ok(match token {
                Some(token) => {
                    json!({"status":"initialized","owner_token":token,"message":match locale{Locale::ZhCn=>"保存此凭据以登录经营台；此凭据只显示一次。",Locale::En=>"Save this token to sign in to the company console. It is shown only once."}})
                }
                None => json!({"status":"initialized","owner_token_created":false}),
            })
        }
        Command::Serve { bind, web_dir } => {
            company_server::serve(app, bind, web_dir).await?;
            Ok(json!({"status":"stopped"}))
        }
        Command::Companies => {
            app.list_companies(&Principal {
                kind: "owner".into(),
                subject: actor,
                company_id: None,
                scopes: vec![],
            })
            .await
        }
        Command::Query {
            company,
            kind,
            id,
            history,
            records,
            offset,
            revision,
        } => {
            if let Some(revision) = revision {
                return app.snapshot_revision(&company, revision).await;
            }
            if records || kind.is_some() || id.is_some() || history || offset != 0 {
                app.records_filtered_page(&company, kind.as_deref(), id.as_deref(), history, offset)
                    .await
            } else {
                app.snapshot_localized(&company, locale).await
            }
        }
        Command::Play { company } => app.play_state_localized(&company, locale).await,
        Command::Network {
            company,
            period_start,
            period_end,
            basis,
        } => {
            app.network(
                &company,
                period_start.as_deref(),
                period_end.as_deref(),
                basis.as_deref(),
            )
            .await
        }
        Command::Game { company } => app.game_state_localized(&company, locale).await,
        Command::Skills {
            company,
            id,
            revision,
            history,
            document,
            include_deleted,
        } => {
            let value = match (company.as_deref(), id.as_deref()) {
                (Some(company), Some(id)) if history => app.skill_history(company, id).await?,
                (Some(company), Some(id)) => {
                    app.company_skill(company, id, revision, locale).await?
                }
                (Some(company), None) => {
                    app.company_skills(company, include_deleted, locale).await?
                }
                (None, Some(id)) => {
                    if revision.is_some_and(|r| r != 1) {
                        anyhow::bail!(DomainError::new("not_found", "Skill 版本不存在"));
                    }
                    app.builtin_skill(id, locale).await?
                }
                (None, None) => app.builtin_skills(locale).await?,
            };
            Ok(if document {
                value["document"].clone()
            } else {
                value
            })
        }
        Command::Preview {
            company,
            file,
            revision,
        } => app.file_preview(&company, &file, revision).await,
        Command::Metrics { company, as_of } => app.metrics(&company, as_of.as_deref()).await,
        Command::Apply {
            company,
            operation,
            input,
            request_id,
            expected_revision,
        } => {
            app.execute(
                company.as_deref(),
                &operation,
                OperationRequest {
                    request_id,
                    expected_revision,
                    input: read_json(&input)?,
                },
                &actor,
            )
            .await
        }
        Command::Import { company, file } => {
            let operations = serde_json::from_value::<ImportDocument>(read_json(&file)?)
                .context("导入格式应为 {\"operations\":[{\"operation\":...,\"request\":...}]}。")?
                .into_operations();
            if operations.is_empty() {
                anyhow::bail!("导入至少需要一个操作。")
            }
            if operations.len() > 1000 {
                anyhow::bail!("一次导入最多 1,000 个操作。")
            }
            app.import_batch(&company, operations, &actor).await
        }
        Command::Sync {
            company,
            path,
            request_id,
        } => app.sync_file_as(&company, &path, &request_id, &actor).await,
        Command::Export { company, output } => app.backup(company.as_deref(), Some(&output)).await,
        Command::Backup {
            company,
            destination,
        } => app.backup(company.as_deref(), destination.as_deref()).await,
        Command::Restore { snapshot } => app.restore(&snapshot).await,
        Command::Token { action } => match action {
            TokenAction::List => app.list_tokens().await,
            TokenAction::Grant {
                company,
                scopes,
                owner,
            } => app.grant_token(company.as_deref(), scopes, owner).await,
            TokenAction::Revoke { id } => app.revoke_token(&id).await,
        },
        Command::Plugin { action } => match action {
            PluginAction::List => app.plugin_catalog_localized(locale).await,
            PluginAction::Catalog => app.plugin_catalog_localized(locale).await,
            PluginAction::Install { manifest, config } => {
                app.install_plugin(
                    read_json(&manifest)?,
                    match config {
                        Some(path) => read_json(&path)?,
                        None => json!({}),
                    },
                )
                .await
            }
            PluginAction::Enable { id } => app.set_plugin_enabled(&id, true).await,
            PluginAction::Disable { id } => app.set_plugin_enabled(&id, false).await,
            PluginAction::Uninstall { id, purge } => app.uninstall_plugin(&id, purge).await,
            PluginAction::DeliverBackup { plugin, snapshot } => {
                app.deliver_backup(&plugin, &snapshot).await
            }
            PluginAction::Request {
                id,
                company,
                request_id,
            } => app.plugin_request(&company, &id, &request_id).await,
            PluginAction::DataExit {
                id,
                company,
                snapshot,
                sha256,
            } => {
                app.remove_plugin_data(&company, &id, &snapshot, &sha256)
                    .await
            }
            PluginAction::Reconcile {
                id,
                company,
                request_id,
                result,
            } => {
                app.reconcile_plugin(&company, &id, &request_id, read_json(&result)?)
                    .await
            }
        },
        Command::Discover {
            view,
            service,
            operation,
        } => {
            app.discovery_localized(
                view.as_deref(),
                service.as_deref(),
                operation.as_deref(),
                locale,
            )
            .await
        }
        Command::Guide { topic } => app.agent_guide_localized(&topic, locale).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_shape_requires_write_request_identity() {
        assert!(Cli::try_parse_from([
            "company",
            "apply",
            "--operation",
            "cash.record",
            "--input",
            "input.json"
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "company",
            "apply",
            "--operation",
            "cash.record",
            "--input",
            "input.json",
            "--request-id",
            "stable-id"
        ])
        .is_ok());
    }
    #[test]
    fn import_accepts_envelope_and_array() {
        for value in [json!([]), json!({"operations":[]})] {
            assert!(serde_json::from_value::<ImportDocument>(value)
                .unwrap()
                .into_operations()
                .is_empty());
        }
    }
    #[test]
    fn language_is_a_global_option_and_only_supported_locales_are_accepted() {
        assert_eq!(
            Cli::try_parse_from(["company", "--lang", "en", "discover"])
                .unwrap()
                .lang,
            "en"
        );
        assert_eq!(
            Cli::try_parse_from(["company", "guide", "--lang", "zh-CN"])
                .unwrap()
                .lang,
            "zh-CN"
        );
        assert!(Cli::try_parse_from(["company", "--lang", "unknown", "discover"]).is_err());
    }
}
