use std::{collections::HashMap, io, path::Path};

use axum::{
    async_trait,
    body::{Bytes, HttpBody},
    extract::{FromRequest, RequestParts},
    BoxError,
};
use futures::{Stream, TryStreamExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;

use crate::{config::Config, errors::*, helpers::struct_fields};

/// uploaded file struct in multipart
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadedFile {
    pub content_type: String,
    pub upload_path: String,
    pub filename: String,
}

pub struct Multipart<F>(pub F);

#[async_trait]
impl<F, B> FromRequest<B> for Multipart<F>
where
    F: DeserializeOwned,
    B: HttpBody<Data = Bytes> + Default + Unpin + Send + 'static,
    B::Error: Into<BoxError>,
{
    type Rejection = ErrResponse;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let config = req
            .extensions()
            .get::<Config>()
            .expect("Config Extension should be added")
            .clone();

        let mut f = axum::extract::multipart::Multipart::from_request(req).await?;

        #[derive(Debug, Serialize)]
        #[serde(untagged)]
        enum FieldInner {
            UploadedFile(UploadedFile),
            Text(String),
        }

        #[derive(Debug, Serialize)]
        #[serde(untagged)]
        enum Field {
            Single(FieldInner),
            Array(Vec<FieldInner>),
        }

        let allowed_fields = struct_fields::<F>();

        let mut form: HashMap<String, Field> = HashMap::new();

        while let Some(field) = f.next_field().await? {
            let name = field.name().unwrap().to_string();
            let (name, is_vec) = if let Some(name) = name.strip_suffix("[]") {
                (name.to_string(), true)
            } else {
                (name, false)
            };

            if !allowed_fields.contains(&name.as_str()) {
                continue;
            }

            let new = if let Some(file_name) = field.file_name() {
                if file_name.is_empty() {
                    continue;
                }

                // the field is file
                let original_name: String = file_name.to_string();
                let content_type = field.content_type().unwrap().to_string();

                let mut upload_path = config.get_random_folder()?;
                upload_path.push(original_name.clone());
                stream_to_file(&upload_path, field).await?;

                FieldInner::UploadedFile(UploadedFile {
                    content_type,
                    filename: original_name,
                    upload_path: upload_path
                        .strip_prefix(&config.get_upload_path())?
                        .display()
                        .to_string(),
                })
            } else {
                // the field is text
                FieldInner::Text(field.text().await?)
            };

            // if there was already a file, it's definitely a vec
            if let Some(f) = form.remove(&name) {
                let v = match f {
                    Field::Single(a) => vec![a, new],
                    Field::Array(mut vec) => {
                        vec.push(new);
                        vec
                    }
                };
                form.insert(name, Field::Array(v));
            } else {
                let v = if is_vec {
                    Field::Array(vec![new])
                } else {
                    Field::Single(new)
                };
                form.insert(name, v);
            }
        }

        let ser = serde_json::to_string(&form)?;
        drop(form);
        tracing::debug!("{}", ser);
        let form: F = serde_json::from_str(&ser)?;
        drop(ser);

        Ok(Multipart::<F>(form))
    }
}

// Save a `Stream` to a file
async fn stream_to_file<S, E>(path: &Path, stream: S) -> Result<(), io::Error>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    let mut file = BufWriter::new(File::create(path).await?);

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await?;

    Ok(())
}
