pub fn setup_logger() -> Result<(), fern::InitError> {
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
        .level(log::LevelFilter::Debug)
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Warn)
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
