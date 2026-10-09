use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        default_value = "config.tomr",
        help = "Configuration file paht."
    )]
    pub config_file: String,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Load data from configured sources."
    )]
    pub load: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Generate and send report to configured recipients."
    )]
    pub report: bool,
}
