use clap::{App, Arg, ArgMatches};

pub const CONFIG: &str = "config";

pub fn matches() -> ArgMatches<'static> {
    let matches = App::new("Eve Tradeworks")
        .arg(
            Arg::with_name(CONFIG)
                .short("c")
                .long("config")
                .takes_value(true),
        )
        .get_matches();
    matches
}
