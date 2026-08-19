use hetzner_storage_box_to_mqtt::{config, hetzner, run};
use std::process::ExitCode;

struct Args {
    config_path: Option<String>,
    dump_raw_id: Option<u64>,
}

fn parse_args() -> Args {
    let mut config_path = None;
    let mut dump_raw_id = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config_path = args.next(),
            "--dump-raw" => {
                dump_raw_id = args.next().and_then(|s| s.parse().ok());
            }
            _ => {}
        }
    }

    Args {
        config_path,
        dump_raw_id,
    }
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

    let cfg = match cfg_result {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "Failed to load config");
            return ExitCode::FAILURE;
        }
    };
    cfg.log();

    if let Some(id) = args.dump_raw_id {
        let client = hetzner::HetznerClient::new(
            cfg.hetzner.api_base_url.clone(),
            cfg.hetzner.api_token.clone(),
        );
        return match client.fetch_raw(id).await {
            Ok(raw) => {
                tracing::debug!(box_id = id, raw = %raw, "Fetched raw storage box data");
                println!("{raw}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                tracing::error!(box_id = id, error = %e, "Failed to fetch raw storage box data");
                ExitCode::FAILURE
            }
        };
    }

    match run(&cfg).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %format!("{e:#}"), "Run failed");
            ExitCode::FAILURE
        }
    }
}
