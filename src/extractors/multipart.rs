use std::{collections::HashMap, io, path::Path};

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
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

impl<F, S> FromRequest<S> for Multipart<F>
where
    F: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ErrResponse;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let config = req
            .extensions()
            .get::<Config>()
            .expect("Config Extension should be added")
            .clone();

        let mut f = axum::extract::multipart::Multipart::from_request(req, state).await?;

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
            // a part without a name can't be matched against the struct, so skip it
            let Some(name) = field.name() else {
                continue;
            };
            let name = name.to_string();
            let (name, is_vec) = if let Some(name) = name.strip_suffix("[]") {
                (name.to_string(), true)
            } else {
                (name, false)
            };

            if !allowed_fields.contains(&name.as_str()) {
                continue;
            }

            let new = if let Some(file_name) = field.file_name() {
                // the name is client supplied, so it has to be reduced to a single
                // component before it is joined onto the upload directory
                let Some(original_name) = sanitize_file_name(file_name) else {
                    continue;
                };

                // the field is file
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                let mut upload_path = config.get_random_folder()?;
                upload_path.push(&original_name);
                stream_to_file(&upload_path, field).await?;

                FieldInner::UploadedFile(UploadedFile {
                    content_type,
                    filename: original_name,
                    upload_path: upload_path
                        .strip_prefix(config.get_upload_path())?
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

/// reduces a client supplied file name to a single path component, so that it can
/// never escape the directory it gets joined onto. returns `None` when nothing usable
/// is left, eg for `""`, `".."` or `"foo/"`
pub fn sanitize_file_name(file_name: &str) -> Option<String> {
    // windows clients send backslash separated paths, which unix treats as a normal char
    let last = file_name.rsplit(['/', '\\']).next()?;

    if last.is_empty() || last == "." || last == ".." {
        return None;
    }

    Some(last.to_string())
}

// Save a `Stream` to a file
async fn stream_to_file<S, E>(path: &Path, stream: S) -> Result<(), io::Error>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(io::Error::other);
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    let mut file = BufWriter::new(File::create(path).await?);

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sanitize_file_name;

    #[test]
    fn test_sanitize_file_name() {
        assert_eq!(
            sanitize_file_name("hello.png").as_deref(),
            Some("hello.png")
        );
        assert_eq!(
            sanitize_file_name("../../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(sanitize_file_name("/etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(
            sanitize_file_name(r"C:\windows\system32\evil.dll").as_deref(),
            Some("evil.dll")
        );
        assert_eq!(sanitize_file_name(".."), None);
        assert_eq!(sanitize_file_name("a/b/"), None);
        assert_eq!(sanitize_file_name(""), None);
    }
}
