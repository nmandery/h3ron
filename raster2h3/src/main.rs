// using jemalloc reduces the processing time by a small amount
// maybe because the number of minor page faults gets reduced to ~50%
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

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
