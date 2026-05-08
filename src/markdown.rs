use pulldown_cmark::{html, Options, Parser};

#[allow(dead_code)]
pub fn render_markdown(markdown: &str) -> String {
    let mut rendered_html = String::new();
    let parser = Parser::new_ext(markdown, markdown_options());

    html::push_html(&mut rendered_html, parser);

    ammonia::Builder::default().clean(&rendered_html).to_string()
}

fn markdown_options() -> Options {
    Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
}

#[cfg(test)]
mod tests {
    use super::render_markdown;

    #[test]
    fn renders_basic_markdown() {
        let html = render_markdown("# Hello\n\n**world**");

        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn strips_script_tags_from_raw_html() {
        let html = render_markdown(r#"Hello<script>alert("xss")</script>"#);

        assert!(html.contains("<p>Hello</p>"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("alert(\"xss\")"));
    }

    #[test]
    fn strips_dangerous_attributes_and_urls() {
        let html = render_markdown(
            r#"<a href="javascript:alert('xss')">click me</a><img src="x" onerror="alert('xss')">"#,
        );

        assert!(html.contains("click me"));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("alert('xss')"));
    }
}
