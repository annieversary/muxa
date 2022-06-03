use std::path::PathBuf;

use crate::{
    config::Config, errors::ErrResponse, extractors::multipart::UploadedFile,
    helpers::copy_extension,
};
use image::{
    imageops::{resize, FilterType},
    io::Reader as ImageReader,
};

/// assumes the file is an image
/// compresses it, and returns the path to the compressed image
pub fn resize_and_compress_image(
    file: &UploadedFile,
    config: &Config,
    nwidth: u32,
    nheight: u32,
) -> Result<PathBuf, ErrResponse> {
    let mut path = config.get_upload_path().clone();
    path.push(&file.upload_path);

    // process
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let resized = resize(&img, nwidth, nheight, FilterType::Gaussian);
    let compressed = turbojpeg::compress_image(&resized, 90, turbojpeg::Subsamp::Sub2x2)?;

    let filename = copy_extension("image.jpg", &file.filename);
    let mut upload_path = config.get_random_folder()?;
    upload_path.push(filename);

    std::fs::write(&upload_path, &compressed)?;

    Ok(upload_path)
}
