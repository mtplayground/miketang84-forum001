use askama::Template;

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub page_title: &'a str,
    pub heading: &'a str,
    pub intro: &'a str,
}
