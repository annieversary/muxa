use validator::ValidationError;

pub fn alpha_dash(s: &str) -> Result<(), ValidationError> {
    if !s
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ValidationError::new("not alpha-dash"));
    }

    Ok(())
}

/// validates that `p` is either `"public"` or `"private"`
pub fn public_private(p: &str) -> Result<(), ValidationError> {
    if p != "public" && p != "private" {
        return Err(ValidationError::new("bad"));
    }
    Ok(())
}
