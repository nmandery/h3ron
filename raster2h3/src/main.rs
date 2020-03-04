
mod lib;

use crate::lib::{TopLevelArguments, convert_to_ogr, Subcommands, convert_to_sqlite};

fn main() {
    simple_logger::init().unwrap();
    let args: TopLevelArguments = argh::from_env();

    let result = match &args.subcommand {
        Subcommands::ToOgr(to_ogr_args) => {
            convert_to_ogr(&args, to_ogr_args)
        },
        Subcommands::ToSqlite(to_sqlite_args) => {
           convert_to_sqlite(&args, to_sqlite_args)
        }
    };

    if let Err(msg) = result {
        eprintln!("error: {}", msg);
        std::process::exit(1);
    }
}
