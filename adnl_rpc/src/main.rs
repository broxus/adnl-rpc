use std::path::PathBuf;

use anyhow::Result;
use clap::{Clap, IntoApp};

use adnl_rpc::Config;

#[derive(Clone, Debug, Clap)]
pub struct Arguments {
    /// Path to config
    #[clap(short, long, conflicts_with = "gen-config")]
    pub config: Option<PathBuf>,

    /// Generate default config
    #[clap(long)]
    pub gen_config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Arguments = Arguments::parse();

    match (args.config, args.gen_config) {
        (_, Some(new_config_path)) => generate_config(new_config_path)?,
        (Some(config), None) => {
            let config = read_config(config)?;
            init_logger(&config.logger_settings)?;

            adnl_rpc::serve(config).await?
        }
        _ => Arguments::into_app().print_help()?,
    }

    Ok(())
}

pub fn generate_config<T>(path: T) -> Result<()>
where
    T: AsRef<std::path::Path>,
{
    use std::io::Write;

    let mut file = std::fs::File::create(path)?;
    let config = Config::default();
    file.write_all(serde_yaml::to_string(&config)?.as_bytes())?;
    Ok(())
}

pub fn read_config(path: PathBuf) -> Result<Config> {
    let mut config = config::Config::new();
    config.merge(config::File::from(path).format(config::FileFormat::Yaml))?;
    config.merge(config::Environment::new())?;

    let config: Config = config.try_into()?;
    Ok(config)
}

fn init_logger(config: &serde_yaml::Value) -> Result<()> {
    let config = serde_yaml::from_value(config.clone())?;
    log4rs::config::init_raw_config(config)?;
    Ok(())
}
