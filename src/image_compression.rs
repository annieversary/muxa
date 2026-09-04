use std::path::PathBuf;

use axum::http::StatusCode;

use crate::{
    config::Config,
    errors::ErrResponse,
    extractors::multipart::{sanitize_file_name, UploadedFile},
    helpers::copy_extension,
};
use image::{
    codecs::jpeg::JpegEncoder,
    imageops::{resize, FilterType},
    ImageReader,
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
    let mut compressed = Vec::new();
    JpegEncoder::new_with_quality(&mut compressed, 90).encode_image(&resized)?;

    // `filename` came off the wire, so it gets reduced to a single component before
    // it is joined onto the new folder
    let filename = sanitize_file_name(&file.filename)
        .ok_or_else(|| ErrResponse::new(StatusCode::BAD_REQUEST, "unusable file name"))?;
    let filename = copy_extension("image.jpg", &filename);
    let mut upload_path = config.get_random_folder()?;
    upload_path.push(filename);

    std::fs::write(&upload_path, &compressed)?;

    Ok(upload_path)
}
