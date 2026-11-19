use pulldown_cmark::{Options, Parser};
use std::fs;
use std::io;
use std::path::Path;

use lol_html::{RewriteStrSettings, element, rewrite_str};

type Result<T> = std::result::Result<T, ParserError>;

#[derive(Debug)]
pub enum ParserError {
    Io(io::Error),
    ReadFailure,
    WriteFailure,
    RewriteFailure,
}

impl From<io::Error> for ParserError {
    fn from(e: io::Error) -> Self {
        ParserError::Io(e)
    }
}

#[derive(Debug)]
pub enum PageType {
    HomePage,
    BlogPost,
}

fn html_file_starter() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title></title>
    <link rel="stylesheet" href="/styles.css" />
    <link rel="icon" href="/favicon.png" /
</head>
<body>
"#
    .to_string()
}

fn html_file_end() -> String {
    "</body>\n</html>\n".to_string()
}

/// Renders a new HTML file given markdown input.
pub fn render_file(page_type: &PageType, md_path: &Path, html_path: &Path) -> Result<()> {
    // Read Markdown file
    let markdown_bytes: Vec<u8> = fs::read(md_path)?;
    let markdown_string =
        String::from_utf8(markdown_bytes).map_err(|_| ParserError::ReadFailure)?;

    // Parse Markdown -> HTML body only
    let parser = Parser::new_ext(&markdown_string, Options::all());
    let mut html_body = String::new();
    pulldown_cmark::html::push_html(&mut html_body, parser);

    // The home page links to blog posts via [[wikilinks]]; point them at publish/<post>.html
    if let PageType::HomePage = page_type {
        html_body = fix_post_links(&html_body)?;
    }

    // Compose final HTML
    let mut final_html = String::new();
    final_html.push_str(&html_file_starter());
    final_html.push_str(&html_body);
    final_html.push_str(&html_file_end());

    // Write final HTML file
    fs::write(html_path, final_html).map_err(|_| ParserError::WriteFailure)?;

    Ok(())
}

/// Render all of the content.
pub fn render_content(page_type: PageType, md_path: &Path, content_path: &Path) -> Result<()> {
    // Iterate thru each file in the Markdown path and render it.
    for entry in fs::read_dir(md_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or(ParserError::WriteFailure)?;

            let new_html_file = content_path.join(format!("{}.html", stem));

            render_file(&page_type, &path, &new_html_file)?;
        } else if path.is_dir() {
            // don't do anything
        }
    }

    Ok(())
}

/// Rewrite relative links (what pulldown_cmark emits for Obsidian [[wikilinks]])
/// so `href="blog-post-example"` becomes `href="publish/blog-post-example.html"`.
fn fix_post_links(html: &str) -> Result<String> {
    rewrite_str(
        html,
        RewriteStrSettings::new().append_element_content_handler(element!("a[href]", |el| {
            let href = el.get_attribute("href").unwrap_or_default();

            // Leave external, absolute, anchor, and already-fixed links alone.
            let is_external = href.contains("://") || href.starts_with("mailto:");
            let skip = href.is_empty()
                || is_external
                || href.starts_with('/')
                || href.starts_with('#')
                || href.ends_with(".html");

            if !skip {
                el.set_attribute("href", &format!("publish/{href}.html"))?;
            }
            Ok(())
        })),
    )
    .map_err(|_| ParserError::RewriteFailure)
}

/// Copy the photos from obsidian to the deployed site path
pub fn copy_photos(from: &Path, to: &Path) -> io::Result<()> {
    if !to.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Destination directory does not exist: {:?}", to),
        ));
    }

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let to = to.join(file_name);
                fs::copy(&path, &to)?;
                println!("Copied: {:?}", file_name);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(md: &str) -> String {
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, Parser::new_ext(md, Options::all()));
        fix_post_links(&html).unwrap()
    }

    #[test]
    fn wikilinks_point_to_published_html() {
        let out = render("[[blog-post-example]] and [[Save the Children - Marvin Gaye]]");
        assert!(
            out.contains(r#"href="publish/blog-post-example.html""#),
            "{out}"
        );
        assert!(
            out.contains(r#"href="publish/Save%20the%20Children%20-%20Marvin%20Gaye.html""#),
            "{out}"
        );
    }

    #[test]
    fn external_and_anchor_links_untouched() {
        let out = render("[a](https://example.com) [b](#top) [c](/about.html)");
        assert!(out.contains(r#"href="https://example.com""#));
        assert!(out.contains(r##"href="#top""##));
        assert!(out.contains(r#"href="/about.html""#));
    }
}
