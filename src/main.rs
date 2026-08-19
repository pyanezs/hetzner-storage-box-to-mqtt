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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = parse_args();
    let path = config::resolve_config_path(args.config_path);

    let cfg = match config::load_from_path(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("{e:#}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(id) = args.dump_raw_id {
        let client = hetzner::HetznerClient::new(
            cfg.hetzner.api_base_url.clone(),
            cfg.hetzner.api_token.clone(),
        );
        return match client.fetch_raw(id).await {
            Ok(raw) => {
                println!("{raw}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                tracing::error!("{e}");
                ExitCode::FAILURE
            }
        };
    }

    match run(&cfg).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}
