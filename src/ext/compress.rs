use crate::ext::anyhow::{Context, Result};
use brotli::enc::BrotliEncoderParams;
use libflate::gzip;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use tokio::time::Instant;

pub async fn compress_static_files(path: PathBuf) -> Result<()> {
    let start = Instant::now();

    tokio::task::spawn_blocking(move || compress_dir_all(path)).await??;

    log::info!(
        "Precompression of static files finished after {} ms",
        start.elapsed().as_millis()
    );
    Ok(())
}

// This is sync / blocking because an async / parallel execution did provide only a small benefit
// in performance (~4%) while needing quite a few more dependencies and much more verbose code.
fn compress_dir_all(path: PathBuf) -> Result<()> {
    log::trace!("FS compress_dir_all {:?}", path);

    let dir = fs::read_dir(&path).context(format!("Could not read {:?}", path))?;
    let brotli_params = BrotliEncoderParams::default();

    for entry in dir.into_iter() {
        let path = entry?.path();
        let metadata = fs::metadata(&path)?;

        if metadata.is_dir() {
            compress_dir_all(path)?;
        } else {
            let pstr = path.to_str().unwrap_or_default();
            if pstr.ends_with(".gz") || pstr.ends_with(".br") {
                // skip all files that are already compressed
                continue;
            }

            let file = fs::read(&path)?;

            // gzip
            let mut encoder = gzip::Encoder::new(Vec::new())?;
            encoder.write_all(file.as_ref())?;
            let encoded_data = encoder.finish().into_result()?;
            let path_gz = format!("{}.gz", pstr.unwrap());
            fs::write(path_gz, encoded_data)?;

            // brotli
            let path_br = format!("{}.br", pstr.unwrap());
            let mut output = File::create(path_br)?;
            let mut reader = BufReader::new(file.as_slice());
            brotli::BrotliCompress(&mut reader, &mut output, &brotli_params).unwrap();
        }
    }

    Ok(())
}
