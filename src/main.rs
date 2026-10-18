use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use claude_termux::{
    cli::{Cli, Command},
    doctor, installation, launcher, setup,
};

#[tokio::main]
async fn main() {
    if let Err(error) = execute().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn execute() -> Result<()> {
    let cli = Cli::parse();
    let root = installation::resolve_root(cli.root)?;
    match cli.command {
        Command::Doctor {
            network,
            json,
            patchelf,
        } => {
            let report = doctor::examine(
                &root,
                &doctor::Options {
                    network,
                    patchelf,
                    ..Default::default()
                },
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print();
            }
            anyhow::ensure!(
                report.is_ready(),
                "Doctor found failed checks; see the report above."
            );
        }
        Command::Setup {
            version,
            dry_run,
            patchelf,
        } => {
            setup::run(&root, version, patchelf, dry_run).await?;
        }
        Command::Install { version, patchelf } => {
            installation::install(&root, &version, &patchelf).await?
        }
        Command::Inspect => println!(
            "{}",
            serde_json::to_string_pretty(&installation::verify_active(&root)?)?
        ),
        Command::Run { allow_hosts, args } => {
            let status = launcher::run(&root, allow_hosts, args).await?;
            std::process::exit(status);
        }
        Command::Rollback => installation::rollback(&root)?,
        Command::Completions { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "claude-termux",
                &mut std::io::stdout(),
            );
        }
        Command::Uninstall => installation::uninstall(&root)?,
    }
    Ok(())
}
