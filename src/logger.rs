pub fn setup_logger(quiet: bool, log_file_debug: bool) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(if log_file_debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .chain(
            fern::Dispatch::new()
                .level(if quiet {
                    log::LevelFilter::Off
                } else {
                    log::LevelFilter::Info
                })
                .chain(std::io::stdout()),
        )
        .chain(
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open("cache/output.log")?,
        )
        .apply()?;
    Ok(())
}
