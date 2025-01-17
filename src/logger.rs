pub fn setup_logger(quiet: bool, file: bool) -> Result<(), fern::InitError> {
    let mut chain = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(
            fern::Dispatch::new()
                .level(if quiet {
                    log::LevelFilter::Off
                } else {
                    log::LevelFilter::Info
                })
                .chain(std::io::stdout()),
        );
    if file {
        chain = chain.chain(
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open("cache/output.log")?,
        );
    }
    chain.apply()?;
    Ok(())
}
