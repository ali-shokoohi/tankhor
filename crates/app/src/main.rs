//! Tankhor CLI composition entry point (`openapi` offline export and `serve` HTTP runtime).

use std::io;
use std::process::ExitCode;

use app::config::AppConfig;
use app::openapi::write_openapi_json;
use app::provision_admin::provision_from_env;
use app::serve::run_from_env;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("usage: app <openapi|serve|provision-admin>");
        return ExitCode::from(2);
    };

    if !AppConfig::command_requires_runtime_config(&command) {
        if args.next().is_some() {
            eprintln!("unexpected arguments after `openapi`");
            return ExitCode::from(2);
        }
        return match write_openapi_json(io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        };
    }

    if command == "provision-admin" {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("failed to initialize async runtime: {err}");
                return ExitCode::FAILURE;
            }
        };
        return match runtime.block_on(provision_from_env(args.collect::<Vec<_>>())) {
            Ok(_) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        };
    }

    if command == "serve" {
        if args.next().is_some() {
            eprintln!("unexpected arguments after `serve`");
            return ExitCode::from(2);
        }
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("failed to initialize async runtime: {err}");
                return ExitCode::FAILURE;
            }
        };
        return match runtime.block_on(run_from_env()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        };
    }

    eprintln!("unsupported command `{command}`");
    ExitCode::from(2)
}
