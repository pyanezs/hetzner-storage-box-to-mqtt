use chrono::Local;
use hetzner_storage_box_to_mqtt::{config, run, schedule};
use std::process::ExitCode;
use std::time::Duration;

struct Args {
    config_path: Option<String>,
}

fn parse_args() -> Args {
    let mut config_path = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--config" {
            config_path = args.next();
        }
    }

    Args { config_path }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = parse_args();
    let path = config::resolve_config_path(args.config_path);
    let cfg_result = config::load_from_path(&path);

    // `RUST_LOG` always wins when set; otherwise fall back to `general.log_level`
    // from the config file, or the tracing_subscriber default (ERROR only) if the
    // config itself failed to load.
    let filter = match &cfg_result {
        Ok(cfg) if std::env::var("RUST_LOG").is_err() => {
            tracing_subscriber::EnvFilter::new(&cfg.general.log_level)
        }
        _ => tracing_subscriber::EnvFilter::from_default_env(),
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = match cfg_result {
        Ok(cfg) => {
            cfg.log();
            cfg
        }
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "Failed to load config");
            return ExitCode::FAILURE;
        }
    };

    let run_times = config.general.parsed_run_times();

    // Run one cycle immediately on startup, then follow the configured schedule.
    if let Err(e) = run(&config).await {
        tracing::error!(error = %format!("{e:#}"), "Run failed");
    }

    if run_times.is_empty() {
        return ExitCode::SUCCESS;
    }

    loop {
        let next_run = schedule::next_run_after(Local::now(), &run_times);
        tracing::info!(
            next_run_at = %next_run.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "Next cycle will start"
        );
        let sleep_duration = (next_run - Local::now()).to_std().unwrap_or(Duration::ZERO);

        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {
                if let Err(e) = run(&config).await {
                    tracing::error!(error = %format!("{e:#}"), "Run failed");
                }
            }
            _ = shutdown_signal() => {
                tracing::info!("Shutdown signal received, exiting");
                break;
            }
        }
    }

    ExitCode::SUCCESS
}

/// Resolves when either SIGINT or SIGTERM is received.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            return;
        };
        sig.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
