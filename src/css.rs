use std::path::Path;
use tokio::{fs::File, io::AsyncWriteExt};

/// uses the zephyr inventory to generate css and store it in `path`
pub async fn generate_css_from_inventory(path: impl AsRef<Path>) -> std::io::Result<()> {
    let z = maud::zephyr::Zephyr::new();
    let generated_css = z.generate_from_inventory();

    let mut file = File::create(path).await?;
    file.write_all(generated_css.as_bytes()).await?;

    Ok(())
}
