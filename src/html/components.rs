use maud::{html, Markup};

/// `link rel="stylesheet" href=(url)`
pub fn stylesheet(url: &str, version: u16) -> Markup {
    let url = format!("{url}?v={version}");
    html! {
        link rel="stylesheet" href=(url);
    }
}

/// `script src=(url) defer`
pub fn script(url: &str, version: u16) -> Markup {
    let url = format!("{url}?v={version}");
    html! {
        script src=(url) defer {}
    }
}
