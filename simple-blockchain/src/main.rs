use simple_blockchain::{cli, errors::Result};

fn main() -> Result<()> {
    let mut cli = cli::Cli::new().unwrap();
    cli.run()
}
