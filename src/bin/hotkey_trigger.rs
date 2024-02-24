use interprocess::os::unix::udsocket::UdStream;
use unusable_eve_tradeworks_lib::{consts::UD_SOCKET_PATH, logger};

#[tokio::main]
async fn main() {
    let result = run().await;
    if let Err(ref err) = result {
        log::error!("ERROR: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));
        std::process::exit(1)
    }
}
async fn run() -> Result<(), anyhow::Error> {
    logger::setup_logger(false, false)?;

    let conn = UdStream::connect(UD_SOCKET_PATH)?;

    log::info!("Socket created");

    conn.send(&[1])?;

    log::info!("Data sent");

    Ok(())
}
