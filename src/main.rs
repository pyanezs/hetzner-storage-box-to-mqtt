use hetzner_storage_box_to_mqtt::{config, run};
use std::process::ExitCode;

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

    match run(&config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "Run failed");
            ExitCode::FAILURE
        }
    }
}
